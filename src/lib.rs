//! Archaven checks dependency rules over Rust module paths.
//!
//! The crate exposes a small policy language:
//!
//! ```rust,no_run
//! use archaven::{Access, Archaven, Rule};
//!
//! let violations = Archaven::new()
//!     .rule(
//!         Rule::between("app::*")
//!             .named("bounded contexts")
//!             .deny_all()
//!             .allow(
//!                 Access::from("*::infrastructure::adapter::**")
//!                     .to("*::application::command::**"),
//!             ),
//!     )
//!     .check("./src")?;
//!
//! assert!(violations.is_empty(), "{violations}");
//! # Ok::<(), archaven::ArchavenError>(())
//! ```

mod error;
mod graph;
mod path;
mod rule;
mod scanner;
mod violation;

pub use crate::error::ArchavenError;
pub use crate::graph::{Dependency, DependencyGraph, Location};
pub use crate::path::{ModulePath, PathPattern, PrefixMatch};
pub use crate::rule::{Access, Rule, RuleSet};
pub use crate::violation::{Violation, Violations};

use std::path::Path;

/// Entry point for scanning Rust source files and checking dependency rules.
#[derive(Default)]
pub struct Archaven {
    rules: Vec<Box<dyn RuleSet>>,
}

impl Archaven {
    /// Creates an empty checker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a rule set to the checker.
    #[must_use]
    pub fn rule<R>(mut self, rule: R) -> Self
    where
        R: RuleSet + 'static,
    {
        self.rules.push(Box::new(rule));
        self
    }

    /// Scans a source directory and returns all dependency violations.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be walked, a Rust file cannot
    /// be read or parsed, or a configured rule contains an invalid pattern.
    pub fn check(&self, root: impl AsRef<Path>) -> Result<Violations, ArchavenError> {
        let graph = scanner::scan(root.as_ref())?;
        let mut violations = Violations::new();

        for rule in &self.rules {
            violations.extend(rule.check(&graph)?);
        }

        Ok(violations)
    }
}
