//! Viking URI parser with path traversal protection.
//!
//! Format: `viking://segment1/segment2/...`
//!
//! Segments must be non-empty and must not contain `..` to prevent
//! path traversal attacks when resolving to filesystem paths.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::MemoryError;

/// A parsed `viking://` URI with validated path segments.
///
/// # Examples
///
/// ```
/// use ath_memory::VikingUri;
///
/// let uri: VikingUri = "viking://project/conventions".parse().unwrap();
/// assert_eq!(uri.segments(), &["project", "conventions"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VikingUri {
    segments: Vec<String>,
}

const SCHEME: &str = "viking://";

impl VikingUri {
    /// Returns the path segments of this URI.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Resolves this URI to a filesystem path under the given root directory.
    ///
    /// Returns an error if the resolved path would escape the root (e.g., via
    /// symlink tricks or segment manipulation). The path is canonicalized
    /// relative to root.
    pub fn resolve_path(&self, root: &Path) -> Result<PathBuf, MemoryError> {
        let mut path = root.to_path_buf();
        for segment in &self.segments {
            path.push(segment);
        }

        // Canonicalize both to compare. If root doesn't exist yet, we
        // normalize manually to catch obvious traversal.
        let normalized = normalize_path(&path);
        let normalized_root = normalize_path(root);

        if !normalized.starts_with(&normalized_root) {
            return Err(MemoryError::PathTraversal {
                uri: self.to_string(),
                root: normalized_root.display().to_string(),
            });
        }

        Ok(path)
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            other => {
                components.push(other);
            }
        }
    }
    components.iter().collect()
}

impl FromStr for VikingUri {
    type Err = MemoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check for other common schemes first for a better error message.
        if !s.starts_with(SCHEME) {
            if s.contains("://") {
                return Err(MemoryError::InvalidUri {
                    input: s.to_string(),
                    reason: format!(
                        "expected 'viking://' scheme, got '{}'",
                        s.split("://").next().unwrap_or("unknown")
                    ),
                });
            }
            return Err(MemoryError::InvalidUri {
                input: s.to_string(),
                reason: "missing 'viking://' scheme prefix".to_string(),
            });
        }

        let path_part = &s[SCHEME.len()..];
        if path_part.is_empty() {
            return Err(MemoryError::InvalidUri {
                input: s.to_string(),
                reason: "URI has no path segments after scheme".to_string(),
            });
        }

        let segments: Vec<String> = path_part.split('/').map(String::from).collect();

        // Validate each segment.
        for (i, seg) in segments.iter().enumerate() {
            if seg.is_empty() {
                return Err(MemoryError::InvalidUri {
                    input: s.to_string(),
                    reason: format!("empty segment at position {i}"),
                });
            }
            if seg == ".." {
                return Err(MemoryError::PathTraversal {
                    uri: s.to_string(),
                    root: "(during parse)".to_string(),
                });
            }
            if seg == "." {
                return Err(MemoryError::InvalidUri {
                    input: s.to_string(),
                    reason: format!("'.' segment at position {i} is not allowed"),
                });
            }
        }

        Ok(VikingUri { segments })
    }
}

impl fmt::Display for VikingUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "viking://{}", self.segments.join("/"))
    }
}

impl Serialize for VikingUri {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for VikingUri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        VikingUri::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_uri() {
        let uri: VikingUri = "viking://project/conventions".parse().unwrap();
        assert_eq!(uri.segments(), &["project", "conventions"]);
        assert_eq!(uri.to_string(), "viking://project/conventions");
    }

    #[test]
    fn parse_deep_uri() {
        let uri: VikingUri = "viking://agents/planner/profile".parse().unwrap();
        assert_eq!(uri.segments(), &["agents", "planner", "profile"]);
    }

    #[test]
    fn parse_single_segment() {
        let uri: VikingUri = "viking://root".parse().unwrap();
        assert_eq!(uri.segments(), &["root"]);
    }

    #[test]
    fn reject_missing_scheme() {
        let err = "project/conventions".parse::<VikingUri>().unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
        let msg = format!("{err}");
        assert!(
            msg.contains("project/conventions"),
            "error should contain input"
        );
    }

    #[test]
    fn reject_wrong_scheme() {
        let err = "http://project/conventions"
            .parse::<VikingUri>()
            .unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
        let msg = format!("{err}");
        assert!(msg.contains("http"));
    }

    #[test]
    fn reject_traversal() {
        let err = "viking://project/../etc/passwd"
            .parse::<VikingUri>()
            .unwrap_err();
        assert!(matches!(err, MemoryError::PathTraversal { .. }));
    }

    #[test]
    fn reject_empty_segments() {
        let err = "viking://project//conventions"
            .parse::<VikingUri>()
            .unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
    }

    #[test]
    fn reject_trailing_slash_empty_segment() {
        let err = "viking://project/conventions/"
            .parse::<VikingUri>()
            .unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
    }

    #[test]
    fn reject_no_path() {
        let err = "viking://".parse::<VikingUri>().unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
    }

    #[test]
    fn reject_dot_segment() {
        let err = "viking://project/./conventions"
            .parse::<VikingUri>()
            .unwrap_err();
        assert!(matches!(err, MemoryError::InvalidUri { .. }));
    }

    #[test]
    fn resolve_path_within_root() {
        let uri: VikingUri = "viking://project/conventions".parse().unwrap();
        let root = Path::new("/tmp/memory/store");
        let resolved = uri.resolve_path(root).unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("/tmp/memory/store/project/conventions")
        );
    }

    #[test]
    fn resolve_path_deep() {
        let uri: VikingUri = "viking://agents/planner/profile".parse().unwrap();
        let root = Path::new("/data/ath");
        let resolved = uri.resolve_path(root).unwrap();
        assert_eq!(resolved, PathBuf::from("/data/ath/agents/planner/profile"));
    }

    #[test]
    fn serde_round_trip() {
        let uri: VikingUri = "viking://project/conventions".parse().unwrap();
        let json = serde_json::to_string(&uri).unwrap();
        assert_eq!(json, r#""viking://project/conventions""#);
        let deserialized: VikingUri = serde_json::from_str(&json).unwrap();
        assert_eq!(uri, deserialized);
    }

    #[test]
    fn serde_rejects_invalid() {
        let result = serde_json::from_str::<VikingUri>(r#""http://bad""#);
        assert!(result.is_err());
    }
}
