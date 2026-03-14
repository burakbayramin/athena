---
id: S01
parent: M007
milestone: M007
provides:
  - VectorIndex with cosine similarity search
  - JSON persistence (atomic write)
  - add/remove/search/contains/save/load API
requires: []
affects:
  - S02
key_files:
  - crates/ath-memory/src/vector.rs
key_decisions:
  - "D047: O(n) brute-force cosine similarity — suitable for <10K entries, no external vector DB needed"
  - "D048: JSON persistence for vectors — simple, human-readable, acceptable size for <10K entries"
duration: 20m
verification_result: passed
completed_at: 2026-03-14
---

# S01: VectorIndex with Cosine Similarity

**VectorIndex type with cosine similarity search and JSON persistence. 692 tests pass (+13 new).**

## What Happened

Created `VectorIndex` in `crates/ath-memory/src/vector.rs`. Stores embeddings as `Vec<VectorEntry>` with HashMap URI lookup. Cosine similarity search returns top-k results ranked by similarity. Upsert and swap-remove for efficient mutations. Atomic JSON persistence with temp+rename pattern.

## Verification

- 13 new tests: empty/single/ranking/top-k/upsert/remove/similarity/persistence/edge cases
