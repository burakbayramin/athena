//! Filesystem-backed persistence layer for layered memory content.
//!
//! Each [`VikingUri`] maps to a markdown file under the store root directory.
//! Files use YAML frontmatter for metadata and markdown sections for the three
//! content layers (L0 abstract, L1 overview, L2 detail).
//!
//! Write operations use temp-file + rename for atomicity.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use tracing::instrument;

use crate::error::MemoryError;
use crate::types::LayeredContent;
use crate::uri::VikingUri;

/// Filesystem-backed store for [`LayeredContent`].
///
/// Each URI is stored as a human-readable markdown file with YAML frontmatter.
#[derive(Debug)]
pub struct VikingStore {
    root: PathBuf,
}

impl VikingStore {
    /// Creates a new store rooted at the given directory.
    ///
    /// Creates the directory (and parents) if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, MemoryError> {
        let root = root.into();
        if !root.exists() {
            std::fs::create_dir_all(&root).map_err(|e| MemoryError::IoError {
                path: root.display().to_string(),
                message: "failed to create store root directory".to_string(),
                source: e,
            })?;
        }
        Ok(Self { root })
    }

    /// Returns the store root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes content to the store, creating or overwriting the backing file.
    ///
    /// Parent directories are created as needed. The write uses a temp file
    /// followed by a rename for atomicity.
    #[instrument(skip(self, content), fields(uri = %content.uri))]
    pub fn write(&self, content: &LayeredContent) -> Result<(), MemoryError> {
        let file_path = self.file_path(&content.uri)?;

        // Ensure parent directory exists.
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| MemoryError::IoError {
                    path: parent.display().to_string(),
                    message: "failed to create parent directory".to_string(),
                    source: e,
                })?;
            }
        }

        let markdown = serialize_content(content);

        // Write to temp file in the same directory, then rename for atomicity.
        let temp_path = file_path.with_extension("md.tmp");
        std::fs::write(&temp_path, &markdown).map_err(|e| MemoryError::IoError {
            path: temp_path.display().to_string(),
            message: "failed to write temp file".to_string(),
            source: e,
        })?;

        // On Windows, rename fails if the target exists — remove it first.
        if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| MemoryError::IoError {
                path: file_path.display().to_string(),
                message: "failed to remove existing file before rename".to_string(),
                source: e,
            })?;
        }

        std::fs::rename(&temp_path, &file_path).map_err(|e| MemoryError::IoError {
            path: file_path.display().to_string(),
            message: "failed to rename temp file to final path".to_string(),
            source: e,
        })?;

        tracing::debug!(path = %file_path.display(), "wrote store entry");
        Ok(())
    }

    /// Reads content from the store by URI.
    ///
    /// Returns `Ok(None)` if no file exists for the given URI.
    #[instrument(skip(self), fields(uri = %uri))]
    pub fn read(&self, uri: &VikingUri) -> Result<Option<LayeredContent>, MemoryError> {
        let file_path = self.file_path(uri)?;

        if !file_path.exists() {
            tracing::debug!(path = %file_path.display(), "store entry not found");
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&file_path).map_err(|e| MemoryError::IoError {
            path: file_path.display().to_string(),
            message: "failed to read store file".to_string(),
            source: e,
        })?;

        let content = deserialize_content(&raw, uri)?;
        tracing::debug!(path = %file_path.display(), "read store entry");
        Ok(Some(content))
    }

    /// Deletes the backing file for the given URI.
    ///
    /// Returns `true` if the file existed and was deleted, `false` if it
    /// didn't exist.
    #[instrument(skip(self), fields(uri = %uri))]
    pub fn delete(&self, uri: &VikingUri) -> Result<bool, MemoryError> {
        let file_path = self.file_path(uri)?;

        if !file_path.exists() {
            tracing::debug!(path = %file_path.display(), "nothing to delete");
            return Ok(false);
        }

        std::fs::remove_file(&file_path).map_err(|e| MemoryError::IoError {
            path: file_path.display().to_string(),
            message: "failed to delete store file".to_string(),
            source: e,
        })?;

        tracing::debug!(path = %file_path.display(), "deleted store entry");
        Ok(true)
    }

    /// Lists all URIs stored in the store directory.
    ///
    /// Walks the directory tree and reconstructs URIs from the relative
    /// file paths (stripping the `.md` extension).
    #[instrument(skip(self))]
    pub fn list(&self) -> Result<Vec<VikingUri>, MemoryError> {
        let mut uris = Vec::new();
        self.walk_dir(&self.root, &mut uris)?;
        uris.sort_by_key(|uri| uri.to_string());
        tracing::debug!(count = uris.len(), "listed store entries");
        Ok(uris)
    }

    /// Resolves a URI to its backing `.md` file path.
    fn file_path(&self, uri: &VikingUri) -> Result<PathBuf, MemoryError> {
        let base = uri.resolve_path(&self.root)?;
        Ok(base.with_extension("md"))
    }

    /// Recursively walks a directory, collecting URIs from `.md` files.
    fn walk_dir(&self, dir: &Path, uris: &mut Vec<VikingUri>) -> Result<(), MemoryError> {
        if !dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir).map_err(|e| MemoryError::IoError {
            path: dir.display().to_string(),
            message: "failed to read store directory".to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| MemoryError::IoError {
                path: dir.display().to_string(),
                message: "failed to read directory entry".to_string(),
                source: e,
            })?;
            let path = entry.path();

            if path.is_dir() {
                self.walk_dir(&path, uris)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                // Skip temp files.
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".md.tmp"))
                {
                    continue;
                }

                if let Some(uri) = self.path_to_uri(&path) {
                    uris.push(uri);
                }
            }
        }

        Ok(())
    }

    /// Converts a filesystem path back to a `VikingUri`.
    fn path_to_uri(&self, path: &Path) -> Option<VikingUri> {
        let relative = path.strip_prefix(&self.root).ok()?;

        // Strip the .md extension from the final component.
        let stem = relative.with_extension("");
        let segments: Vec<&str> = stem.iter().filter_map(|s| s.to_str()).collect();

        if segments.is_empty() {
            return None;
        }

        let uri_str = format!("viking://{}", segments.join("/"));
        uri_str.parse().ok()
    }
}

// ── Markdown serialization ──────────────────────────────────────────────────

/// Serialize `LayeredContent` to human-readable markdown with YAML frontmatter.
fn serialize_content(content: &LayeredContent) -> String {
    let mut out = String::new();

    // YAML frontmatter
    out.push_str("---\n");
    out.push_str(&format!("uri: {}\n", content.uri));
    out.push_str(&format!(
        "created_at: {}\n",
        content.created_at.to_rfc3339()
    ));
    out.push_str(&format!(
        "updated_at: {}\n",
        content.updated_at.to_rfc3339()
    ));
    out.push_str("---\n\n");

    // L0: Abstract
    out.push_str("## Abstract\n\n");
    out.push_str(&content.abstract_text);
    out.push('\n');

    // L1: Overview
    out.push_str("\n## Overview\n\n");
    out.push_str(&content.overview_text);
    out.push('\n');

    // L2: Detail (only if present)
    if let Some(ref detail) = content.detail {
        out.push_str("\n## Detail\n\n");
        out.push_str(detail);
        out.push('\n');
    }

    out
}

/// Deserialize markdown with YAML frontmatter back into `LayeredContent`.
fn deserialize_content(raw: &str, uri: &VikingUri) -> Result<LayeredContent, MemoryError> {
    let (frontmatter, body) = parse_frontmatter(raw)?;

    let stored_uri = frontmatter
        .get("uri")
        .ok_or_else(|| MemoryError::SerializationError {
            message: "missing 'uri' in frontmatter".to_string(),
            source: None,
        })?;

    let parsed_uri: VikingUri =
        stored_uri
            .parse()
            .map_err(|e: MemoryError| MemoryError::SerializationError {
                message: format!("invalid uri in frontmatter: {e}"),
                source: None,
            })?;

    // Sanity check: stored URI should match the one we're reading.
    if &parsed_uri != uri {
        return Err(MemoryError::SerializationError {
            message: format!(
                "URI mismatch: file contains '{}' but was read as '{}'",
                parsed_uri, uri
            ),
            source: None,
        });
    }

    let created_at = parse_timestamp(frontmatter.get("created_at"), "created_at")?;
    let updated_at = parse_timestamp(frontmatter.get("updated_at"), "updated_at")?;

    let sections = parse_sections(&body);

    let abstract_text = sections.get("abstract").cloned().unwrap_or_default();
    let overview_text = sections.get("overview").cloned().unwrap_or_default();
    let detail = sections.get("detail").cloned();

    Ok(LayeredContent {
        uri: parsed_uri,
        abstract_text,
        overview_text,
        detail,
        created_at,
        updated_at,
    })
}

/// Parse YAML-style frontmatter between `---` fences.
///
/// Returns (key-value map, remaining body text).
fn parse_frontmatter(
    raw: &str,
) -> Result<(std::collections::HashMap<String, String>, String), MemoryError> {
    let trimmed = raw.trim_start();

    if !trimmed.starts_with("---") {
        return Err(MemoryError::SerializationError {
            message: "expected YAML frontmatter starting with '---'".to_string(),
            source: None,
        });
    }

    // Find the closing ---
    let after_open = &trimmed[3..].trim_start_matches(['\r', '\n']);
    let close_pos = after_open
        .find("\n---")
        .or_else(|| after_open.find("\r\n---"));

    let (fm_text, body) = match close_pos {
        Some(pos) => {
            let fm = &after_open[..pos];
            // Skip past the closing --- and any trailing newline.
            let rest_start = pos + 4; // "\n---"
            let rest = if rest_start < after_open.len() {
                after_open[rest_start..].trim_start_matches(['\r', '\n'])
            } else {
                ""
            };
            (fm, rest.to_string())
        }
        None => {
            return Err(MemoryError::SerializationError {
                message: "unterminated YAML frontmatter — missing closing '---'".to_string(),
                source: None,
            });
        }
    };

    let mut map = std::collections::HashMap::new();
    for line in fm_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    Ok((map, body))
}

/// Parse a timestamp string from frontmatter.
fn parse_timestamp(value: Option<&String>, field_name: &str) -> Result<DateTime<Utc>, MemoryError> {
    let s = value.ok_or_else(|| MemoryError::SerializationError {
        message: format!("missing '{field_name}' in frontmatter"),
        source: None,
    })?;

    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| MemoryError::SerializationError {
            message: format!("invalid timestamp for '{field_name}': {s}"),
            source: None,
        })
}

/// Known section headings used by the store format.
const KNOWN_SECTIONS: &[&str] = &["abstract", "overview", "detail"];

/// Check if a `## ` line is one of our known store section headings.
fn is_store_heading(line: &str) -> Option<&'static str> {
    if let Some(heading) = line.strip_prefix("## ") {
        let lower = heading.trim().to_lowercase();
        KNOWN_SECTIONS.iter().find(|&&s| s == lower).copied()
    } else {
        None
    }
}

/// Parse markdown body into named sections keyed by known heading names.
///
/// Only splits on the known headings (`## Abstract`, `## Overview`,
/// `## Detail`). Other `## ` lines within content are preserved as-is.
fn parse_sections(body: &str) -> std::collections::HashMap<String, String> {
    let mut sections = std::collections::HashMap::new();
    let mut current_heading: Option<&str> = None;
    let mut current_body = String::new();

    for line in body.lines() {
        if let Some(heading) = is_store_heading(line) {
            // Flush previous section.
            if let Some(h) = current_heading {
                let trimmed = current_body.trim().to_string();
                if !trimmed.is_empty() {
                    sections.insert(h.to_string(), trimmed);
                }
            }
            current_heading = Some(heading);
            current_body.clear();
        } else if current_heading.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    // Flush last section.
    if let Some(h) = current_heading {
        let trimmed = current_body.trim().to_string();
        if !trimmed.is_empty() {
            sections.insert(h.to_string(), trimmed);
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_content() -> LayeredContent {
        let uri: VikingUri = "viking://project/conventions".parse().unwrap();
        LayeredContent::new(
            uri,
            "Project follows hexagonal architecture.".to_string(),
            "## Conventions\n- Hexagonal architecture\n- Error types use hint() method".to_string(),
        )
        .with_detail(
            "Full detail about project conventions including examples and rationale.".to_string(),
        )
    }

    fn sample_content_no_detail() -> LayeredContent {
        let uri: VikingUri = "viking://small/entry".parse().unwrap();
        LayeredContent::new(
            uri,
            "A small entry with no detail layer.".to_string(),
            "Overview of the small entry.".to_string(),
        )
    }

    #[test]
    fn store_write_read_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content();
        store.write(&content).unwrap();

        let loaded = store
            .read(&content.uri)
            .unwrap()
            .expect("should find stored content");

        assert_eq!(loaded.uri, content.uri);
        assert_eq!(loaded.abstract_text, content.abstract_text);
        assert_eq!(loaded.overview_text, content.overview_text);
        assert_eq!(loaded.detail, content.detail);
        assert_eq!(loaded.created_at, content.created_at);
        assert_eq!(loaded.updated_at, content.updated_at);
    }

    #[test]
    fn store_round_trip_no_detail() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content_no_detail();
        store.write(&content).unwrap();

        let loaded = store
            .read(&content.uri)
            .unwrap()
            .expect("should find stored content");

        assert_eq!(loaded.detail, None);
        assert_eq!(loaded.abstract_text, content.abstract_text);
        assert_eq!(loaded.overview_text, content.overview_text);
    }

    #[test]
    fn store_overwrite_replaces_content() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let original = sample_content();
        store.write(&original).unwrap();

        // Create updated content with same URI.
        let mut updated = LayeredContent::new(
            original.uri.clone(),
            "Updated abstract.".to_string(),
            "Updated overview.".to_string(),
        );
        updated.detail = Some("Updated detail.".to_string());

        store.write(&updated).unwrap();

        let loaded = store
            .read(&original.uri)
            .unwrap()
            .expect("should find content");

        assert_eq!(loaded.abstract_text, "Updated abstract.");
        assert_eq!(loaded.overview_text, "Updated overview.");
        assert_eq!(loaded.detail, Some("Updated detail.".to_string()));
    }

    #[test]
    fn store_delete_removes_file() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content();
        store.write(&content).unwrap();

        let deleted = store.delete(&content.uri).unwrap();
        assert!(deleted, "should return true when file existed");

        let loaded = store.read(&content.uri).unwrap();
        assert!(loaded.is_none(), "should be gone after delete");
    }

    #[test]
    fn store_delete_nonexistent_returns_false() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let uri: VikingUri = "viking://does/not/exist".parse().unwrap();
        let deleted = store.delete(&uri).unwrap();
        assert!(!deleted, "should return false for nonexistent file");
    }

    #[test]
    fn store_read_nonexistent_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let uri: VikingUri = "viking://missing/entry".parse().unwrap();
        let result = store.read(&uri).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn store_list_returns_all_entries() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let c1 = sample_content();
        let c2 = sample_content_no_detail();
        let c3 = {
            let uri: VikingUri = "viking://agents/planner/profile".parse().unwrap();
            LayeredContent::new(uri, "Planner agent.".into(), "Plans things.".into())
        };

        store.write(&c1).unwrap();
        store.write(&c2).unwrap();
        store.write(&c3).unwrap();

        let uris = store.list().unwrap();
        assert_eq!(uris.len(), 3);

        let uri_strings: Vec<String> = uris.iter().map(|u| u.to_string()).collect();
        assert!(uri_strings.contains(&"viking://project/conventions".to_string()));
        assert!(uri_strings.contains(&"viking://small/entry".to_string()));
        assert!(uri_strings.contains(&"viking://agents/planner/profile".to_string()));
    }

    #[test]
    fn store_list_empty() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let uris = store.list().unwrap();
        assert!(uris.is_empty());
    }

    #[test]
    fn store_creates_root_directory() {
        let dir = TempDir::new().unwrap();
        let store_path = dir.path().join("deeply/nested/store");
        assert!(!store_path.exists());

        let store = VikingStore::new(&store_path).unwrap();
        assert!(store_path.exists());
        assert_eq!(store.root(), store_path);
    }

    #[test]
    fn store_creates_parent_dirs_on_write() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        // URI with nested segments — parent dirs don't exist yet.
        let uri: VikingUri = "viking://deep/nested/entry".parse().unwrap();
        let content = LayeredContent::new(uri, "Deep.".into(), "Nested.".into());
        store.write(&content).unwrap();

        let loaded = store.read(&content.uri).unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn store_written_file_is_human_readable_markdown() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content();
        store.write(&content).unwrap();

        // Read the raw file and verify it's readable markdown.
        let file_path = dir.path().join("store/project/conventions.md");
        let raw = std::fs::read_to_string(&file_path).unwrap();

        assert!(raw.starts_with("---\n"), "should start with frontmatter");
        assert!(raw.contains("uri: viking://project/conventions"));
        assert!(raw.contains("created_at:"));
        assert!(raw.contains("updated_at:"));
        assert!(raw.contains("## Abstract"));
        assert!(raw.contains("## Overview"));
        assert!(raw.contains("## Detail"));
        assert!(raw.contains("Project follows hexagonal architecture."));
    }

    #[test]
    fn store_written_file_omits_detail_when_none() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content_no_detail();
        store.write(&content).unwrap();

        let file_path = dir.path().join("store/small/entry.md");
        let raw = std::fs::read_to_string(&file_path).unwrap();

        assert!(raw.contains("## Abstract"));
        assert!(raw.contains("## Overview"));
        assert!(
            !raw.contains("## Detail"),
            "should omit Detail section when None"
        );
    }

    #[test]
    fn store_write_uses_temp_file() {
        // Verify atomicity by checking no .tmp files remain after write.
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();

        let content = sample_content();
        store.write(&content).unwrap();

        // Walk directory — no .tmp files should exist.
        let has_tmp = walkdir_has_extension(dir.path(), "tmp");
        assert!(!has_tmp, "no temp files should remain after write");
    }

    #[test]
    fn store_path_traversal_blocked() {
        // VikingUri rejects ".." at parse time, so we can't even construct
        // a traversal URI. This test confirms that layer of protection.
        let result = "viking://project/../etc/passwd".parse::<VikingUri>();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MemoryError::PathTraversal { .. }
        ));
    }

    /// Helper: check if any file in the tree has the given extension.
    fn walkdir_has_extension(root: &Path, ext: &str) -> bool {
        fn walk(dir: &Path, ext: &str) -> bool {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if walk(&path, ext) {
                            return true;
                        }
                    } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                        return true;
                    }
                }
            }
            false
        }
        walk(root, ext)
    }
}
