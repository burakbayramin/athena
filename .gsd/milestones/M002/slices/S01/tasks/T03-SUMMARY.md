---
id: T03
parent: S01
milestone: M002
provides:
  - MemoryIndex with HNSW vector search (cosine similarity), upsert/search/delete/save/load
  - KeywordIndex with inverted index fallback search, JSON persistence
  - Integration test proving store + vector index + keyword index work together with disk persistence
key_files:
  - crates/ath-memory/src/index.rs
  - crates/ath-memory/src/keyword.rs
  - crates/ath-memory/src/lib.rs
  - crates/ath-memory/Cargo.toml
key_decisions:
  - "Used `hnsw` crate (v0.11) instead of `hnswlib-rs` (v0.10) because hnswlib-rs depends on `off64` which uses Unix-only APIs (std::os::unix) and fails to compile on Windows"
  - "HNSW graph rebuilt from stored vectors on load rather than serializing the graph directly, because the `hnsw` crate's Pcg64 RNG doesn't implement serde traits"
  - "Soft-delete via HashSet tracking rather than true graph deletion, because the `hnsw` crate doesn't support node removal"
  - "Cosine distance mapped to u32 via f32::to_bits() for the space::Metric trait requirement of unsigned integer distance"
patterns_established:
  - "MemoryIndex wraps hnsw::Hnsw with URI-to-ID bidirectional mapping, separate vectors HashMap for persistence, and deleted HashSet for soft-delete filtering"
  - "KeywordIndex uses hand-rolled tokenizer (lowercase, split on non-alphanumeric, filter stopwords) with TF scoring"
observability_surfaces:
  - "tracing info_span on index search (logs dimension, top_k, index_len)"
  - "tracing debug on upsert and search completion (logs URI, ID, results count)"
  - "meta.json in index directory shows dimension, entry count, created_at"
  - "MemoryError::DimensionMismatch includes expected vs actual dimensions"
  - "MemoryError::IndexError wraps hnsw errors with operation context"
duration: 40m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T03: HNSW vector index, keyword fallback, and persistence integration tests

**Implemented MemoryIndex (HNSW cosine search) and KeywordIndex (inverted index fallback) with full persistence and integration tests proving store + index round-trip.**

## What Happened

Added `hnsw` crate (v0.11, pure-Rust, cross-platform) and `space` (v0.17) for the metric trait. Implemented `CosineDistance` metric mapping cosine distance to u32 for the space::Metric requirement. Built `MemoryIndex` wrapping `hnsw::Hnsw<CosineDistance, Vec<f32>, Pcg64, 12, 24>` with bidirectional URI↔ID mapping and a separate vectors HashMap for persistence.

The `hnswlib-rs` crate specified in the task plan couldn't be used because its `off64` dependency uses `std::os::unix` which doesn't compile on Windows. Pivoted to the `hnsw` crate which is pure Rust and cross-platform. The API difference required building a mapping layer (URI strings to integer IDs) and custom persistence (vectors saved as JSON, graph rebuilt on load).

KeywordIndex is a straightforward inverted index with TF scoring, stopword filtering, and JSON serialization. Provides viable no-embedding search for keyword queries.

Integration test exercises the full pipeline: create VikingStore + MemoryIndex + KeywordIndex → write LayeredContent entries → upsert vectors → search by vector → search by keyword → persist all to disk → reload all → search again → verify consistency and cross-reference store entries with search results.

## Verification

- `cargo test -p ath-memory` — 54 tests + 1 doc-test pass
- `cargo check --workspace` — no regressions
- All slice verification checks pass (this is the final task):
  - URI parsing (valid/invalid/traversal): 11 tests
  - Store round-trip, overwrite, delete, list, missing entry: 10 tests
  - HNSW insert→search→persist→reload→search: 2 tests
  - Keyword fallback search and persistence: 5 tests
  - Dimension mismatch rejection: 3 tests
  - Empty index search: 1 test
  - MemoryError::hint() actionable: 1 test
  - Path traversal → MemoryError::PathTraversal: 1 test
  - Integration store+index: 1 test

## Diagnostics

- Inspect index behavior: `cargo test -p ath-memory -- index` runs all index tests
- Inspect keyword behavior: `cargo test -p ath-memory -- keyword` runs all keyword tests
- Inspect integration: `cargo test -p ath-memory -- integration` runs the combined test
- Index files on disk: `meta.json` (dimension, count, created_at), `index.json` (vectors + mappings)
- Keyword index on disk: single JSON file with postings and doc mappings
- MemoryError Display output embeds dimensions, URIs, and paths for log-level diagnosis

## Deviations

- Replaced `hnswlib-rs = "0.10"` with `hnsw = "0.11"` + `space = "0.17"` + `rand_pcg = "0.3"` because hnswlib-rs doesn't compile on Windows (Unix-only `off64` dependency)
- HNSW graph is rebuilt from stored vectors on load rather than being serialized, because the RNG type doesn't implement serde
- `MemoryIndex` methods take `&mut self` instead of `&self` (the `hnsw` crate's insert requires mutable reference, unlike hnswlib-rs which used interior mutability)
- `search()` result type is `SearchResult` (uri_str + score) rather than `MemoryHit` (which includes full LayeredContent), because the index doesn't own store data — consumers join with the store themselves

## Known Issues

- Soft-delete doesn't reclaim graph memory — deleted entries remain in the HNSW graph until a full rebuild. Acceptable for current scale.
- Graph rebuild on load is O(n log n) — fine for typical index sizes (<100k entries) but would need optimization for very large indices.
- `rand_pcg` is an additional dependency (used by the `hnsw` crate's generic RNG parameter).

## Files Created/Modified

- `crates/ath-memory/src/index.rs` — MemoryIndex implementation with HNSW wrapper, persistence, and 8 unit + 1 integration test
- `crates/ath-memory/src/keyword.rs` — KeywordIndex with inverted index, TF scoring, JSON persistence, and 7 tests
- `crates/ath-memory/src/lib.rs` — Added index and keyword module declarations and re-exports
- `crates/ath-memory/Cargo.toml` — Added hnsw, space, rand_pcg dependencies; rand as dev-dependency
