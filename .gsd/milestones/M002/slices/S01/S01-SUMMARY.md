---
id: S01
parent: M002
milestone: M002
provides:
  - ath-memory crate with VikingUri parser, LayeredContent types, MemoryError with hints
  - Filesystem-backed VikingStore with markdown persistence and atomic writes
  - MemoryIndex wrapping HNSW cosine vector search with persistence
  - KeywordIndex inverted-index fallback for no-embedding search
requires: []
affects:
  - S03
  - S04
  - S05
key_files:
  - crates/ath-memory/Cargo.toml
  - crates/ath-memory/src/lib.rs
  - crates/ath-memory/src/uri.rs
  - crates/ath-memory/src/types.rs
  - crates/ath-memory/src/error.rs
  - crates/ath-memory/src/store.rs
  - crates/ath-memory/src/index.rs
  - crates/ath-memory/src/keyword.rs
  - Cargo.toml
key_decisions:
  - "Used `hnsw` crate (v0.11) instead of `hnswlib-rs` (v0.10) — hnswlib-rs depends on `off64` which uses Unix-only APIs (std::os::unix), fails on Windows"
  - "HNSW graph rebuilt from stored vectors on load rather than serialized — the `hnsw` crate's Pcg64 RNG doesn't implement serde"
  - "Soft-delete via HashSet tracking — `hnsw` crate doesn't support node removal"
  - "Store markdown parser splits only on known section headings to preserve user content with ## headings"
  - "VikingUri custom Serialize/Deserialize (string form) so URIs serialize as 'viking://project/conventions' not as struct fields"
  - "Atomic temp-file + rename for store writes with Windows remove-before-rename handling"
  - "Keyword fallback via simple HashMap-based inverted index with TF scoring — avoids tantivy dep weight"
patterns_established:
  - "VikingUri FromStr/Display/Serialize/Deserialize as the canonical URI type across memory subsystem"
  - "MemoryError with hint() matching ath-config's ConfigError pattern"
  - "LayeredContent L0/L1/L2 markdown format with YAML frontmatter"
  - "MemoryIndex wraps hnsw::Hnsw with URI-to-ID bidirectional mapping and separate vectors HashMap for persistence"
  - "VikingStore temp-file + rename for atomic writes"
observability_surfaces:
  - "tracing::instrument on all store CRUD methods and index search/upsert/save/load"
  - "MemoryError::hint() returns actionable resolution strings for all 6 variants"
  - "meta.json in index directory shows dimension, entry count, created_at"
  - "Store files at .ath/memory/store/<segments>.md are plain markdown, directly inspectable"
drill_down_paths:
  - .gsd/milestones/M002/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M002/slices/S01/tasks/T03-SUMMARY.md
duration: ~75m
verification_result: passed
completed_at: 2026-03-14
---

# S01: Viking Store & Vector Index

**`ath-memory` crate with VikingUri parsing, filesystem-backed layered content store, HNSW vector search, and keyword fallback — 54 tests + 1 doc-test pass, workspace clean.**

## What Happened

Built the `ath-memory` crate across three tasks:

**T01** scaffolded the crate with core types: `VikingUri` parser (validates `viking://` scheme, rejects traversal/empty/dot segments, resolves to filesystem paths), `LayeredContent` (L0 abstract, L1 overview, L2 detail with timestamps), `MemoryHit` (search result wrapper), and `MemoryError` (6 variants with actionable `hint()` method). Wired into workspace root.

**T02** built `VikingStore` — CRUD operations persisting LayeredContent as human-readable markdown files with YAML frontmatter. Uses temp-file + rename for atomic writes (with Windows-specific remove-before-rename). Section parser restricted to known headings so user content with `##` markers survives round-trip. All four methods (`write`, `read`, `delete`, `list`) instrumented with tracing.

**T03** implemented `MemoryIndex` wrapping the `hnsw` crate (pure Rust, cross-platform — pivoted from `hnswlib-rs` which doesn't compile on Windows). Cosine similarity search with bidirectional URI↔ID mapping, soft-delete via HashSet, and JSON persistence of vectors + mappings (graph rebuilt on load since the RNG type isn't serializable). Also built `KeywordIndex` — a HashMap-based inverted index with TF scoring and stopword filtering for no-embedding fallback. Integration test proves store + vector index + keyword index work together end-to-end with disk persistence.

## Verification

- `cargo test -p ath-memory` — **54 tests + 1 doc-test pass**
- `cargo check --workspace` — clean, no regressions
- URI parsing: 11 tests (valid/invalid/traversal/serde)
- Store CRUD: 14 tests (round-trip, overwrite, delete, list, path traversal, atomic write, human-readable format)
- HNSW index: 8 tests (insert, search, upsert, delete, persist/reload, dimension mismatch on upsert/search/load, empty index)
- Keyword index: 7 tests (add/search, remove, stopwords, tokenization, upsert, persist/reload, empty/no-match)
- Integration: 1 test (store + vector index + keyword index end-to-end with persistence)
- Error hints: all 6 MemoryError variants produce actionable hints (>20 chars)
- Path traversal: produces `MemoryError::PathTraversal` with URI in message

## Deviations

- **`hnsw` (v0.11) instead of `hnswlib-rs` (v0.10)** — hnswlib-rs depends on `off64` which uses `std::os::unix` and fails on Windows. The `hnsw` crate is pure Rust and cross-platform. Required building a URI↔ID mapping layer and custom persistence (vectors as JSON, graph rebuilt on load).
- **`MemoryIndex` methods take `&mut self`** — the `hnsw` crate requires mutable reference for insert, unlike hnswlib-rs which used interior mutability.
- **`search()` returns `SearchResult` (uri + score) not `MemoryHit`** — the index doesn't own store data, so consumers join with the store themselves.

## Known Limitations

- Soft-delete doesn't reclaim graph memory — deleted entries remain in HNSW graph until full rebuild. Acceptable at current scale.
- Graph rebuild on load is O(n log n) — fine for <100k entries, would need optimization at larger scale.
- `rand_pcg` is an additional dependency required by the `hnsw` crate's generic RNG parameter.

## Follow-ups

None — all slice must-haves are satisfied.

## Files Created/Modified

- `crates/ath-memory/Cargo.toml` — new crate manifest with workspace deps (serde, thiserror, tracing, hnsw, space, rand_pcg)
- `crates/ath-memory/src/lib.rs` — crate root with module declarations and re-exports
- `crates/ath-memory/src/uri.rs` — VikingUri parser with FromStr/Display/Serialize/Deserialize (14 tests)
- `crates/ath-memory/src/types.rs` — LayeredContent and MemoryHit structs (3 tests)
- `crates/ath-memory/src/error.rs` — MemoryError enum with 6 variants and hint() (6 tests)
- `crates/ath-memory/src/store.rs` — VikingStore with markdown persistence and atomic writes (14 tests)
- `crates/ath-memory/src/index.rs` — MemoryIndex with HNSW wrapper and persistence (8 unit + 1 integration test)
- `crates/ath-memory/src/keyword.rs` — KeywordIndex with inverted index and TF scoring (7 tests)
- `Cargo.toml` — added ath-memory workspace member and dependency

## Forward Intelligence

### What the next slice should know
- `MemoryIndex::search()` returns `SearchResult { uri: String, score: f32 }` — consumers must call `VikingStore::read()` separately to get the full `LayeredContent`. This join pattern should be wrapped in a convenience method by whichever slice needs it (likely S03 or S04).
- `MemoryIndex` methods require `&mut self`. If shared across threads, wrap in `Mutex` or `RwLock`.
- The `hnsw` crate uses generic type params `<CosineDistance, Vec<f32>, Pcg64, 12, 24>` — don't change the const generics without understanding their effect on recall/performance.

### What's fragile
- **HNSW persistence format** — vectors stored as JSON with URI↔ID mappings. If `MemoryIndex` struct fields change, existing persisted indexes won't load. Consider a version field in meta.json before shipping to users.
- **Store markdown parser** — splits only on `## Abstract`, `## Overview`, `## Detail` headings. If these section names change, existing store files won't parse correctly.

### Authoritative diagnostics
- `cargo test -p ath-memory -- index` — exercises all vector index paths including persistence round-trip
- `cargo test -p ath-memory -- integration` — the combined store+index test is the single best health check
- `meta.json` in the index directory — shows dimension, count, and timestamps; first place to look for index state

### What assumptions changed
- **Original: hnswlib-rs is cross-platform** → it depends on Unix-only APIs via `off64`. Replaced with `hnsw` crate (pure Rust).
- **Original: HNSW graph can be serialized directly** → RNG type isn't serde-compatible. Graph rebuilt from vectors on load.
- **Original: index methods can be `&self`** → `hnsw` crate requires `&mut self` for insert.
