# S01: Viking Store & Vector Index — Research

**Date:** 2026-03-14

## Summary

The `ath-memory` crate needs a Viking URI parser, a layered content store (L0/L1/L2), and a vector index for semantic search — all backed by the `.ath/memory/` filesystem. The primary risk was finding a pure-Rust HNSW library that supports incremental insert, persistence, and works cross-platform without C++ dependencies. That risk is retired: **`hnswlib-rs`** (crates.io) is a pure-Rust port of hnswlib with exactly the right API surface — decoupled graph/vector storage, `save_to`/`load_from` persistence, `insert`/`delete`/`set` for incremental mutation, cosine distance support, and string keys that map naturally to Viking URIs.

The existing codebase uses clean patterns: workspace-level dependencies, serde for serialization, `thiserror` for error types, `async-trait` for trait objects, and well-structured `Cargo.toml` configurations. The `ath-memory` crate should follow these conventions exactly. `AgentRequest` already has a `context: Option<String>` field — perfect for S04's context injection, meaning no type changes needed in `ath-types` for this slice.

For embeddings, OpenAI's `text-embedding-3-small` outputs 1536 dimensions by default, but supports Matryoshka shortening via the `dimensions` API parameter. At Athena's scale (hundreds to low thousands of entries), 256 or 512 dimensions is sufficient and reduces index size ~3-6x. The system must gracefully degrade to keyword/FTS search when no embedding API is configured.

## Recommendation

Use **`hnswlib-rs`** (the crates.io pure-Rust port, not `hnsw_rs` by jean-pierreBoth which depends on `anndists` with SIMD features). It provides:

- Decoupled `Hnsw<K, M>` graph + `InMemoryVectorStore` — graph owns key→NodeId mapping, vectors stored separately
- `save_to()`/`load_from()` for both graph and vector store via `std::io::Write`/`std::io::Read`
- Cosine distance metric (correct for text embeddings)
- Incremental `insert`/`delete`/`set` (insert-or-update with connection repair)
- String keys — Viking URI strings map directly as keys
- Concurrent search + mutation (relevant for future but not needed in S01)
- Pure Rust, no C/C++ build dependencies

For the Viking store itself, use the filesystem directly: markdown files at Viking URI paths under `.ath/memory/store/`. L0 (abstract), L1 (overview), and L2 (detail) are stored as sections within each file, parsed and written via serde or a simple markdown structure. The vector index files (`hnsw.bin`, `vectors.bin`) persist alongside in `.ath/memory/index/`.

Keyword fallback: when no embeddings are available, implement a simple TF-IDF or BM25-style search over the stored text using the `tantivy` crate or a hand-rolled inverted index. Given the small scale, a simple inverted index with `HashMap<String, Vec<(VikingUri, f32)>>` serialized as JSON is sufficient and avoids a heavy dependency.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| HNSW vector index | `hnswlib-rs` crate | Pure Rust, persistence API, incremental insert, cosine distance, string keys |
| Serialization | `serde` + `serde_json` (workspace) | Already used everywhere in the project |
| Error types | `thiserror` (workspace) | Consistent with all other ath-* crates |
| URI parsing | Hand-roll `VikingUri` parser | Simple enough (scheme + 2-3 path segments), no need for a URI crate |
| Full-text search fallback | Simple inverted index | At Athena's scale (~1000 entries), BM25 over a `HashMap` is sufficient; `tantivy` would be overkill |
| Unique IDs | `uuid` (workspace) | Already in workspace deps |

## Existing Code and Patterns

- `crates/ath-config/Cargo.toml` — Pattern for new crate: workspace version/edition, workspace deps, internal crate deps
- `crates/ath-config/src/store.rs` — `ConfigStore` shows clean layered-load pattern; `VikingStore` should follow similar ergonomics
- `crates/ath-types/src/agent.rs` — `AgentRequest.context: Option<String>` — the integration point for S04 context injection. Already exists, no type changes needed for this slice
- `crates/ath-types/src/lib.rs` — Re-export pattern: pub mod + pub use at crate root for ergonomic imports
- `crates/ath-config/src/error.rs` — Error enum pattern with `thiserror` derives
- `crates/ath-orchestrator/src/phase_runner.rs` — Typestate pattern; memory types should use simpler enums but follow the same rigor for state validity
- `Cargo.toml` (workspace root) — Add `ath-memory = { path = "crates/ath-memory" }` to `[workspace.dependencies]`

## Constraints

- **Pure Rust only** — No C/C++ build dependencies. Rules out `hnsw_rs` (depends on `anndists` with SIMD features), `faiss-rs`, and any crate wrapping C++ hnswlib
- **Cross-platform** — Must compile on Windows (MSVC), macOS, Linux. `hnswlib-rs` is pure Rust, so this is fine
- **Embedding dimension configurable** — `text-embedding-3-small` defaults to 1536 dims but supports shortening to 256/512/1024 via API parameter. Index dimension must be set at creation time and validated on load
- **Filesystem-based storage** — All data in `.ath/memory/` under project root. No database, no server
- **`hnswlib-rs` requires `max_nodes` at construction** — Must choose a reasonable upper bound (e.g., 10,000). Can be made configurable in memory config
- **Vector store and graph persist separately** — `hnswlib-rs` saves graph+key mapping in one file, vectors in another. Must keep these in sync (write both atomically, or at least sequentially with error handling)
- **No embedding API in S01** — S01 proves the store + index mechanics work with synthetic vectors. Real embeddings come in S03/S04

## Common Pitfalls

- **Graph/vector store desync** — `hnswlib-rs` persists graph and vectors separately. If one write succeeds and the other fails, the index is corrupt. Write both to temp files first, then rename atomically (or at least handle partial-write recovery)
- **Dimension mismatch on reload** — If the embedding model changes (or user switches from 512→256 dims), the persisted index is incompatible. Store dimension in a metadata file and validate on load; offer a `reindex` command for dimension changes
- **Viking URI path traversal** — `viking://project/../../../etc/passwd` must be rejected. Validate that resolved paths stay within `.ath/memory/store/`
- **Empty index search** — Searching an empty HNSW index may return no results or panic (depending on library). Guard with a `len() == 0` check before search
- **Key collision on upsert** — `hnswlib-rs` `set()` does insert-or-update, which is what we want for `upsert()`. But the vector must also be updated in the store. Ensure both graph key and vector store are updated together
- **`max_nodes` exhaustion** — If the index fills up beyond `max_nodes`, inserts will fail. Either set a generous default (10,000) or implement index compaction/rebuild

## Open Risks

- **`hnswlib-rs` crate maturity** — Published January 2026, relatively new. May have edge cases or API changes. Mitigation: wrap it behind our own `MemoryIndex` trait so we can swap implementations if needed
- **Keyword fallback quality** — A simple inverted index won't match semantic search quality. This is acceptable for the "no embedding API" fallback, but should be documented as a limitation
- **Persistence atomicity on Windows** — Atomic rename (`std::fs::rename`) behaves differently on Windows (can fail if target exists). Use `rename` with explicit `remove_file` + error handling, or write-to-temp + rename pattern
- **Memory pressure with large indexes** — `InMemoryVectorStore` loads all vectors into RAM. At 10,000 entries × 512 dims × 4 bytes = ~20MB, this is fine. At 100,000 entries it'd be ~200MB — but Athena's scale is well below that

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust development | `cosmonic-labs/skills@rust-development` | available (3 installs — low adoption, skip) |
| Vector databases | `melodic-software/claude-code-plugins@vector-databases` | available (14 installs — generic, not Rust-specific, skip) |

No skills are directly relevant enough to warrant installation. The work is standard Rust crate development.

## Sources

- `hnswlib-rs` is pure-Rust HNSW with decoupled graph/vector storage, string keys, persistence, and cosine distance (source: [crates.io](https://crates.io/crates/hnswlib-rs))
- `hnsw_rs` depends on `anndists` crate with SIMD features, not suitable for pure-Rust cross-platform constraint (source: [GitHub](https://github.com/jean-pierreBoth/hnswlib-rs))
- `small-world-rs` is simpler but less mature; `hnswlib-rs` has richer API for our needs (source: [crates.io](https://crates.io/crates/small-world-rs))
- `text-embedding-3-small` outputs 1536 dimensions by default, supports Matryoshka shortening via `dimensions` API parameter (source: [OpenAI](https://openai.com/index/new-embedding-models-and-api-updates/))
- `instant-distance` is build-once with no incremental insert — unsuitable for a memory system that grows over time (source: [crates.io](https://crates.io/crates/instant-distance))
