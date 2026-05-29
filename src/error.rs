use std::path::PathBuf;

use thiserror::Error;

/// Error returned when Archaven cannot complete analysis.
#[derive(Debug, Error)]
pub enum ArchavenError {
    /// A path pattern is malformed.
    #[error("invalid pattern `{pattern}`: {message}")]
    InvalidPattern {
        /// Original pattern string.
        pattern: String,
        /// Human-readable explanation.
        message: String,
    },

    /// A rule is internally inconsistent.
    #[error("invalid rule `{rule}`: {message}")]
    InvalidRule {
        /// Rule name.
        rule: String,
        /// Human-readable explanation.
        message: String,
    },

    /// Walking the source tree failed.
    #[error("failed to walk `{path}`: {source}")]
    WalkDir {
        /// Path being walked.
        path: PathBuf,
        /// Original I/O error.
        #[source]
        source: walkdir::Error,
    },

    /// Reading a Rust source file failed.
    #[error("failed to read `{path}`: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Original I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Parsing a Rust source file failed.
    #[error("failed to parse Rust file `{path}`: {source}")]
    ParseFile {
        /// File path.
        path: PathBuf,
        /// Original parser error.
        #[source]
        source: syn::Error,
    },
}

impl ArchavenError {
    pub(crate) fn invalid_pattern(pattern: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidPattern {
            pattern: pattern.into(),
            message: message.into(),
        }
    }

    pub(crate) fn invalid_rule(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidRule {
            rule: rule.into(),
            message: message.into(),
        }
    }
}
