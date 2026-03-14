---
id: T01
result: passed
---

# T01: VectorIndex type with cosine similarity search

VectorIndex in `crates/ath-memory/src/vector.rs`. In-memory `Vec<VectorEntry>` with `HashMap<String, usize>` URI→index lookup. `add()` with upsert, `remove()` with swap_remove, `search()` with cosine similarity ranking and top-k. 10 unit tests for search, ranking, upsert, removal, similarity edge cases.
