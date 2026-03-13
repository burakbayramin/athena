---
id: T01
parent: S06
milestone: M002
provides:
  - keyword index persistence in post_run_extraction
  - index_path field on MemoryContext
key_files:
  - crates/ath-orchestrator/src/memory.rs
key_decisions:
  - Save keyword index after extraction regardless of extraction success/failure (keywords may have been partially updated)
  - KeywordIndex::save already creates parent directories, so no separate mkdir needed in orchestrator
patterns_established:
  - Fail-soft save with tracing::warn on error, matching existing memory error pattern
observability_surfaces:
  - "tracing::warn with 'keyword index save failed' on persistence failure (includes run_id, error, index_path)"
  - "keyword.json file at index_path — presence/absence is the primary diagnostic signal"
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Fix keyword index persistence in post_run_extraction

**Added `index_path` field to `MemoryContext` and `keywords.save()` call in `post_run_extraction`, so keyword search works after orchestrated runs.**

## What Happened

Added `index_path: PathBuf` field to `MemoryContext` — convention is `memory_dir/index/keyword.json`, matching CLI. In `post_run_extraction`, after `extract_all` completes (regardless of success/failure), the held `keywords_guard` calls `.save(&memory.index_path)`. Wrapped fail-soft with `tracing::warn` on error. Updated all 4 `MemoryContext` construction sites (helper + 3 inline tests) to include `index_path`. Added `keyword_index_persisted_after_extraction` test proving the file appears on disk after a run.

## Verification

- `cargo test -p ath-orchestrator -- memory` — 12/12 pass (11 existing + 1 new)
- `cargo check --workspace` — no new warnings (3 pre-existing in ath-memory)
- Slice verification:
  - `keyword_index_persisted_after_extraction` ✅ passes
  - `two_run_end_to_end` — does not exist yet (T02)
  - `cargo test --workspace` — deferred to final task

## Diagnostics

- After a run, check `keyword.json` at `memory.index_path`. Absence indicates persistence failure.
- Grep logs for `"keyword index save failed"` to find save errors.
- `KeywordIndex::load(path)` is the programmatic inspection surface.

## Deviations

- Task plan step 5 suggested asserting `KeywordIndex::load` returns non-empty index if store has entries. In practice, `MockBackend::always_ok` returns task-output JSON for extraction calls too — extraction writes store entries via a different path than keyword indexing. The store can have entries while the keyword index is correctly empty. Removed the conditional assertion; the primary assertion (file exists + loads successfully) is sufficient.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-orchestrator/src/memory.rs` — added `index_path` field to `MemoryContext`, `keywords.save()` in `post_run_extraction`, updated all test construction sites, added `keyword_index_persisted_after_extraction` test
- `.gsd/milestones/M002/slices/S06/S06-PLAN.md` — added Observability/Diagnostics and diagnostic verification sections
- `.gsd/milestones/M002/slices/S06/tasks/T01-PLAN.md` — added Observability Impact section
