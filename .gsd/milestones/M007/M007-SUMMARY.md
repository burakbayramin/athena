---
id: M007
title: "Embedding-Based Memory Recall"
status: complete
slices_completed: 2
slices_total: 2
tests_before: 679
tests_after: 694
started: 2026-03-14
completed: 2026-03-14
---

# M007: Embedding-Based Memory Recall — Summary

Added VectorIndex for embedding-based semantic search and integrated it into ContextInjector as the preferred search method, with automatic keyword fallback.

## Architecture

```
VectorIndex (in-memory Vec<f32> embeddings + cosine similarity)
  ↔ JSON persistence (atomic write)
  → ContextInjector.with_vector_search(vector_index, query_embedding)
    → read_semantic_results prefers vector hits
    → falls back to KeywordIndex when no vector results
```

## Slices Delivered

- **S01**: VectorIndex with cosine similarity, add/remove/search/persist/load. 13 new tests.
- **S02**: ContextInjector integration — `with_vector_search()` builder, vector-preferred search. 2 new tests.

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| D047 | O(n) brute-force cosine similarity | Suitable for <10K entries, no external vector DB needed |
| D048 | JSON persistence for vectors | Simple, human-readable, acceptable for project-scale data |
| D049 | Query embedding provided by caller | ContextInjector stays sync — embedding API call handled externally |
| D050 | Falls back to keyword when vector index empty | Graceful degradation for users without embedding API |

## Files Changed

- `crates/ath-memory/src/vector.rs` — VectorIndex (new)
- `crates/ath-memory/src/inject/injector.rs` — vector search integration
- `crates/ath-memory/src/lib.rs` — exports
