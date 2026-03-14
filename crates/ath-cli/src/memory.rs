//! `ath memory` subcommands for inspecting and managing the Viking memory store.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::{Context, Result};
use ath_memory::{KeywordIndex, LayeredContent, VikingStore, VikingUri};
use clap::{Args, Subcommand};

use crate::GlobalArgs;

/// Memory store inspection and management.
#[derive(Debug, Args)]
#[command(about = "Inspect and manage the Viking memory store")]
pub(crate) struct MemoryArgs {
    #[command(subcommand)]
    pub(crate) command: MemoryCommands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum MemoryCommands {
    /// Show the store contents as a tree.
    Tree,
    /// Search the keyword index.
    Search {
        /// Search query string.
        query: String,
        /// Maximum number of results to return.
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// Read and display a specific memory entry.
    Read {
        /// Viking URI to read (e.g. viking://project/conventions).
        uri: String,
    },
    /// Add a new entry to the store and keyword index.
    Add {
        /// Viking URI for the new entry (e.g. viking://project/notes).
        uri: String,
        /// Content text (stored as abstract).
        content: String,
    },
    /// Show memory store statistics.
    Stats,
    /// Garbage-collect old observation files.
    Gc {
        /// Delete observation files older than this many days.
        #[arg(long, default_value_t = 30)]
        days: u32,
    },
}

/// Entry point dispatched from main.rs.
pub(crate) fn memory_command(args: MemoryArgs, _global: GlobalArgs) -> Result<()> {
    let project_dir = std::env::current_dir().context("failed to determine current directory")?;
    let memory_dir = project_dir.join(".ath").join("memory");

    match args.command {
        MemoryCommands::Tree => cmd_tree(&memory_dir),
        MemoryCommands::Search { query, top } => cmd_search(&memory_dir, &query, top),
        MemoryCommands::Read { uri } => cmd_read(&memory_dir, &uri),
        MemoryCommands::Add { uri, content } => cmd_add(&memory_dir, &uri, &content),
        MemoryCommands::Stats => cmd_stats(&memory_dir),
        MemoryCommands::Gc { days } => cmd_gc(&memory_dir, days),
    }
}

// ── Subcommand implementations ──────────────────────────────────────────────

/// Formats the store listing as an indented tree with box-drawing characters.
fn cmd_tree(memory_dir: &Path) -> Result<()> {
    let store = open_store(memory_dir)?;
    let uris = store.list().context("failed to list store entries")?;

    if uris.is_empty() {
        println!("(empty store)");
        return Ok(());
    }

    let output = format_tree(&uris);
    print!("{output}");
    Ok(())
}

/// Searches the keyword index and displays results with scores and abstracts.
fn cmd_search(memory_dir: &Path, query: &str, top: usize) -> Result<()> {
    let index_path = memory_dir.join("index").join("keyword.json");

    let index = if index_path.exists() {
        KeywordIndex::load(&index_path).context("failed to load keyword index")?
    } else {
        println!("(no keyword index found)");
        return Ok(());
    };

    let hits = index.search(query, top);
    if hits.is_empty() {
        println!("No results for '{query}'");
        return Ok(());
    }

    let store = open_store(memory_dir)?;
    let output = format_search_results(&store, &hits);
    print!("{output}");
    Ok(())
}

/// Reads and displays a single memory entry with all layers.
fn cmd_read(memory_dir: &Path, uri_str: &str) -> Result<()> {
    let uri = VikingUri::from_str(uri_str).context("invalid URI")?;
    let store = open_store(memory_dir)?;

    let content = store
        .read(&uri)
        .context("failed to read store entry")?
        .ok_or_else(|| anyhow::anyhow!("entry not found: {uri_str}"))?;

    let output = format_entry(&content);
    print!("{output}");
    Ok(())
}

/// Adds a new entry to both the store and keyword index.
fn cmd_add(memory_dir: &Path, uri_str: &str, content_text: &str) -> Result<()> {
    let uri = VikingUri::from_str(uri_str).context("invalid URI")?;
    let store = open_store(memory_dir)?;

    let content = LayeredContent::new(uri, content_text.to_string(), String::new());
    store
        .write(&content)
        .context("failed to write store entry")?;

    // Update keyword index.
    let index_path = memory_dir.join("index").join("keyword.json");
    let mut index = if index_path.exists() {
        KeywordIndex::load(&index_path).unwrap_or_else(|_| KeywordIndex::new())
    } else {
        KeywordIndex::new()
    };

    index.add(uri_str, content_text);
    index
        .save(&index_path)
        .context("failed to save keyword index")?;

    println!("Added {uri_str}");
    Ok(())
}

/// Shows memory store statistics: entry count, index size, observations, disk usage.
fn cmd_stats(memory_dir: &Path) -> Result<()> {
    // Store entry count.
    let store_dir = memory_dir.join("store");
    let entry_count = if store_dir.exists() {
        let store = VikingStore::new(&store_dir).context("failed to open store")?;
        store.list().context("failed to list store entries")?.len()
    } else {
        0
    };

    // Keyword index size.
    let index_path = memory_dir.join("index").join("keyword.json");
    let index_entries = if index_path.exists() {
        KeywordIndex::load(&index_path)
            .map(|i| i.len())
            .unwrap_or(0)
    } else {
        0
    };

    // Observation file count.
    let obs_dir = memory_dir.join("observations");
    let obs_count = count_jsonl_files(&obs_dir);

    // Disk usage across all of .ath/memory/.
    let disk_bytes = dir_disk_usage(memory_dir);

    let output = format_stats(entry_count, index_entries, obs_count, disk_bytes);
    print!("{output}");
    Ok(())
}

/// Deletes observation .jsonl files older than `days` days.
fn cmd_gc(memory_dir: &Path, days: u32) -> Result<()> {
    let obs_dir = memory_dir.join("observations");

    if !obs_dir.exists() {
        println!("No observations to clean");
        return Ok(());
    }

    let threshold = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(u64::from(days) * 86400);

    let entries = std::fs::read_dir(&obs_dir).context("failed to read observations directory")?;

    let mut deleted = 0u32;
    let mut bytes_freed = 0u64;

    for entry in entries {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();

        // Only process .jsonl files.
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let meta = std::fs::metadata(&path)
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;

        let modified = meta
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if modified < threshold {
            let size = meta.len();
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to delete {}", path.display()))?;
            deleted += 1;
            bytes_freed += size;
        }
    }

    println!(
        "GC complete: {} file(s) deleted, {} freed",
        deleted,
        format_bytes(bytes_freed)
    );
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn open_store(memory_dir: &Path) -> Result<VikingStore> {
    let store_dir = memory_dir.join("store");
    VikingStore::new(store_dir).context("failed to open store")
}

/// Build a tree view from a sorted list of URIs, grouping by path prefix.
fn format_tree(uris: &[VikingUri]) -> String {
    // Build a nested tree structure: BTreeMap<segment, children>
    let mut root: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for uri in uris {
        let segments = uri.segments();
        if segments.is_empty() {
            continue;
        }
        let top = &segments[0];
        let rest = if segments.len() > 1 {
            segments[1..].join("/")
        } else {
            String::new()
        };
        root.entry(top.clone()).or_default().push(rest);
    }

    let mut out = String::new();
    out.push_str(&format!("memory ({} entries)\n", uris.len()));

    let keys: Vec<&String> = root.keys().collect();
    for (i, key) in keys.iter().enumerate() {
        let is_last_top = i == keys.len() - 1;
        let connector = if is_last_top {
            "└── "
        } else {
            "├── "
        };
        let children = &root[*key];
        let leaf_count = children.len();

        out.push_str(&format!("{connector}{key}/ ({leaf_count})\n"));

        let indent = if is_last_top { "    " } else { "│   " };
        for (j, child) in children.iter().enumerate() {
            if child.is_empty() {
                continue; // single-segment URI already shown as the directory
            }
            let is_last_child = j == children.len() - 1;
            let child_connector = if is_last_child {
                "└── "
            } else {
                "├── "
            };
            out.push_str(&format!("{indent}{child_connector}{child}\n"));
        }
    }

    out
}

/// Format search hits with scores and abstracts.
fn format_search_results(store: &VikingStore, hits: &[ath_memory::KeywordHit]) -> String {
    let mut out = String::new();
    for hit in hits {
        let abstract_text = match VikingUri::from_str(&hit.uri_str) {
            Ok(uri) => match store.read(&uri) {
                Ok(Some(content)) => {
                    if content.abstract_text.is_empty() {
                        "(no abstract)".to_string()
                    } else {
                        content.abstract_text.clone()
                    }
                }
                _ => "(read error)".to_string(),
            },
            Err(_) => "(invalid URI)".to_string(),
        };
        out.push_str(&format!(
            "[{:.2}] {} — {}\n",
            hit.score, hit.uri_str, abstract_text
        ));
    }
    out
}

/// Format a single entry showing all content layers.
fn format_entry(content: &LayeredContent) -> String {
    let mut out = String::new();
    out.push_str(&format!("URI: {}\n", content.uri));
    out.push_str(&format!(
        "Created: {}\n",
        content.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));
    out.push_str(&format!(
        "Updated: {}\n\n",
        content.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    if !content.abstract_text.is_empty() {
        out.push_str("## Abstract\n\n");
        out.push_str(&content.abstract_text);
        out.push_str("\n\n");
    }

    if !content.overview_text.is_empty() {
        out.push_str("## Overview\n\n");
        out.push_str(&content.overview_text);
        out.push_str("\n\n");
    }

    if let Some(ref detail) = content.detail {
        if !detail.is_empty() {
            out.push_str("## Detail\n\n");
            out.push_str(detail);
            out.push('\n');
        }
    }

    out
}

/// Count `.jsonl` files in a directory (non-recursive).
fn count_jsonl_files(dir: &Path) -> usize {
    if !dir.exists() {
        return 0;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
                .count()
        })
        .unwrap_or(0)
}

/// Recursively sum file sizes under a directory.
fn dir_disk_usage(dir: &Path) -> u64 {
    if !dir.exists() {
        return 0;
    }
    fn walk(dir: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    total += walk(&path);
                } else if let Ok(meta) = std::fs::metadata(&path) {
                    total += meta.len();
                }
            }
        }
        total
    }
    walk(dir)
}

/// Format a stats summary.
fn format_stats(
    entries: usize,
    index_entries: usize,
    observations: usize,
    disk_bytes: u64,
) -> String {
    format!(
        "Store entries:  {entries}\n\
         Index entries:  {index_entries}\n\
         Observations:   {observations}\n\
         Disk usage:     {}\n",
        format_bytes(disk_bytes)
    )
}

/// Human-readable byte size formatting.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Set up a temp store with test data.
    fn setup_populated_store() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        let store_dir = memory_dir.join("store");
        std::fs::create_dir_all(&store_dir).unwrap();

        let store = VikingStore::new(&store_dir).unwrap();

        // Write test entries.
        let entries = vec![
            (
                "viking://project/conventions",
                "Uses hexagonal architecture.",
                "Detailed overview of conventions.",
            ),
            (
                "viking://project/identity",
                "A Rust CLI orchestrator.",
                "Project identity overview.",
            ),
            (
                "viking://agents/claude/profile",
                "Claude prefers explicit errors.",
                "Agent profile for Claude.",
            ),
        ];

        let mut index = KeywordIndex::new();
        for (uri_str, abstract_text, overview_text) in &entries {
            let uri = VikingUri::from_str(uri_str).unwrap();
            let content =
                LayeredContent::new(uri, abstract_text.to_string(), overview_text.to_string());
            store.write(&content).unwrap();
            index.add(uri_str, &format!("{abstract_text} {overview_text}"));
        }

        // Save keyword index.
        let index_dir = memory_dir.join("index");
        std::fs::create_dir_all(&index_dir).unwrap();
        index.save(&index_dir.join("keyword.json")).unwrap();

        (dir, memory_dir)
    }

    #[test]
    fn memory_tree_shows_entries() {
        let (_dir, memory_dir) = setup_populated_store();
        let store = open_store(&memory_dir).unwrap();
        let uris = store.list().unwrap();
        let output = format_tree(&uris);

        assert!(output.contains("memory (3 entries)"));
        assert!(output.contains("agents/"));
        assert!(output.contains("project/"));
        assert!(output.contains("conventions"));
        assert!(output.contains("identity"));
        // Box-drawing characters present
        assert!(output.contains("├── ") || output.contains("└── "));
    }

    #[test]
    fn memory_tree_empty_store() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        let store = VikingStore::new(memory_dir.join("store")).unwrap();
        let uris = store.list().unwrap();
        assert!(uris.is_empty());
    }

    #[test]
    fn memory_search_finds_results() {
        let (_dir, memory_dir) = setup_populated_store();
        let index_path = memory_dir.join("index").join("keyword.json");
        let index = KeywordIndex::load(&index_path).unwrap();
        let hits = index.search("hexagonal architecture", 10);

        assert!(!hits.is_empty());
        assert_eq!(hits[0].uri_str, "viking://project/conventions");

        let store = open_store(&memory_dir).unwrap();
        let output = format_search_results(&store, &hits);
        assert!(output.contains("viking://project/conventions"));
        assert!(output.contains("hexagonal"));
    }

    #[test]
    fn memory_search_no_results() {
        let (_dir, memory_dir) = setup_populated_store();
        let index_path = memory_dir.join("index").join("keyword.json");
        let index = KeywordIndex::load(&index_path).unwrap();
        let hits = index.search("zzzznonexistent", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn memory_search_missing_index() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        let index_path = memory_dir.join("index").join("keyword.json");
        assert!(!index_path.exists());
    }

    #[test]
    fn memory_read_displays_content() {
        let (_dir, memory_dir) = setup_populated_store();
        let store = open_store(&memory_dir).unwrap();
        let uri = VikingUri::from_str("viking://project/conventions").unwrap();
        let content = store.read(&uri).unwrap().unwrap();
        let output = format_entry(&content);

        assert!(output.contains("URI: viking://project/conventions"));
        assert!(output.contains("## Abstract"));
        assert!(output.contains("hexagonal architecture"));
        assert!(output.contains("## Overview"));
        assert!(output.contains("Detailed overview"));
    }

    #[test]
    fn memory_read_nonexistent_returns_none() {
        let (_dir, memory_dir) = setup_populated_store();
        let store = open_store(&memory_dir).unwrap();
        let uri = VikingUri::from_str("viking://does/not/exist").unwrap();
        let result = store.read(&uri).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn memory_add_writes_to_store_and_index() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        std::fs::create_dir_all(&memory_dir).unwrap();

        let uri_str = "viking://test/add";
        let content_text = "This is a test entry about testing.";

        // Add the entry.
        cmd_add(&memory_dir, uri_str, content_text).unwrap();

        // Verify store contains it.
        let store = open_store(&memory_dir).unwrap();
        let uri = VikingUri::from_str(uri_str).unwrap();
        let entry = store.read(&uri).unwrap().expect("entry should exist");
        assert_eq!(entry.abstract_text, content_text);
        assert!(entry.overview_text.is_empty());

        // Verify keyword index contains it.
        let index_path = memory_dir.join("index").join("keyword.json");
        let index = KeywordIndex::load(&index_path).unwrap();
        let hits = index.search("testing", 5);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].uri_str, uri_str);
    }

    #[test]
    fn memory_add_updates_existing_index() {
        let (_dir, memory_dir) = setup_populated_store();

        // Add a new entry to the already-populated store/index.
        cmd_add(
            &memory_dir,
            "viking://new/entry",
            "Brand new content about databases",
        )
        .unwrap();

        // Verify the new entry is searchable.
        let index_path = memory_dir.join("index").join("keyword.json");
        let index = KeywordIndex::load(&index_path).unwrap();
        let hits = index.search("databases", 5);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].uri_str, "viking://new/entry");

        // Verify existing entries are still in the index.
        let hits = index.search("hexagonal", 5);
        assert!(!hits.is_empty());
    }

    #[test]
    fn memory_tree_format_structure() {
        // Build a specific set of URIs to test tree formatting.
        let uris: Vec<VikingUri> = vec![
            "viking://agents/claude/profile",
            "viking://agents/gemini/profile",
            "viking://project/conventions",
            "viking://project/identity",
        ]
        .into_iter()
        .map(|s| VikingUri::from_str(s).unwrap())
        .collect();

        let output = format_tree(&uris);
        assert!(output.contains("memory (4 entries)"));
        // agents/ should come before project/ (BTreeMap ordering)
        let agents_pos = output.find("agents/").unwrap();
        let project_pos = output.find("project/").unwrap();
        assert!(agents_pos < project_pos);
        // Check entry counts
        assert!(output.contains("agents/ (2)"));
        assert!(output.contains("project/ (2)"));
    }

    #[test]
    fn memory_read_entry_with_detail() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        let store = VikingStore::new(memory_dir.join("store")).unwrap();

        let uri = VikingUri::from_str("viking://detailed/entry").unwrap();
        let content = LayeredContent::new(
            uri.clone(),
            "Abstract text.".to_string(),
            "Overview text.".to_string(),
        )
        .with_detail("Full detail text with examples.".to_string());
        store.write(&content).unwrap();

        let loaded = store.read(&uri).unwrap().unwrap();
        let output = format_entry(&loaded);

        assert!(output.contains("## Abstract"));
        assert!(output.contains("Abstract text."));
        assert!(output.contains("## Overview"));
        assert!(output.contains("Overview text."));
        assert!(output.contains("## Detail"));
        assert!(output.contains("Full detail text"));
    }

    // ── Stats tests ─────────────────────────────────────────────────────

    /// Set up a fully populated memory dir with store, index, and observations.
    fn setup_full_memory_dir() -> (TempDir, PathBuf) {
        let (dir, memory_dir) = setup_populated_store();

        // Create observation files.
        let obs_dir = memory_dir.join("observations");
        std::fs::create_dir_all(&obs_dir).unwrap();
        std::fs::write(obs_dir.join("run-001.jsonl"), "{\"type\":\"obs\"}\n").unwrap();
        std::fs::write(
            obs_dir.join("run-002.jsonl"),
            "{\"type\":\"obs\"}\n{\"type\":\"obs2\"}\n",
        )
        .unwrap();

        (dir, memory_dir)
    }

    #[test]
    fn memory_stats_shows_counts() {
        let (_dir, memory_dir) = setup_full_memory_dir();
        let output = capture_stats(&memory_dir);

        assert!(
            output.contains("Store entries:  3"),
            "expected 3 store entries, got: {output}"
        );
        assert!(
            output.contains("Index entries:  3"),
            "expected 3 index entries, got: {output}"
        );
        assert!(
            output.contains("Observations:   2"),
            "expected 2 observations, got: {output}"
        );
        assert!(
            output.contains("Disk usage:"),
            "expected disk usage line, got: {output}"
        );
    }

    #[test]
    fn memory_stats_empty_store() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        std::fs::create_dir_all(&memory_dir).unwrap();

        let output = capture_stats(&memory_dir);
        assert!(output.contains("Store entries:  0"));
        assert!(output.contains("Index entries:  0"));
        assert!(output.contains("Observations:   0"));
        assert!(output.contains("Disk usage:     0 B"));
    }

    #[test]
    fn memory_stats_nonexistent_dir() {
        let dir = TempDir::new().unwrap();
        let memory_dir = dir.path().join(".ath").join("memory");
        // Don't create the directory — stats should handle gracefully.
        let output = capture_stats(&memory_dir);
        assert!(output.contains("Store entries:  0"));
        assert!(output.contains("Observations:   0"));
    }

    /// Helper: build stats output string without printing.
    fn capture_stats(memory_dir: &Path) -> String {
        let store_dir = memory_dir.join("store");
        let entry_count = if store_dir.exists() {
            let store = VikingStore::new(&store_dir).unwrap();
            store.list().unwrap().len()
        } else {
            0
        };

        let index_path = memory_dir.join("index").join("keyword.json");
        let index_entries = if index_path.exists() {
            KeywordIndex::load(&index_path)
                .map(|i| i.len())
                .unwrap_or(0)
        } else {
            0
        };

        let obs_dir = memory_dir.join("observations");
        let obs_count = count_jsonl_files(&obs_dir);
        let disk_bytes = dir_disk_usage(memory_dir);

        format_stats(entry_count, index_entries, obs_count, disk_bytes)
    }

    // ── GC tests ────────────────────────────────────────────────────────

    #[test]
    fn memory_gc_deletes_old_files() {
        use filetime::{set_file_mtime, FileTime};

        let dir = TempDir::new().unwrap();
        let obs_dir = dir.path().join("observations");
        std::fs::create_dir_all(&obs_dir).unwrap();

        // Create a "recent" file (now).
        let recent = obs_dir.join("recent.jsonl");
        std::fs::write(&recent, "recent data\n").unwrap();

        // Create an "old" file and backdate it 60 days.
        let old = obs_dir.join("old.jsonl");
        std::fs::write(&old, "old data\n").unwrap();
        let sixty_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (60 * 86400);
        set_file_mtime(&old, FileTime::from_unix_time(sixty_days_ago as i64, 0)).unwrap();

        // Run gc with 30-day threshold.
        // We call cmd_gc with the parent as memory_dir (since it looks for observations/ inside).
        cmd_gc(dir.path(), 30).unwrap();

        // Old file should be gone, recent should survive.
        assert!(!old.exists(), "old file should have been deleted");
        assert!(recent.exists(), "recent file should survive");
    }

    #[test]
    fn memory_gc_keeps_recent_files() {
        let dir = TempDir::new().unwrap();
        let obs_dir = dir.path().join("observations");
        std::fs::create_dir_all(&obs_dir).unwrap();

        let recent = obs_dir.join("recent.jsonl");
        std::fs::write(&recent, "data\n").unwrap();

        // GC with 30 days — recent file should survive.
        cmd_gc(dir.path(), 30).unwrap();
        assert!(recent.exists());
    }

    #[test]
    fn memory_gc_missing_observations_dir() {
        let dir = TempDir::new().unwrap();
        // No observations/ directory — should not error.
        let result = cmd_gc(dir.path(), 30);
        assert!(result.is_ok());
    }

    #[test]
    fn memory_gc_ignores_non_jsonl_files() {
        use filetime::{set_file_mtime, FileTime};

        let dir = TempDir::new().unwrap();
        let obs_dir = dir.path().join("observations");
        std::fs::create_dir_all(&obs_dir).unwrap();

        // Create an old .txt file — should be ignored by gc.
        let txt = obs_dir.join("notes.txt");
        std::fs::write(&txt, "notes\n").unwrap();
        let sixty_days_ago = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (60 * 86400);
        set_file_mtime(&txt, FileTime::from_unix_time(sixty_days_ago as i64, 0)).unwrap();

        cmd_gc(dir.path(), 30).unwrap();
        assert!(txt.exists(), "non-jsonl files should not be deleted");
    }

    // ── Format helpers tests ────────────────────────────────────────────

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1572864), "1.5 MB");
    }
}
