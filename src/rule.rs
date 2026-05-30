use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{
    ArchavenError, Dependency, DependencyGraph, ModulePath, PathPattern, Violation, Violations,
};

/// A custom dependency rule set.
pub trait RuleSet {
    /// Checks a graph and returns all violations found by this rule set.
    ///
    /// # Errors
    ///
    /// Returns an error when the rule set configuration is invalid.
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError>;
}

/// Describes dependency access from one path pattern to one or more target patterns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Access {
    from: String,
    to: Vec<String>,
    reason: Option<String>,
}

impl Access {
    /// Starts an access rule from a source pattern.
    #[must_use]
    pub fn from(pattern: impl Into<String>) -> Self {
        Self {
            from: pattern.into(),
            to: Vec::new(),
            reason: None,
        }
    }

    /// Adds one allowed or denied target pattern.
    #[must_use]
    pub fn to(mut self, pattern: impl Into<String>) -> Self {
        self.to.push(pattern.into());
        self
    }

    /// Adds many allowed or denied target patterns.
    #[must_use]
    pub fn to_any<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.to.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Adds a human-readable reason used in violation messages.
    #[must_use]
    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    fn compile(&self, rule_name: &str) -> Result<CompiledAccess, ArchavenError> {
        if self.to.is_empty() {
            return Err(ArchavenError::invalid_rule(
                rule_name,
                "access rule must define at least one target pattern",
            ));
        }

        let from = PathPattern::parse(&self.from)?;
        let to = self
            .to
            .iter()
            .map(|pattern| PathPattern::parse(pattern))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CompiledAccess {
            from,
            to,
            reason: self.reason.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompiledAccess {
    from: PathPattern,
    to: Vec<PathPattern>,
    reason: Option<String>,
}

impl CompiledAccess {
    fn matches(&self, source: &ModulePath, target: &ModulePath) -> bool {
        self.from.matches(source) && self.to.iter().any(|pattern| pattern.matches(target))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Scope {
    Global,
    Between(String),
    Within(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CompiledScope {
    Global,
    Between(PathPattern),
    Within(PathPattern),
}

/// Neutral dependency rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rule {
    name: String,
    scope: Scope,
    deny_all: bool,
    allows: Vec<Access>,
    denies: Vec<Access>,
    ignored_files: Vec<String>,
    reason: Option<String>,
}

impl Rule {
    /// Creates a global rule over absolute module path patterns.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "dependency rule".to_owned(),
            scope: Scope::Global,
            deny_all: false,
            allows: Vec::new(),
            denies: Vec::new(),
            ignored_files: Vec::new(),
            reason: None,
        }
    }

    /// Creates a rule for dependencies between different instances of a scope pattern.
    #[must_use]
    pub fn between(scope: impl Into<String>) -> Self {
        Self {
            scope: Scope::Between(scope.into()),
            ..Self::new()
        }
    }

    /// Creates a rule for dependencies inside the same instance of a scope pattern.
    #[must_use]
    pub fn within(scope: impl Into<String>) -> Self {
        Self {
            scope: Scope::Within(scope.into()),
            ..Self::new()
        }
    }

    /// Sets a human-readable rule name.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Denies all dependencies in this rule's scope unless an `allow` matches.
    #[must_use]
    pub fn deny_all(mut self) -> Self {
        self.deny_all = true;
        self
    }

    /// Adds an allowed access exception.
    #[must_use]
    pub fn allow(mut self, access: Access) -> Self {
        self.allows.push(access);
        self
    }

    /// Adds an explicitly denied access pattern.
    #[must_use]
    pub fn deny(mut self, access: Access) -> Self {
        self.denies.push(access);
        self
    }

    /// Ignores dependencies discovered in source files matching the given glob patterns.
    #[must_use]
    pub fn ignore_files<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.ignored_files.extend(
            patterns
                .into_iter()
                .map(|pattern| pattern.as_ref().to_owned()),
        );
        self
    }

    /// Adds a default reason used when no more specific reason is available.
    #[must_use]
    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Checks this rule against an already-built dependency graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the rule contains an invalid pattern.
    pub fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        Ok(self.compile()?.check(graph))
    }

    fn compile(&self) -> Result<CompiledRule, ArchavenError> {
        let scope = match &self.scope {
            Scope::Global => CompiledScope::Global,
            Scope::Between(pattern) => CompiledScope::Between(PathPattern::parse(pattern)?),
            Scope::Within(pattern) => CompiledScope::Within(PathPattern::parse(pattern)?),
        };

        let allows = self
            .allows
            .iter()
            .map(|access| access.compile(&self.name))
            .collect::<Result<Vec<_>, _>>()?;
        let denies = self
            .denies
            .iter()
            .map(|access| access.compile(&self.name))
            .collect::<Result<Vec<_>, _>>()?;
        let ignored_files = compile_ignored_files(&self.ignored_files)?;

        Ok(CompiledRule {
            name: self.name.clone(),
            scope,
            deny_all: self.deny_all,
            allows,
            denies,
            ignored_files,
            reason: self.reason.clone(),
        })
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleSet for Rule {
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        Ok(self.compile()?.check(graph))
    }
}

struct CompiledRule {
    name: String,
    scope: CompiledScope,
    deny_all: bool,
    allows: Vec<CompiledAccess>,
    denies: Vec<CompiledAccess>,
    ignored_files: GlobSet,
    reason: Option<String>,
}

impl CompiledRule {
    fn check(&self, graph: &DependencyGraph) -> Violations {
        let mut violations = Violations::new();

        for dependency in graph.dependencies() {
            if self.ignores_file(dependency.location().file()) {
                continue;
            }

            if let Some(context) = self.context(dependency) {
                if let Some(deny) = self
                    .denies
                    .iter()
                    .find(|access| access.matches(&context.source, &context.target))
                {
                    violations.push(Violation::new(
                        &self.name,
                        self.reason_for_explicit_deny(deny),
                        dependency,
                    ));
                    continue;
                }

                if self.deny_all
                    && !self
                        .allows
                        .iter()
                        .any(|access| access.matches(&context.source, &context.target))
                {
                    violations.push(Violation::new(
                        &self.name,
                        self.reason_for_default_deny(),
                        dependency,
                    ));
                }
            }
        }

        violations
    }

    fn ignores_file(&self, file: &Path) -> bool {
        let normalized = file.to_string_lossy().replace('\\', "/");

        if self.ignored_files.is_match(normalized.as_str()) {
            return true;
        }

        let trimmed = normalized.trim_start_matches('/');
        let segments = trimmed.split('/').collect::<Vec<_>>();
        (1..segments.len()).any(|start| {
            let suffix = segments[start..].join("/");
            self.ignored_files.is_match(suffix.as_str())
        })
    }

    fn context(&self, dependency: &Dependency) -> Option<EvalContext> {
        match &self.scope {
            CompiledScope::Global => Some(EvalContext {
                source: dependency.source().clone(),
                target: dependency.target().clone(),
            }),
            CompiledScope::Between(pattern) => {
                let source = pattern.match_prefix(dependency.source())?;
                let target = pattern.match_prefix(dependency.target())?;

                (source.matched() != target.matched()).then(|| EvalContext {
                    source: source.remainder().clone(),
                    target: target.remainder().clone(),
                })
            }
            CompiledScope::Within(pattern) => {
                let source = pattern.match_prefix(dependency.source())?;
                let target = pattern.match_prefix(dependency.target())?;

                (source.matched() == target.matched()).then(|| EvalContext {
                    source: source.remainder().clone(),
                    target: target.remainder().clone(),
                })
            }
        }
    }

    fn reason_for_explicit_deny(&self, deny: &CompiledAccess) -> String {
        deny.reason
            .clone()
            .or_else(|| self.reason.clone())
            .unwrap_or_else(|| "dependency is denied by this rule".to_owned())
    }

    fn reason_for_default_deny(&self) -> String {
        if let Some(reason) = &self.reason {
            return reason.clone();
        }

        let reasons = self
            .allows
            .iter()
            .filter_map(|access| access.reason.as_deref())
            .collect::<Vec<_>>();

        if reasons.is_empty() {
            "dependency is not allowed by this rule".to_owned()
        } else {
            format!(
                "dependency is not allowed by this rule; allowed access: {}",
                reasons.join("; ")
            )
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvalContext {
    source: ModulePath,
    target: ModulePath,
}

fn compile_ignored_files(patterns: &[String]) -> Result<GlobSet, ArchavenError> {
    let mut builder = GlobSetBuilder::new();

    for pattern in patterns {
        let glob = Glob::new(pattern)
            .map_err(|source| ArchavenError::invalid_pattern(pattern, source.to_string()))?;
        builder.add(glob);
    }

    builder
        .build()
        .map_err(|source| ArchavenError::invalid_pattern(patterns.join(", "), source.to_string()))
}
