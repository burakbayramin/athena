# Decisions

<!-- Append-only register of architectural and pattern decisions -->

| ID | Decision | Rationale | Date |
|----|----------|-----------|------|
| D001 | Use `hnswlib-rs` (v0.10) for vector index | Pure Rust, no C/C++ deps, cross-platform, persistence API, cosine metric, string keys, incremental insert/delete. `corenn-kernels` dependency is also pure Rust (only depends on `half`). | 2026-03-14 |
| D002 | Keyword fallback via simple inverted index (HashMap-based) | At Athena's scale (~1000 entries), a hand-rolled TF-IDF index is sufficient. Avoids `tantivy` dependency weight. JSON-serialized for simplicity. | 2026-03-14 |
| D003 | Viking Store uses markdown files (not JSON) for content persistence | Human-readable format lets users inspect and manually edit memory entries. L0/L1/L2 stored as markdown sections with YAML frontmatter for metadata. | 2026-03-14 |
| D004 | MemoryIndex uses `meta.json` alongside binary index files | Stores dimension, entry count, timestamps. Enables dimension mismatch detection on reload and provides an inspection surface for diagnostics. | 2026-03-14 |
| D005 | Store markdown parser splits only on known section headings (`## Abstract`, `## Overview`, `## Detail`) | User content within sections may contain its own `##` headings (e.g., overview_text with `## Conventions`). Splitting on all `## ` lines would break round-trip fidelity. | 2026-03-14 |
| D006 | Use `hnsw` crate (v0.11) instead of `hnswlib-rs` (v0.10) for vector index | `hnswlib-rs` depends on `off64` which uses `std::os::unix` APIs — fails to compile on Windows. `hnsw` is pure Rust, cross-platform (confirmed on x86_64-pc-windows-msvc). Requires manual URI↔ID mapping and cosine distance metric impl, but all tests pass. | 2026-03-14 |
| D007 | HNSW graph rebuilt from stored vectors on load (not serialized) | The `hnsw` crate's `Pcg64` RNG type doesn't implement serde traits, so the graph struct can't be directly serialized. Vectors + URI mappings are saved as JSON; graph is O(n log n) rebuilt on load. Acceptable at current scale. | 2026-03-14 |
