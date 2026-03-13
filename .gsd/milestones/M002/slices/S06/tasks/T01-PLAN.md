---
estimated_steps: 5
estimated_files: 1
---

# T01: Fix keyword index persistence in post_run_extraction

**Slice:** S06 — End-to-End Integration
**Milestone:** M002

## Description

The orchestrator's `post_run_extraction` modifies the keyword index during extraction (via `MemoryExtractor` calling `keywords.add()`) but never saves it to disk. This means keyword-based search after an orchestrated run finds nothing — the index lives only in memory and is dropped with the `MemoryContext`. The fix is adding a `keywords.save()` call and an `index_path` field to `MemoryContext`.

## Steps

1. Add `index_path: PathBuf` field to `MemoryContext` struct. This tells the orchestrator where to persist the keyword index (convention: `memory_dir/index/keyword.json`, matching CLI).
2. In `post_run_extraction`, after `extract_all` completes (success or failure — keywords may have been partially updated), call `keywords_guard.save(&memory.index_path)`. Ensure parent directory is created. Wrap in fail-soft: log `tracing::warn` on error, don't propagate.
3. Update `make_memory_context` test helper to set `index_path` to `tmp.path().join("index").join("keyword.json")`.
4. Update the `MemoryContext` construction in `memory_errors_do_not_fail_run` and `memory_context_injects_context_from_store` tests to include `index_path`.
5. Add test `keyword_index_persisted_after_extraction`: construct a single-phase run with `MockBackend::always_ok` (extraction uses same responses — JSON parsing may fail but that's fine, the test asserts the save mechanism works). After `run_plan_with_memory`, assert `index_path` file exists. If extraction succeeded (check via store entries), also assert `KeywordIndex::load` returns non-empty index.

## Must-Haves

- [ ] `index_path: PathBuf` field on `MemoryContext`
- [ ] `keywords.save()` called in `post_run_extraction` after extraction (fail-soft)
- [ ] Parent directory for index_path created if needed
- [ ] All existing `MemoryContext` construction sites updated
- [ ] Test proving keyword.json appears on disk after a run

## Verification

- `cargo test -p ath-orchestrator -- memory` — all tests pass (existing + new)
- `cargo check --workspace` — no new warnings

## Observability Impact

- **New signal**: `tracing::warn` emitted when `keywords.save(&index_path)` fails, with `run_id`, `error`, and `index_path` fields. A future agent can detect persistence failures by grepping for `"keyword index save failed"`.
- **Inspection surface**: `keyword.json` at `memory.index_path` — its presence/absence after a run is the primary signal. `KeywordIndex::load()` is the programmatic verification.
- **Failure state visibility**: save failure is logged but does not affect run outcome (fail-soft). The existing extraction completion log (`"Extraction pipeline completed"`) still fires regardless of save outcome.
- **No new happy-path log**: silent save on success — the existing extraction log is sufficient.

## Inputs

- `crates/ath-orchestrator/src/memory.rs` — existing `MemoryContext` struct and `post_run_extraction` method
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::save(&self, path: &Path)` API
- CLI pattern: `memory_dir.join("index").join("keyword.json")` from `crates/ath-cli/src/memory.rs`

## Expected Output

- `crates/ath-orchestrator/src/memory.rs` — `MemoryContext` gains `index_path` field, `post_run_extraction` gains `save()` call, new test proving persistence
