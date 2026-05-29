use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    ItemUse, UseTree,
};
use walkdir::WalkDir;

use crate::{ArchavenError, Dependency, DependencyGraph, Location, ModulePath};

pub(crate) fn scan(root: &Path) -> Result<DependencyGraph, ArchavenError> {
    let files = discover_files(root)?;
    let roots = local_roots(&files);
    let mut graph = DependencyGraph::new();

    for source_file in files {
        let content =
            fs::read_to_string(&source_file.path).map_err(|source| ArchavenError::ReadFile {
                path: source_file.path.clone(),
                source,
            })?;
        let syntax = syn::parse_file(&content).map_err(|source| ArchavenError::ParseFile {
            path: source_file.path.clone(),
            source,
        })?;

        let mut visitor = DependencyVisitor {
            source: &source_file.module,
            roots: &roots,
            file: &source_file.path,
            graph: &mut graph,
        };
        visitor.visit_file(&syntax);
    }

    Ok(graph)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceFile {
    path: PathBuf,
    module: ModulePath,
}

fn discover_files(root: &Path) -> Result<Vec<SourceFile>, ArchavenError> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|source| ArchavenError::WalkDir {
            path: source
                .path()
                .map_or_else(|| root.to_path_buf(), Path::to_path_buf),
            source,
        })?;

        let is_rust_file = entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "rs");

        if !entry.file_type().is_file() || !is_rust_file {
            continue;
        }

        if let Some(module) = module_path_for_file(root, entry.path()) {
            files.push(SourceFile {
                path: entry.path().to_path_buf(),
                module,
            });
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn module_path_for_file(root: &Path, file: &Path) -> Option<ModulePath> {
    let relative = file.strip_prefix(root).ok()?;
    let mut segments = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let stem = file.file_stem()?.to_string_lossy();
    match stem.as_ref() {
        "lib" | "main" if segments.is_empty() => {}
        "mod" => {}
        other => segments.push(other.to_owned()),
    }

    Some(ModulePath::from_segments(segments))
}

fn local_roots(files: &[SourceFile]) -> BTreeSet<String> {
    files
        .iter()
        .filter_map(|file| file.module.segments().first().cloned())
        .collect()
}

struct DependencyVisitor<'a> {
    source: &'a ModulePath,
    roots: &'a BTreeSet<String>,
    file: &'a Path,
    graph: &'a mut DependencyGraph,
}

impl<'ast> Visit<'ast> for DependencyVisitor<'_> {
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.collect_use_tree(&item.tree, &mut prefix);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        self.collect_segments(&segments, path.span());
        visit::visit_path(self, path);
    }
}

impl DependencyVisitor<'_> {
    fn collect_use_tree(&mut self, tree: &UseTree, prefix: &mut Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect_use_tree(&path.tree, prefix);
                prefix.pop();
            }
            UseTree::Name(name) => {
                prefix.push(name.ident.to_string());
                self.collect_segments(prefix, name.ident.span());
                prefix.pop();
            }
            UseTree::Rename(rename) => {
                prefix.push(rename.ident.to_string());
                self.collect_segments(prefix, rename.ident.span());
                prefix.pop();
            }
            UseTree::Glob(glob) => {
                self.collect_segments(prefix, glob.star_token.span);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.collect_use_tree(item, prefix);
                }
            }
        }
    }

    fn collect_segments(&mut self, segments: &[String], span: proc_macro2::Span) {
        let Some(target) = resolve_segments(self.source, self.roots, segments) else {
            return;
        };

        if target.is_empty() {
            return;
        }

        let start = span.start();
        self.graph.push(Dependency::new(
            self.source.clone(),
            target,
            Location::with_line_column(self.file, Some(start.line), Some(start.column + 1)),
        ));
    }
}

fn resolve_segments(
    source: &ModulePath,
    roots: &BTreeSet<String>,
    segments: &[String],
) -> Option<ModulePath> {
    let first = segments.first()?;

    match first.as_str() {
        "crate" => Some(ModulePath::from_segments(segments[1..].to_vec())),
        "self" => {
            let mut resolved = source.segments().to_vec();
            resolved.extend_from_slice(&segments[1..]);
            Some(ModulePath::from_segments(resolved))
        }
        "super" => resolve_super_segments(source, segments),
        local_root if roots.contains(local_root) => {
            Some(ModulePath::from_segments(segments.to_vec()))
        }
        _ => None,
    }
}

fn resolve_super_segments(source: &ModulePath, segments: &[String]) -> Option<ModulePath> {
    let mut resolved = source.segments().to_vec();
    let mut index = 0;

    while segments
        .get(index)
        .is_some_and(|segment| segment == "super")
    {
        resolved.pop()?;
        index += 1;
    }

    if segments.get(index).is_some_and(|segment| segment == "self") {
        index += 1;
    }

    resolved.extend_from_slice(&segments[index..]);
    Some(ModulePath::from_segments(resolved))
}
