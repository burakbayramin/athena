# S01: Viking Store & Vector Index

**Goal:** `ath-memory` crate exists with Viking URI parsing, layered content store (L0/L1/L2), and vector search — all backed by `.ath/memory/` filesystem.
**Demo:** Unit tests prove URI parsing, store read/write round-trips, HNSW vector search with persistence, and keyword fallback — all with data on disk.

## Must-Haves

- `VikingUri` parser validates `viking://` scheme with path segments, rejects traversal attacks
- `LayeredContent` struct with `abstract_text` (L0), `overview_text` (L1), `detail` (L2) fields
- Filesystem-backed store reads/writes `LayeredContent` as markdown under `.ath/memory/store/`
- `MemoryIndex` wraps `hnswlib-rs` with `upsert`, `search`, `delete`, `save`, `load` operations
- Keyword fallback search works when no embeddings are available (inverted index over stored text)
- Index and store persist to disk and survive process restart (load from persisted files)
- Path traversal protection: resolved paths must stay within `.ath/memory/store/`
- Dimension mismatch detection on index reload
- Crate wired into workspace with consistent conventions (thiserror, serde, workspace deps)

## Proof Level

- This slice proves: contract (VikingStore + MemoryIndex APIs work end-to-end with synthetic vectors)
- Real runtime required: no (unit + integration tests with tempdir)
- Human/UAT required: no

## Verification

- `cargo test -p ath-memory` — all tests pass
- `cargo check --workspace` — no regressions in other crates
- Tests cover: URI parsing (valid/invalid/traversal), store round-trip (write→read), store overwrite, missing entry handling, HNSW insert→search→persist→reload→search, keyword fallback search, dimension mismatch rejection, empty index search
- `MemoryError::hint()` returns actionable strings for all variants (verified by unit test)
- Path traversal attempts produce `MemoryError::PathTraversal` with the offending URI in the error message

## Observability / Diagnostics

- Runtime signals: `tracing` spans on store read/write and index search operations
- Inspection surfaces: index metadata file (`.ath/memory/index/meta.json`) stores dimension, entry count, last modified
- Failure visibility: `MemoryError` enum with actionable `hint()` method (matching project pattern)

## Integration Closure

- Upstream surfaces consumed: none (first slice)
- New wiring introduced in this slice: `ath-memory` workspace member, workspace dependency entry
- What remains before the milestone is truly usable end-to-end: observation system (S02), extraction pipeline (S03), orchestrator integration (S04), CLI (S05), full e2e test (S06)

## Tasks

- [x] **T01: Scaffold ath-memory crate with VikingUri and LayeredContent types** `est:45m`
  - Why: Every other task depends on the crate existing with core types defined. URI parsing with path traversal protection is the first security boundary.
  - Files: `crates/ath-memory/Cargo.toml`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/src/uri.rs`, `crates/ath-memory/src/types.rs`, `crates/ath-memory/src/error.rs`, `Cargo.toml` (workspace root)
  - Do: Create crate with workspace conventions. Implement `VikingUri` parser (scheme validation, path segment extraction, traversal rejection). Define `LayeredContent` with L0/L1/L2 fields. Define `MemoryError` enum with `hint()` method. Wire into workspace root. Unit tests for URI parsing edge cases.
  - Verify: `cargo test -p ath-memory` passes, `cargo check --workspace` passes
  - Done when: VikingUri parses valid URIs, rejects invalid/traversal URIs, LayeredContent serializes/deserializes, crate compiles in workspace

- [x] **T02: Filesystem-backed Viking Store with markdown persistence** `est:45m`
  - Why: The store is the persistence layer that S03's extraction pipeline writes to and S04's injector reads from. Must work reliably with the filesystem.
  - Files: `crates/ath-memory/src/store.rs`, `crates/ath-memory/src/lib.rs`
  - Do: Implement `VikingStore` with `read(uri) -> Option<LayeredContent>`, `write(uri, content)`, `delete(uri)`, `list()` methods. Store each entry as a markdown file at the URI-derived path under a configurable root. Parse/write L0/L1/L2 as markdown sections. Guard against path traversal using `VikingUri::resolve_path()`. Use tempdir in tests.
  - Verify: `cargo test -p ath-memory -- store` passes with tests for write→read round-trip, overwrite, delete, list, missing entry returns None, path traversal is blocked
  - Done when: Store correctly persists and retrieves LayeredContent as markdown files, rejects traversal paths, handles missing entries gracefully

- [x] **T03: HNSW vector index, keyword fallback, and persistence integration tests** `est:1h`
  - Why: This is the slice's primary risk — proving that hnswlib-rs works for our use case with persistence and that keyword fallback provides a viable no-embedding mode. Also writes the slice-level integration tests.
  - Files: `crates/ath-memory/src/index.rs`, `crates/ath-memory/src/keyword.rs`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/Cargo.toml`
  - Do: Implement `MemoryIndex` wrapping `Hnsw<String, Cosine>` + `InMemoryVectorStore`. Methods: `upsert(uri, vector)`, `search(query_vector, top_k) -> Vec<MemoryHit>`, `delete(uri)`, `save(path)`, `load(path)`. Store index metadata (dimension, count) in `meta.json`. Validate dimension on load. Implement `KeywordIndex` with simple inverted index (HashMap-based, JSON-serialized). Write integration tests: insert entries → search → persist → reload → search again; keyword fallback search; dimension mismatch rejection; empty index behavior.
  - Verify: `cargo test -p ath-memory` — all tests pass including integration tests with persisted data in tempdir. `cargo check --workspace` clean.
  - Done when: HNSW index inserts, searches, persists, and reloads correctly with synthetic vectors. Keyword fallback returns relevant results by term matching. Dimension mismatch on reload produces a clear error. All slice must-haves are satisfied.

## Files Likely Touched

- `Cargo.toml` (workspace root — add member + dependency)
- `crates/ath-memory/Cargo.toml`
- `crates/ath-memory/src/lib.rs`
- `crates/ath-memory/src/uri.rs`
- `crates/ath-memory/src/types.rs`
- `crates/ath-memory/src/error.rs`
- `crates/ath-memory/src/store.rs`
- `crates/ath-memory/src/index.rs`
- `crates/ath-memory/src/keyword.rs`
