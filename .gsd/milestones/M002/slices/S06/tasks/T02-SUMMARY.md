---
id: T02
parent: S06
milestone: M002
provides:
  - two-run end-to-end memory lifecycle integration test
  - milestone M002 definition-of-done test
key_files:
  - crates/ath-orchestrator/src/memory.rs
key_decisions:
  - Sequenced MockBackend with 5 responses (1 task + 4 extraction) for Run 1 Claude, proving discriminant-based registry routes both task and extraction calls through same backend
  - Run 2 uses CapturingBackend wrapping always_ok mock — extraction in Run 2 is irrelevant, only context injection matters
patterns_established:
  - Two-run test pattern: shared temp dir, fresh store/index instances per run, CapturingBackend for injection verification
observability_surfaces:
  - Test assertions at each lifecycle boundary serve as diagnostic surface — a failure pinpoints which stage regressed (store persistence, keyword persistence, context injection, or API access)
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Two-run end-to-end integration test

**Added `two_run_end_to_end_memory_lifecycle` test proving the complete memory lifecycle: Run 1 captures/extracts/persists, Run 2 receives injected context from Run 1, cross-run APIs show accumulated data.**

## What Happened

Built the milestone's definition-of-done test. Run 1 uses a sequenced `MockBackend` with 5 Claude responses (1 task output JSON + 4 extraction JSONs matching `MockExtractionLlm::with_all_stages()` shapes). After Run 1, assertions verify: observations exist on disk, store has entries (run summary, conventions, decisions), keyword index file persisted.

Run 2 points a fresh `VikingStore` and `KeywordIndex::load()` at the same temp dir. A `CapturingBackend` wraps an `always_ok` mock for Claude. After Run 2, the captured task request's `context` field contains `<athena_context>` with a `<recent_run>` section referencing Run 1's extraction content ("authentication"/"JWT").

Cross-run API verification: `store.list()` shows Run 1 entries, `KeywordIndex::load()` + `.search("authentication")` and `.search("JWT")` return non-empty results.

## Verification

- `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle` — passed
- `cargo test -p ath-orchestrator -- memory::tests::keyword_index_persisted_after_extraction` — passed
- `cargo test --workspace` — 575 passed, 0 failed, 0 regressions

## Diagnostics

- If the test fails on Run 1 store assertions: extraction JSON shapes may have changed — compare with `MockExtractionLlm::with_all_stages()` in `pipeline.rs`.
- If the test fails on Run 2 context injection: check `ContextInjector::read_recent_run` — it looks for `viking://runs/*/summary` URIs and reads `abstract_text`.
- If keyword search returns empty: verify `KeywordIndex::save`/`load` round-trip and that extraction stages call `keywords.add()`.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-orchestrator/src/memory.rs` — added `two_run_end_to_end_memory_lifecycle` test (~170 lines)
- `.gsd/milestones/M002/slices/S06/S06-PLAN.md` — added diagnostic verification step, marked T02 done
- `.gsd/milestones/M002/slices/S06/tasks/T02-PLAN.md` — added Observability Impact section
