---
estimated_steps: 4
estimated_files: 2
---

# T02: Filesystem-backed Viking Store with markdown persistence

**Slice:** S01 — Viking Store & Vector Index
**Milestone:** M002

## Description

Implement `VikingStore` — the filesystem-backed persistence layer for layered memory content. Each `VikingUri` maps to a markdown file under `.ath/memory/store/`. The file format uses markdown sections for L0 (abstract), L1 (overview), and L2 (detail). The store provides CRUD operations with path traversal protection inherited from `VikingUri`.

## Steps

1. Implement `VikingStore` struct in `src/store.rs` with a configurable `root: PathBuf` (defaults to `.ath/memory/store/`). Constructor validates root exists or creates it.
2. Implement core methods: `write(uri, content)` — serialize `LayeredContent` as markdown with `## Abstract`, `## Overview`, `## Detail` sections plus YAML frontmatter for metadata (timestamps, uri). Write to temp file then rename for atomicity. `read(uri) -> Result<Option<LayeredContent>>` — parse markdown back into struct. `delete(uri) -> Result<bool>` — remove file, return whether it existed. `list() -> Result<Vec<VikingUri>>` — walk the store directory and return all stored URIs.
3. Handle edge cases: missing parent directories (create on write), concurrent access (temp-file + rename pattern), Windows rename semantics (remove target first if exists), empty L2 detail (omit section entirely).
4. Write tests using `tempfile::TempDir`: round-trip write→read, overwrite preserves new content, delete removes file, list returns all entries, read of nonexistent URI returns Ok(None), path traversal via VikingUri is blocked at the URI level.

## Must-Haves

- [ ] Write→read round-trip preserves all LayeredContent fields
- [ ] Overwrite replaces content correctly
- [ ] Delete removes the backing file
- [ ] List returns all stored URIs
- [ ] Missing entry returns `Ok(None)`, not an error
- [ ] Markdown format is human-readable (sections with headings, not raw JSON)
- [ ] Write uses temp-file + rename for atomicity

## Verification

- `cargo test -p ath-memory -- store` — all store tests pass
- Manual inspection: written files are valid, human-readable markdown

## Inputs

- `crates/ath-memory/src/uri.rs` — `VikingUri` with `resolve_path()` for mapping URIs to filesystem paths
- `crates/ath-memory/src/types.rs` — `LayeredContent` struct to persist
- `crates/ath-memory/src/error.rs` — `MemoryError` for error reporting

## Observability Impact

- **Tracing spans:** `tracing::instrument` on `write`, `read`, `delete`, `list` methods — logs URI, file path, and operation result at `debug` level
- **Inspection surface:** Stored files at `.ath/memory/store/<segments>.md` are plain markdown, directly inspectable with any text editor or `cat`
- **Failure visibility:** All I/O failures produce `MemoryError::IoError` with the path and operation context; serialization parse failures produce `MemoryError::SerializationError` with the field that failed
- **Diagnostic commands:** `cargo test -p ath-memory -- store` exercises all CRUD paths; inspect any store file with `cat .ath/memory/store/<path>.md`

## Expected Output

- `crates/ath-memory/src/store.rs` — VikingStore implementation with CRUD + tests
- `crates/ath-memory/src/lib.rs` — updated with `pub mod store` and re-exports
