use std::fmt;

use crate::{Dependency, Location, ModulePath};

/// One dependency rule violation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    rule_name: String,
    reason: String,
    source: ModulePath,
    target: ModulePath,
    location: Location,
}

impl Violation {
    pub(crate) fn new(rule_name: &str, reason: String, dependency: &Dependency) -> Self {
        Self {
            rule_name: rule_name.to_owned(),
            reason,
            source: dependency.source().clone(),
            target: dependency.target().clone(),
            location: dependency.location().clone(),
        }
    }

    /// Returns the rule name.
    #[must_use]
    pub fn rule_name(&self) -> &str {
        &self.rule_name
    }

    /// Returns the violation reason.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Returns the dependency source.
    #[must_use]
    pub fn source(&self) -> &ModulePath {
        &self.source
    }

    /// Returns the dependency target.
    #[must_use]
    pub fn target(&self) -> &ModulePath {
        &self.target
    }

    /// Returns the source location.
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.location
    }
}

/// Collection of dependency rule violations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Violations {
    items: Vec<Violation>,
}

impl Violations {
    /// Creates an empty violation list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, violation: Violation) {
        self.items.push(violation);
    }

    /// Appends another violation list.
    pub fn extend(&mut self, other: Self) {
        self.items.extend(other.items);
    }

    /// Returns `true` when there are no violations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of violations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterates over violations.
    pub fn iter(&self) -> impl Iterator<Item = &Violation> {
        self.items.iter()
    }

    /// Converts this collection into a vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<Violation> {
        self.items
    }

    /// Panics when any violation exists.
    ///
    /// # Panics
    ///
    /// Panics with the formatted violation list when the collection is not empty.
    #[track_caller]
    pub fn assert_empty(&self) {
        assert!(self.is_empty(), "{self}");
    }
}

impl IntoIterator for Violations {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = Violation;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a Violations {
    type IntoIter = std::slice::Iter<'a, Violation>;
    type Item = &'a Violation;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl fmt::Display for Violations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(formatter, "No dependency violations found.");
        }

        writeln!(
            formatter,
            "Dependency violations found: {}",
            self.items.len()
        )?;

        for violation in &self.items {
            writeln!(formatter)?;
            writeln!(formatter, "[{}]", violation.rule_name)?;
            write!(formatter, "{}", violation.location.file().display())?;
            if let Some(line) = violation.location.line() {
                write!(formatter, ":{line}")?;
                if let Some(column) = violation.location.column() {
                    write!(formatter, ":{column}")?;
                }
            }
            writeln!(formatter)?;
            writeln!(formatter)?;
            writeln!(formatter, "{} depends on", violation.source)?;
            writeln!(formatter, "{}", violation.target)?;
            writeln!(formatter)?;
            writeln!(formatter, "{}", violation.reason)?;
        }

        Ok(())
    }
}
