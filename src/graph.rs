use std::path::{Path, PathBuf};

use crate::ModulePath;

/// Source location for a discovered dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    file: PathBuf,
    line: Option<usize>,
    column: Option<usize>,
}

impl Location {
    /// Creates a location with a file path and no line or column.
    #[must_use]
    pub fn new(file: impl AsRef<Path>) -> Self {
        Self {
            file: file.as_ref().to_path_buf(),
            line: None,
            column: None,
        }
    }

    pub(crate) fn with_line_column(
        file: impl AsRef<Path>,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        Self {
            file: file.as_ref().to_path_buf(),
            line,
            column,
        }
    }

    /// Returns the source file path.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the 1-based source line, when available.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        self.line
    }

    /// Returns the 1-based source column, when available.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        self.column
    }
}

/// A dependency from one Rust module path to another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    source: ModulePath,
    target: ModulePath,
    location: Location,
}

impl Dependency {
    /// Creates a dependency.
    #[must_use]
    pub fn new(source: ModulePath, target: ModulePath, location: Location) -> Self {
        Self {
            source,
            target,
            location,
        }
    }

    /// Returns the source module path.
    #[must_use]
    pub fn source(&self) -> &ModulePath {
        &self.source
    }

    /// Returns the target module path.
    #[must_use]
    pub fn target(&self) -> &ModulePath {
        &self.target
    }

    /// Returns where the dependency was found.
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.location
    }
}

/// Dependency graph discovered from Rust source files.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyGraph {
    dependencies: Vec<Dependency>,
}

impl DependencyGraph {
    /// Creates an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a graph from dependencies.
    pub fn from_dependencies<I>(dependencies: I) -> Self
    where
        I: IntoIterator<Item = Dependency>,
    {
        Self {
            dependencies: dependencies.into_iter().collect(),
        }
    }

    /// Adds one dependency to the graph.
    pub fn push(&mut self, dependency: Dependency) {
        self.dependencies.push(dependency);
    }

    /// Returns all dependencies in discovery order.
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }
}
