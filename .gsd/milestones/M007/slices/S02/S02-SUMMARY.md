---
id: S02
parent: M007
milestone: M007
provides:
  - ContextInjector.with_vector_search() for embedding-based recall
  - Vector search preferred over keyword when available
  - Graceful fallback to keyword when vector index is empty
requires:
  - S01
affects: []
key_files:
  - crates/ath-memory/src/inject/injector.rs
key_decisions:
  - "D049: Query embedding provided by caller — ContextInjector doesn't call embedding API directly (sync boundary)"
  - "D050: Vector search falls back to keyword when vector index returns no results"
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S02: ContextInjector Integration

**ContextInjector prefers vector search with keyword fallback. 694 tests pass (+2 new).**

## What Happened

Added optional `vector_index` and `query_embedding` to `ContextInjector`. `with_vector_search()` builder method attaches both. `read_semantic_results()` unified hit processing — URIs collected from either vector or keyword search, then processed identically. Falls back to keyword when vector index is empty.

## Verification

- 2 new tests: vector preferred over keyword, fallback when empty
- All existing injector tests pass (keyword path unchanged)
