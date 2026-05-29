use std::fmt;

use crate::ArchavenError;

/// Rust module path split into `::`-separated segments.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ModulePath {
    segments: Vec<String>,
}

impl ModulePath {
    /// Parses a module path such as `app::sales::orders`.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty or contains empty segments.
    pub fn parse(path: &str) -> Result<Self, ArchavenError> {
        if path.trim().is_empty() {
            return Err(ArchavenError::invalid_pattern(path, "path cannot be empty"));
        }

        let segments = path
            .split("::")
            .map(str::trim)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        if segments.iter().any(String::is_empty) {
            return Err(ArchavenError::invalid_pattern(
                path,
                "path contains an empty segment",
            ));
        }

        Ok(Self { segments })
    }

    pub(crate) fn from_segments(segments: Vec<String>) -> Self {
        Self { segments }
    }

    /// Returns the path segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.segments.join("::"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PatternSegment {
    Literal(String),
    One,
    Many,
}

/// Segment-based pattern for matching Rust module paths.
///
/// `*` matches exactly one segment. `**` matches one or more segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathPattern {
    segments: Vec<PatternSegment>,
}

impl PathPattern {
    /// Parses a path pattern such as `app::*::domain::**`.
    ///
    /// # Errors
    ///
    /// Returns an error when the pattern is empty, contains empty segments, or
    /// uses `*` inside a literal segment.
    pub fn parse(pattern: &str) -> Result<Self, ArchavenError> {
        if pattern.trim().is_empty() {
            return Err(ArchavenError::invalid_pattern(
                pattern,
                "pattern cannot be empty",
            ));
        }

        let mut segments = Vec::new();
        for segment in pattern.split("::").map(str::trim) {
            match segment {
                "" => {
                    return Err(ArchavenError::invalid_pattern(
                        pattern,
                        "pattern contains an empty segment",
                    ));
                }
                "*" => segments.push(PatternSegment::One),
                "**" => segments.push(PatternSegment::Many),
                literal if literal.contains('*') => {
                    return Err(ArchavenError::invalid_pattern(
                        pattern,
                        "`*` can only be used as a whole segment",
                    ));
                }
                literal => segments.push(PatternSegment::Literal(literal.to_owned())),
            }
        }

        Ok(Self { segments })
    }

    /// Returns `true` when this pattern matches the full path.
    #[must_use]
    pub fn matches(&self, path: &ModulePath) -> bool {
        matches_from(&self.segments, path.segments(), 0, 0)
    }

    /// Matches this pattern against a prefix of `path`.
    ///
    /// When multiple prefixes match, the shortest matching prefix is returned.
    #[must_use]
    pub fn match_prefix(&self, path: &ModulePath) -> Option<PrefixMatch> {
        (1..=path.segments().len()).find_map(|end| {
            let prefix = ModulePath::from_segments(path.segments()[..end].to_vec());
            if self.matches(&prefix) {
                let remainder = ModulePath::from_segments(path.segments()[end..].to_vec());
                Some(PrefixMatch {
                    matched: prefix,
                    remainder,
                })
            } else {
                None
            }
        })
    }
}

/// Result of matching a pattern against the prefix of a module path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixMatch {
    matched: ModulePath,
    remainder: ModulePath,
}

impl PrefixMatch {
    /// Returns the matched prefix.
    #[must_use]
    pub fn matched(&self) -> &ModulePath {
        &self.matched
    }

    /// Returns the path after the matched prefix.
    #[must_use]
    pub fn remainder(&self) -> &ModulePath {
        &self.remainder
    }
}

fn matches_from(
    pattern: &[PatternSegment],
    path: &[String],
    pattern_index: usize,
    path_index: usize,
) -> bool {
    if pattern_index == pattern.len() {
        return path_index == path.len();
    }

    match &pattern[pattern_index] {
        PatternSegment::Literal(expected) => {
            path.get(path_index)
                .is_some_and(|actual| actual == expected)
                && matches_from(pattern, path, pattern_index + 1, path_index + 1)
        }
        PatternSegment::One => {
            path_index < path.len()
                && matches_from(pattern, path, pattern_index + 1, path_index + 1)
        }
        PatternSegment::Many => {
            if path_index >= path.len() {
                return false;
            }

            ((path_index + 1)..=path.len())
                .any(|next_index| matches_from(pattern, path, pattern_index + 1, next_index))
        }
    }
}
