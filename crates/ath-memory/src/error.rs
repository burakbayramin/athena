//! Memory subsystem error types with actionable fix hints.

/// Errors that can occur in the memory subsystem.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    /// A URI string could not be parsed as a valid `viking://` URI.
    #[error("Invalid viking URI '{input}': {reason}")]
    InvalidUri {
        /// The raw input that failed to parse.
        input: String,
        /// What was wrong with it.
        reason: String,
    },

    /// A URI or path resolution attempted to escape the allowed root directory.
    #[error("Path traversal blocked for URI '{uri}' (root: {root})")]
    PathTraversal {
        /// The URI that caused the traversal attempt.
        uri: String,
        /// The root directory it tried to escape.
        root: String,
    },

    /// A filesystem I/O operation failed.
    #[error("I/O error at '{path}': {message}")]
    IoError {
        /// The path involved.
        path: String,
        /// Description of the failure.
        message: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Serialization or deserialization of memory content failed.
    #[error("Serialization error: {message}")]
    SerializationError {
        /// What went wrong.
        message: String,
        /// The underlying serde_json error, if available.
        #[source]
        source: Option<serde_json::Error>,
    },

    /// An error occurred in the vector index.
    #[error("Index error: {message}")]
    IndexError {
        /// Description of the index failure.
        message: String,
    },

    /// The vector dimension of new data doesn't match the existing index dimension.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// The dimension the index was created with.
        expected: usize,
        /// The dimension of the incoming vector.
        actual: usize,
    },
}

impl MemoryError {
    /// Returns an actionable hint for resolving this error.
    pub fn hint(&self) -> &str {
        match self {
            MemoryError::InvalidUri { .. } => {
                "URIs must use the 'viking://' scheme followed by one or more path segments (e.g., 'viking://project/conventions')"
            }
            MemoryError::PathTraversal { .. } => {
                "Remove '..' segments from the URI — all paths must resolve within the memory store root"
            }
            MemoryError::IoError { .. } => {
                "Check that the .ath/memory/ directory exists and is writable, then retry"
            }
            MemoryError::SerializationError { .. } => {
                "The stored content may be corrupted — inspect the file manually or delete and re-extract"
            }
            MemoryError::IndexError { .. } => {
                "The vector index may be corrupted — delete .ath/memory/index/ and rebuild"
            }
            MemoryError::DimensionMismatch { .. } => {
                "The embedding model dimension changed — rebuild the index with the new model or switch back to the original"
            }
        }
    }
}

impl From<std::io::Error> for MemoryError {
    fn from(err: std::io::Error) -> Self {
        MemoryError::IoError {
            path: String::new(),
            message: err.to_string(),
            source: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_uri_displays_input() {
        let err = MemoryError::InvalidUri {
            input: "bad://thing".to_string(),
            reason: "wrong scheme".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("bad://thing"));
        assert!(msg.contains("wrong scheme"));
    }

    #[test]
    fn path_traversal_displays_uri_and_root() {
        let err = MemoryError::PathTraversal {
            uri: "viking://../../etc".to_string(),
            root: "/safe/root".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("viking://../../etc"));
        assert!(msg.contains("/safe/root"));
    }

    #[test]
    fn io_error_has_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err = MemoryError::IoError {
            path: "/tmp/missing".to_string(),
            message: "file not found".to_string(),
            source: io_err,
        };
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn dimension_mismatch_displays_values() {
        let err = MemoryError::DimensionMismatch {
            expected: 384,
            actual: 768,
        };
        let msg = format!("{err}");
        assert!(msg.contains("384"));
        assert!(msg.contains("768"));
    }

    #[test]
    fn all_hints_are_actionable() {
        let cases: Vec<MemoryError> = vec![
            MemoryError::InvalidUri {
                input: "x".into(),
                reason: "y".into(),
            },
            MemoryError::PathTraversal {
                uri: "x".into(),
                root: "y".into(),
            },
            MemoryError::IoError {
                path: "x".into(),
                message: "y".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "z"),
            },
            MemoryError::SerializationError {
                message: "x".into(),
                source: None,
            },
            MemoryError::IndexError {
                message: "x".into(),
            },
            MemoryError::DimensionMismatch {
                expected: 1,
                actual: 2,
            },
        ];

        for err in &cases {
            let hint = err.hint();
            assert!(!hint.is_empty(), "hint for {err:?} should not be empty");
            // Each hint should be a sentence with actionable content.
            assert!(
                hint.len() > 20,
                "hint for {err:?} seems too short to be actionable: '{hint}'"
            );
        }
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        let err: MemoryError = io_err.into();
        assert!(matches!(err, MemoryError::IoError { .. }));
    }
}
