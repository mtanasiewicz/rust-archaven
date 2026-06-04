use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use syn::{
    punctuated::Punctuated,
    spanned::Spanned,
    visit::{self, Visit},
    ItemUse, Token, UseTree,
};
use walkdir::WalkDir;

use crate::graph::SourceDirectory;
use crate::{ArchavenError, Dependency, DependencyGraph, Location, ModulePath};

const STANDARD_EXTERNAL_ROOTS: &[&str] = &["alloc", "core", "proc_macro", "std"];

pub(crate) fn scan(
    root: &Path,
    include_external_dependencies: bool,
) -> Result<DependencyGraph, ArchavenError> {
    let files = discover_files(root)?;
    let roots = local_roots(&files);
    let mut graph = DependencyGraph::new();

    for directory in discover_directories(root)? {
        graph.push_directory(directory);
    }

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

        let aliases = collect_aliases(
            &syntax,
            &source_file.module,
            &roots,
            include_external_dependencies,
        );
        let mut visitor = DependencyVisitor {
            source: &source_file.module,
            roots: &roots,
            include_external_dependencies,
            aliases,
            file: &source_file.path,
            graph: &mut graph,
        };
        visitor.visit_file(&syntax);
    }

    Ok(graph)
}

fn discover_directories(root: &Path) -> Result<Vec<SourceDirectory>, ArchavenError> {
    let mut directories = Vec::new();

    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|source| ArchavenError::WalkDir {
            path: source
                .path()
                .map_or_else(|| root.to_path_buf(), Path::to_path_buf),
            source,
        })?;

        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();
        let files = fs::read_dir(path)
            .map_err(|source| ArchavenError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?
            .map(|entry| {
                entry.map_err(|source| ArchavenError::ReadFile {
                    path: path.to_path_buf(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let rust_files = files
            .iter()
            .filter_map(|entry| {
                let path = entry.path();
                let is_rust_file = entry
                    .file_type()
                    .ok()
                    .is_some_and(|file_type| file_type.is_file())
                    && path.extension().is_some_and(|extension| extension == "rs");

                is_rust_file
                    .then(|| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                    })
                    .flatten()
            })
            .collect::<BTreeSet<_>>();

        let child_directories = files
            .iter()
            .filter(|entry| {
                entry
                    .file_type()
                    .ok()
                    .is_some_and(|file_type| file_type.is_dir())
            })
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<BTreeSet<_>>();

        directories.push(SourceDirectory::new(
            path,
            module_path_for_directory(root, path),
            rust_files,
            child_directories,
        ));
    }

    directories.sort_by(|left, right| left.path().cmp(right.path()));
    Ok(directories)
}

fn module_path_for_directory(root: &Path, directory: &Path) -> ModulePath {
    let segments = directory
        .strip_prefix(root)
        .ok()
        .into_iter()
        .flat_map(Path::components)
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    ModulePath::from_segments(segments)
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

type AliasMap = BTreeMap<String, Vec<String>>;

fn collect_aliases(
    file: &syn::File,
    source: &ModulePath,
    roots: &BTreeSet<String>,
    include_external_dependencies: bool,
) -> AliasMap {
    let mut aliases = AliasMap::new();

    for item in &file.items {
        if let syn::Item::Use(item) = item {
            let mut prefix = Vec::new();
            collect_use_aliases(
                source,
                roots,
                include_external_dependencies,
                &item.tree,
                &mut prefix,
                &mut aliases,
            );
        }
    }

    aliases
}

fn collect_use_aliases(
    source: &ModulePath,
    roots: &BTreeSet<String>,
    include_external_dependencies: bool,
    tree: &UseTree,
    prefix: &mut Vec<String>,
    aliases: &mut AliasMap,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_aliases(
                source,
                roots,
                include_external_dependencies,
                &path.tree,
                prefix,
                aliases,
            );
            prefix.pop();
        }
        UseTree::Name(_) | UseTree::Glob(_) => {}
        UseTree::Rename(rename) => {
            let segments = use_target_segments(prefix, &rename.ident.to_string());
            insert_alias(
                source,
                roots,
                include_external_dependencies,
                aliases,
                rename.rename.to_string(),
                &segments,
            );
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_aliases(
                    source,
                    roots,
                    include_external_dependencies,
                    item,
                    prefix,
                    aliases,
                );
            }
        }
    }
}

fn use_target_segments(prefix: &[String], ident: &str) -> Vec<String> {
    if ident == "self" {
        return prefix.to_vec();
    }

    let mut segments = prefix.to_vec();
    segments.push(ident.to_owned());
    segments
}

fn insert_alias(
    source: &ModulePath,
    roots: &BTreeSet<String>,
    include_external_dependencies: bool,
    aliases: &mut AliasMap,
    alias: String,
    segments: &[String],
) {
    let Some(target) = resolve_segments(
        source,
        roots,
        include_external_dependencies,
        aliases,
        segments,
        SegmentContext::Import,
    ) else {
        return;
    };

    if target.is_empty() {
        return;
    }

    aliases.insert(alias, target.segments().to_vec());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SegmentContext {
    Import,
    Path,
}

struct DependencyVisitor<'a> {
    source: &'a ModulePath,
    roots: &'a BTreeSet<String>,
    include_external_dependencies: bool,
    aliases: AliasMap,
    file: &'a Path,
    graph: &'a mut DependencyGraph,
}

impl<'ast> Visit<'ast> for DependencyVisitor<'_> {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if attribute.path().is_ident("derive") {
            if let Ok(paths) =
                attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            {
                for path in paths {
                    self.collect_path(&path);
                }
            }
        }

        visit::visit_attribute(self, attribute);
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.collect_use_tree(&item.tree, &mut prefix);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.collect_path(path);
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
                let segments = use_target_segments(prefix, &name.ident.to_string());
                self.collect_segments(&segments, name.ident.span(), SegmentContext::Import);
            }
            UseTree::Rename(rename) => {
                let segments = use_target_segments(prefix, &rename.ident.to_string());
                self.collect_segments(&segments, rename.ident.span(), SegmentContext::Import);
            }
            UseTree::Glob(glob) => {
                self.collect_segments(prefix, glob.star_token.span, SegmentContext::Import);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.collect_use_tree(item, prefix);
                }
            }
        }
    }

    fn collect_segments(
        &mut self,
        segments: &[String],
        span: proc_macro2::Span,
        context: SegmentContext,
    ) {
        let Some(target) = resolve_segments(
            self.source,
            self.roots,
            self.include_external_dependencies,
            &self.aliases,
            segments,
            context,
        ) else {
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

    fn collect_path(&mut self, path: &syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        self.collect_segments(&segments, path.span(), SegmentContext::Path);
    }
}

fn resolve_segments(
    source: &ModulePath,
    roots: &BTreeSet<String>,
    include_external_dependencies: bool,
    aliases: &AliasMap,
    segments: &[String],
    context: SegmentContext,
) -> Option<ModulePath> {
    let first = segments.first()?;

    if let Some(alias_target) = aliases.get(first) {
        let mut resolved = alias_target.clone();
        resolved.extend_from_slice(&segments[1..]);
        return resolve_absolute_segments(roots, include_external_dependencies, &resolved, context);
    }

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
        _ => resolve_absolute_segments(roots, include_external_dependencies, segments, context),
    }
}

fn resolve_absolute_segments(
    roots: &BTreeSet<String>,
    include_external_dependencies: bool,
    segments: &[String],
    context: SegmentContext,
) -> Option<ModulePath> {
    let first = segments.first()?;

    if roots.contains(first) {
        return Some(ModulePath::from_segments(segments.to_vec()));
    }

    if !include_external_dependencies
        || is_standard_external_root(first)
        || (context == SegmentContext::Path && segments.len() == 1)
    {
        return None;
    }

    Some(ModulePath::from_segments(segments.to_vec()))
}

fn is_standard_external_root(root: &str) -> bool {
    STANDARD_EXTERNAL_ROOTS.contains(&root)
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
