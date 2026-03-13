# S01: Viking Store & Vector Index — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All deliverables are library APIs tested via unit and integration tests — no runtime server, no UI, no user-facing CLI in this slice.

## Preconditions

- Rust toolchain installed (`cargo` available)
- Working directory is the athena project root
- No prior `.ath/memory/` state required (tests use tempdir)

## Smoke Test

```
cargo test -p ath-memory
```
All 54 tests + 1 doc-test pass. If this fails, nothing below will work.

## Test Cases

### 1. VikingUri parses valid URIs correctly

1. Run `cargo test -p ath-memory -- uri::tests::parse_simple_uri uri::tests::parse_deep_uri uri::tests::parse_single_segment`
2. **Expected:** All 3 tests pass. `viking://project/conventions` parses to segments `["project", "conventions"]`. `viking://project/agents/claude/profile` parses to 4 segments. `viking://meta` parses to 1 segment.

### 2. VikingUri rejects invalid URIs

1. Run `cargo test -p ath-memory -- uri::tests::reject`
2. **Expected:** All 5 rejection tests pass:
   - `https://example.com` → rejected (wrong scheme)
   - `viking://foo/../bar` → rejected (traversal)
   - `viking://foo//bar` → rejected (empty segment)
   - `viking://foo/./bar` → rejected (dot segment)
   - `viking://` → rejected (no path)

### 3. VikingUri path traversal protection at resolve time

1. Run `cargo test -p ath-memory -- uri::tests::resolve_path`
2. **Expected:** `viking://project/conventions` resolves to `<root>/project/conventions` within the given root directory. Resolved path does not escape the root.

### 4. Store write→read round-trip preserves content

1. Run `cargo test -p ath-memory -- store::tests::store_write_read_round_trip`
2. **Expected:** Write a `LayeredContent` with L0="abstract text", L1="overview text", L2="detail text" to `viking://test/entry`. Read it back. All three layers match. `created_at` and `updated_at` timestamps are present.

### 5. Store round-trip without detail layer

1. Run `cargo test -p ath-memory -- store::tests::store_round_trip_no_detail`
2. **Expected:** Write `LayeredContent` with L0 and L1 but `detail: None`. Read back. `detail` field is `None`. L0 and L1 match.

### 6. Store overwrite replaces content

1. Run `cargo test -p ath-memory -- store::tests::store_overwrite_replaces_content`
2. **Expected:** Write entry, then write again with different content to same URI. Read returns the second version's content, not the first.

### 7. Store read of nonexistent entry returns None

1. Run `cargo test -p ath-memory -- store::tests::store_read_nonexistent_returns_none`
2. **Expected:** Reading `viking://does/not/exist` returns `Ok(None)`, not an error.

### 8. Store delete removes file and returns true

1. Run `cargo test -p ath-memory -- store::tests::store_delete_removes_file`
2. **Expected:** Write entry, delete it (returns `Ok(true)`), read returns `Ok(None)`.

### 9. Store list returns all entries

1. Run `cargo test -p ath-memory -- store::tests::store_list_returns_all_entries`
2. **Expected:** Write 3 entries with different URIs. `list()` returns exactly 3 `VikingUri` values matching the written URIs.

### 10. Store files are human-readable markdown

1. Run `cargo test -p ath-memory -- store::tests::store_written_file_is_human_readable_markdown`
2. **Expected:** Written file on disk contains `---` YAML frontmatter fences, `## Abstract`, `## Overview`, and `## Detail` section headings with content underneath. File is valid, inspectable markdown.

### 11. HNSW vector index insert and search

1. Run `cargo test -p ath-memory -- index::tests::insert_and_search`
2. **Expected:** Insert 3 entries with distinct vectors. Search with a query vector close to one entry. Top result is the closest entry with score > 0.

### 12. HNSW index persist→reload→search

1. Run `cargo test -p ath-memory -- index::tests::persist_and_reload`
2. **Expected:** Insert entries, save to disk, create new `MemoryIndex::load()` from saved files. Search on reloaded index returns same results as before save.

### 13. Keyword fallback search

1. Run `cargo test -p ath-memory -- keyword::tests::add_and_search`
2. **Expected:** Add entries with text content. Search for a keyword that appears in one entry. Results include that entry with score > 0.

### 14. Keyword index persist→reload→search

1. Run `cargo test -p ath-memory -- keyword::tests::persist_and_reload`
2. **Expected:** Add entries, save to disk, load from disk. Search on reloaded index returns same results.

### 15. Integration: store + vector index + keyword index end-to-end

1. Run `cargo test -p ath-memory -- index::tests::integration_store_and_index_persist_and_search`
2. **Expected:** Create VikingStore, MemoryIndex, and KeywordIndex. Write LayeredContent entries to store, upsert vectors to index, add text to keyword index. Search both indexes. Persist all to disk. Reload all from disk. Search again. Results are consistent across persist/reload boundary. Store entries referenced by search results can be read back.

## Edge Cases

### Path traversal blocked at store level

1. Run `cargo test -p ath-memory -- store::tests::store_path_traversal_blocked`
2. **Expected:** Attempting to write to a URI that would escape the store root produces `MemoryError::PathTraversal` with the offending URI in the error message.

### Dimension mismatch on upsert

1. Run `cargo test -p ath-memory -- index::tests::dimension_mismatch_on_upsert`
2. **Expected:** Create a 3-dimensional index. Upsert a 5-dimensional vector. Returns `MemoryError::DimensionMismatch { expected: 3, actual: 5 }`.

### Dimension mismatch on search

1. Run `cargo test -p ath-memory -- index::tests::dimension_mismatch_on_search`
2. **Expected:** Create a 3-dimensional index. Search with a 5-dimensional query vector. Returns `MemoryError::DimensionMismatch`.

### Dimension mismatch on load

1. Run `cargo test -p ath-memory -- index::tests::dimension_mismatch_on_load`
2. **Expected:** Save a 3-dimensional index. Load with `expected_dimension: 5`. Returns `MemoryError::DimensionMismatch { expected: 5, actual: 3 }`.

### Empty index search returns empty results

1. Run `cargo test -p ath-memory -- index::tests::empty_index_search_returns_empty`
2. **Expected:** Search on a freshly created index with no entries returns `Ok(vec![])`, not an error.

### Keyword search with no matches

1. Run `cargo test -p ath-memory -- keyword::tests::search_no_matches`
2. **Expected:** Search for a term that doesn't exist in any entry returns an empty result set.

### All MemoryError hints are actionable

1. Run `cargo test -p ath-memory -- error::tests::all_hints_are_actionable`
2. **Expected:** Every `MemoryError` variant's `hint()` returns a non-empty string longer than 20 characters containing resolution guidance.

### Workspace regression check

1. Run `cargo check --workspace`
2. **Expected:** All workspace crates compile cleanly — adding `ath-memory` introduced no regressions.

## Failure Signals

- Any `cargo test -p ath-memory` test failure — indicates a regression in store, index, or type logic
- `cargo check --workspace` errors — indicates the new crate broke workspace compilation
- `MemoryError::hint()` returning empty or unhelpful strings — reduces debuggability
- Persisted index files (`meta.json`, `index.json`) missing after save — persistence broken
- Store markdown files not parseable as human-readable markdown — store format regression

## Not Proven By This UAT

- Performance under load (hundreds/thousands of entries) — no benchmarks in this slice
- Real embedding vectors from an embedding API — all tests use synthetic vectors
- Concurrent access to store or index — no thread safety tests
- Integration with observation system, extraction pipeline, or orchestrator (S02–S06)
- CLI user experience — no CLI in this slice
- Token budget enforcement — later slice concern

## Notes for Tester

- All tests use `tempdir` — no persistent state is left on disk after test runs.
- The `hnsw` crate was substituted for `hnswlib-rs` due to Windows compatibility. If testing on Linux/macOS, everything still works — `hnsw` is cross-platform.
- `MemoryIndex::search()` returns `SearchResult { uri, score }` not `MemoryHit`. Consumers must join with the store to get full content.
- Store files are plain markdown — you can manually inspect them at `<tempdir>/store/<uri-segments>.md` during test debugging by adding a `dbg!` or `println!` of the tempdir path.
