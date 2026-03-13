//! Codebase scanner with gitignore-aware traversal and key file extraction.
//!
//! Scans a directory tree respecting .gitignore patterns, skips known
//! non-essential directories, identifies key files (manifests, READMEs,
//! entry points), and extracts their contents with size caps.

use std::path::Path;

use super::error::InputError;

/// Maximum size for a single key file (5KB).
pub const MAX_SINGLE_FILE_SIZE: usize = 5_120;

/// Maximum total key file content budget (30KB).
pub const MAX_TOTAL_KEY_FILE_SIZE: usize = 30_720;

/// Directories that are always skipped during traversal.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    "vendor",
    "__pycache__",
    ".next",
];

/// File names considered "key files" (manifests, READMEs, configs).
const KEY_FILE_NAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "requirements.txt",
    "README.md",
    "README",
    "README.rst",
    ".env.example",
    "docker-compose.yml",
    "Dockerfile",
    "tsconfig.json",
    "rustfmt.toml",
];

/// Relative path patterns considered "key entry points".
const KEY_ENTRY_PATTERNS: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/index.ts",
    "src/index.js",
    "src/app.ts",
    "src/app.js",
    "main.go",
    "app.py",
    "manage.py",
];

/// Check whether a relative path is a key file worth extracting.
///
/// Matches against known file names (e.g., `Cargo.toml`, `README.md`)
/// and known entry point patterns (e.g., `src/main.rs`).
pub fn is_key_file(rel_path: &Path) -> bool {
    // Check filename against KEY_FILE_NAMES
    if let Some(file_name) = rel_path.file_name().and_then(|n| n.to_str()) {
        if KEY_FILE_NAMES.contains(&file_name) {
            return true;
        }
    }

    // Check full relative path (normalized to forward slashes) against KEY_ENTRY_PATTERNS
    let normalized = rel_path.to_string_lossy().replace('\\', "/");
    KEY_ENTRY_PATTERNS.contains(&normalized.as_str())
}

/// Scan a codebase directory, returning a tree string and key file contents.
///
/// Uses the `ignore` crate for gitignore-respecting traversal, skips known
/// directories (node_modules, target, .git, etc.), identifies key files,
/// and reads their contents with per-file and total size caps.
///
/// Returns `(tree_string, key_files)` where key_files is a list of
/// `(relative_path, content)` pairs.
pub fn scan_codebase(path: &Path) -> Result<(String, Vec<(String, String)>), InputError> {
    if !path.exists() || !path.is_dir() {
        return Err(InputError::CodebaseNotFound {
            path: path.to_path_buf(),
            hint: "Check the directory path and try again".into(),
        });
    }

    let walker = ignore::WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .filter_entry(|entry| {
            // Skip known directories by name
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                if let Some(name) = entry.file_name().to_str() {
                    if SKIP_DIRS.contains(&name) {
                        return false;
                    }
                }
            }
            true
        })
        .build();

    let mut tree_lines = Vec::new();
    let mut key_files: Vec<(String, String)> = Vec::new();
    let mut total_key_bytes: usize = 0;

    for entry in walker.flatten() {
        // Compute relative path
        let entry_path = entry.path();
        let rel_path = match entry_path.strip_prefix(path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Skip the root entry itself
        if rel_path.as_os_str().is_empty() {
            continue;
        }

        // Build tree line with indentation
        let depth = entry.depth();
        let indent = "  ".repeat(depth.saturating_sub(1));
        let name = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let suffix = if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            "/"
        } else {
            ""
        };
        tree_lines.push(format!("{}{}{}", indent, name, suffix));

        // Check if this is a key file (only for regular files)
        if entry.file_type().is_some_and(|ft| ft.is_file()) && is_key_file(rel_path)
            && total_key_bytes < MAX_TOTAL_KEY_FILE_SIZE {
                if let Ok(content) = std::fs::read_to_string(entry_path) {
                    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                    let truncated = if content.len() > MAX_SINGLE_FILE_SIZE {
                        let mut t = content[..MAX_SINGLE_FILE_SIZE].to_string();
                        t.push_str("\n[truncated at 5KB]");
                        t
                    } else {
                        content
                    };
                    total_key_bytes += truncated.len();
                    key_files.push((rel_str, truncated));
                }
            }
    }

    let tree_string = tree_lines.join("\n");
    Ok((tree_string, key_files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: create a file with content inside a TempDir.
    fn create_file(dir: &Path, rel_path: &str, content: &str) {
        let full_path = dir.join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("create dirs");
        }
        let mut f = fs::File::create(&full_path).expect("create file");
        f.write_all(content.as_bytes()).expect("write");
    }

    #[test]
    fn scan_codebase_returns_tree_and_key_files() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "Cargo.toml", "[package]\nname = \"test\"");
        create_file(tmp.path(), "src/main.rs", "fn main() {}");

        let (tree, key_files) = scan_codebase(tmp.path()).unwrap();
        assert!(tree.contains("Cargo.toml"));
        assert!(tree.contains("main.rs"));
        assert!(!key_files.is_empty());
    }

    #[test]
    fn scan_codebase_skips_git_directory() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".git/HEAD", "ref: refs/heads/main");
        create_file(tmp.path(), "src/main.rs", "fn main() {}");

        let (tree, _) = scan_codebase(tmp.path()).unwrap();
        assert!(!tree.contains(".git"), "tree should not contain .git: {}", tree);
        assert!(!tree.contains("HEAD"), "tree should not contain HEAD: {}", tree);
    }

    #[test]
    fn scan_codebase_skips_node_modules() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "node_modules/foo/index.js", "module.exports = {}");
        create_file(tmp.path(), "src/main.rs", "fn main() {}");

        let (tree, _) = scan_codebase(tmp.path()).unwrap();
        assert!(!tree.contains("node_modules"), "tree should not contain node_modules: {}", tree);
    }

    #[test]
    fn scan_codebase_identifies_cargo_toml_as_key_file() {
        let tmp = TempDir::new().unwrap();
        let content = "[package]\nname = \"test-project\"";
        create_file(tmp.path(), "Cargo.toml", content);

        let (_, key_files) = scan_codebase(tmp.path()).unwrap();
        let cargo = key_files.iter().find(|(p, _)| p == "Cargo.toml");
        assert!(cargo.is_some(), "Cargo.toml should be identified as key file");
        assert!(cargo.unwrap().1.contains("test-project"));
    }

    #[test]
    fn scan_codebase_identifies_readme_as_key_file() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "README.md", "# My Project");

        let (_, key_files) = scan_codebase(tmp.path()).unwrap();
        let readme = key_files.iter().find(|(p, _)| p == "README.md");
        assert!(readme.is_some(), "README.md should be identified as key file");
    }

    #[test]
    fn scan_codebase_identifies_src_main_rs_as_key_file() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "src/main.rs", "fn main() { println!(\"hello\"); }");

        let (_, key_files) = scan_codebase(tmp.path()).unwrap();
        let main = key_files.iter().find(|(p, _)| p == "src/main.rs");
        assert!(main.is_some(), "src/main.rs should be identified as key file");
    }

    #[test]
    fn scan_codebase_nonexistent_path_returns_error() {
        let result = scan_codebase(Path::new("/tmp/definitely-nonexistent-athena-test-dir"));
        assert!(result.is_err());
        match result.unwrap_err() {
            InputError::CodebaseNotFound { path, .. } => {
                assert!(path.to_string_lossy().contains("definitely-nonexistent"));
            }
            other => panic!("expected CodebaseNotFound, got: {:?}", other),
        }
    }

    #[test]
    fn scan_codebase_truncates_large_key_files() {
        let tmp = TempDir::new().unwrap();
        // Create a key file larger than 5KB
        let big_content = "x".repeat(MAX_SINGLE_FILE_SIZE + 1000);
        create_file(tmp.path(), "README.md", &big_content);

        let (_, key_files) = scan_codebase(tmp.path()).unwrap();
        let readme = key_files.iter().find(|(p, _)| p == "README.md").unwrap();
        assert!(readme.1.contains("[truncated at 5KB]"), "should have truncation marker");
        // Truncated content should be MAX_SINGLE_FILE_SIZE + marker length
        assert!(readme.1.len() < big_content.len(), "truncated should be smaller than original");
    }

    #[test]
    fn scan_codebase_caps_total_key_file_content() {
        let tmp = TempDir::new().unwrap();
        // Create many key files, each ~4KB, totalling well over 30KB
        let content_4k = "y".repeat(4_000);
        create_file(tmp.path(), "Cargo.toml", &content_4k);
        create_file(tmp.path(), "package.json", &content_4k);
        create_file(tmp.path(), "pyproject.toml", &content_4k);
        create_file(tmp.path(), "go.mod", &content_4k);
        create_file(tmp.path(), "pom.xml", &content_4k);
        create_file(tmp.path(), "build.gradle", &content_4k);
        create_file(tmp.path(), "Gemfile", &content_4k);
        create_file(tmp.path(), "requirements.txt", &content_4k);
        create_file(tmp.path(), "README.md", &content_4k);
        create_file(tmp.path(), "Dockerfile", &content_4k);

        let (_, key_files) = scan_codebase(tmp.path()).unwrap();
        let total: usize = key_files.iter().map(|(_, c)| c.len()).sum();
        // We should have read some files but total should be capped around 30KB
        // (might slightly exceed due to the last file added before check)
        assert!(key_files.len() < 10, "should have stopped before reading all key files, got {}", key_files.len());
        // Total should be roughly at or below 2x the budget (budget checked before each add)
        assert!(total < MAX_TOTAL_KEY_FILE_SIZE * 2, "total {} should be reasonable", total);
    }

    #[test]
    fn is_key_file_true_for_known_files() {
        assert!(is_key_file(Path::new("Cargo.toml")));
        assert!(is_key_file(Path::new("package.json")));
        assert!(is_key_file(Path::new("README.md")));
        assert!(is_key_file(Path::new("src/main.rs")));
        assert!(is_key_file(Path::new("src/lib.rs")));
    }

    #[test]
    fn is_key_file_false_for_random_files() {
        assert!(!is_key_file(Path::new("src/utils.rs")));
        assert!(!is_key_file(Path::new("foo.txt")));
        assert!(!is_key_file(Path::new("data/config.yaml")));
    }
}
