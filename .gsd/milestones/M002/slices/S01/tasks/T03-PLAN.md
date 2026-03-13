---
estimated_steps: 5
estimated_files: 4
---

# T03: HNSW vector index, keyword fallback, and persistence integration tests

**Slice:** S01 — Viking Store & Vector Index
**Milestone:** M002

## Description

Implement `MemoryIndex` wrapping `hnswlib-rs` for vector search, and `KeywordIndex` for fallback when no embeddings are available. Both persist to `.ath/memory/index/`. This task retires the slice's primary risk: proving that a pure-Rust HNSW library works for our use case with persistence, and that keyword fallback provides viable no-embedding search. Also writes the slice-level integration tests combining store + index.

## Steps

1. Add `hnswlib-rs = "0.10"` to `crates/ath-memory/Cargo.toml` dependencies (not workspace — this is a crate-specific dep). Add `rand` as dev-dependency for generating synthetic vectors.
2. Implement `MemoryIndex` in `src/index.rs`: wraps `Hnsw<String, Cosine>` + `InMemoryVectorStore<Dense>`. Constructor takes dimension and max_nodes (default 10,000). Methods: `upsert(uri_str, vector: &[f32])` — uses `hnsw.set()` for insert-or-update. `search(query_vector: &[f32], top_k: usize) -> Vec<MemoryHit>` — guard empty index, return hits with scores. `delete(uri_str)` — soft delete via `hnsw.delete()`. `save(dir: &Path)` — write `hnsw.save_to()` to `hnsw.bin`, `InMemoryVectorStore` serialization to `vectors.bin`, metadata to `meta.json` (dimension, count, created_at). `load(dir: &Path) -> Result<Self>` — read `meta.json`, validate dimension matches, load graph and vectors. `len()`, `is_empty()` accessors.
3. Implement `KeywordIndex` in `src/keyword.rs`: simple inverted index using `HashMap<String, Vec<(String, f32)>>` mapping terms to (uri, tf-idf score). Methods: `add(uri_str, text)` — tokenize, compute term frequencies, update index. `search(query: &str, top_k: usize) -> Vec<MemoryHit>` — tokenize query, score documents by sum of matching term weights, return top_k. `remove(uri_str)`. `save(path)` / `load(path)` — JSON serialization. Tokenization: lowercase, split on whitespace/punctuation, filter stopwords (small hardcoded list).
4. Write unit tests for `MemoryIndex`: insert 10 synthetic vectors → search returns nearest → persist to tempdir → reload → search returns same results. Test dimension mismatch on load. Test empty index search returns empty vec. Test delete removes entry from results. Test upsert updates existing entry.
5. Write integration tests combining store + index: create a `VikingStore` + `MemoryIndex` in tempdir, write several `LayeredContent` entries to store, upsert corresponding vectors into index, search by vector and verify returned URIs match expected entries, persist both to disk, reload both from disk, search again and verify consistency. Test keyword fallback: add text from LayeredContent to KeywordIndex, search by keyword, verify results.

## Must-Haves

- [ ] `MemoryIndex` inserts vectors and returns relevant search results
- [ ] `MemoryIndex` persists (save) and reloads (load) with consistent search results
- [ ] Dimension mismatch on load produces `MemoryError::DimensionMismatch` with clear message
- [ ] Empty index search returns empty vec (no panic)
- [ ] `KeywordIndex` finds entries by keyword match
- [ ] `KeywordIndex` persists and reloads
- [ ] Integration test: store + index work together with disk persistence

## Verification

- `cargo test -p ath-memory` — all tests pass (unit + integration)
- `cargo check --workspace` — no regressions

## Observability Impact

- Signals added: `tracing::info_span!` on index search (logs dimension, query top_k, results count)
- How a future agent inspects this: `meta.json` in index dir shows dimension, entry count, last modified
- Failure state exposed: `MemoryError::DimensionMismatch` includes expected vs actual dimensions, `MemoryError::IndexError` wraps hnswlib-rs errors with context

## Inputs

- `crates/ath-memory/src/uri.rs` — `VikingUri` for URI strings in index keys
- `crates/ath-memory/src/types.rs` — `MemoryHit` for search results, `LayeredContent` for integration tests
- `crates/ath-memory/src/store.rs` — `VikingStore` for integration tests combining store + index
- `crates/ath-memory/src/error.rs` — `MemoryError` variants for index errors
- `hnswlib-rs` API: `Hnsw::new(Cosine, HnswConfig::new(dim, max_nodes))`, `.set()`, `.search()`, `.save_to()`, `Hnsw::load_from(Cosine, reader)`, `InMemoryVectorStore::<Dense>::new(dim)`

## Expected Output

- `crates/ath-memory/src/index.rs` — MemoryIndex implementation with persistence and tests
- `crates/ath-memory/src/keyword.rs` — KeywordIndex with inverted index and tests
- `crates/ath-memory/src/lib.rs` — updated with new module declarations and re-exports
- `crates/ath-memory/Cargo.toml` — updated with hnswlib-rs dependency
