---
id: S06
parent: M002
milestone: M002
provides:
  - keyword index persistence after orchestrated extraction
  - two-run end-to-end memory lifecycle integration test (milestone definition-of-done)
  - index_path field on MemoryContext
requires:
  - slice: S01
    provides: VikingStore + KeywordIndex
  - slice: S02
    provides: ObservationBuffer + ObservationReader
  - slice: S03
    provides: MemoryExtractor + extraction JSON schemas
  - slice: S04
    provides: ContextInjector + MemoryContext + run_plan_with_memory
  - slice: S05
    provides: CLI memory APIs
affects: []
key_files:
  - crates/ath-orchestrator/src/memory.rs
key_decisions:
  - Keyword index saved after extraction regardless of per-stage success/failure — partial keyword updates are still valuable
patterns_established:
  - Two-run test pattern: shared temp dir, fresh store/index instances per run, CapturingBackend for injection verification
  - Fail-soft save with tracing::warn on error for keyword persistence, matching existing memory error pattern
observability_surfaces:
  - "tracing::warn 'keyword index save failed' on persistence failure (includes run_id, error, index_path)"
  - "keyword.json at index_path — presence is the primary diagnostic signal after extraction"
  - "Test assertions at each lifecycle boundary pinpoint which stage regressed"
drill_down_paths:
  - .gsd/milestones/M002/slices/S06/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S06/tasks/T02-SUMMARY.md
duration: 30m
verification_result: passed
completed_at: 2026-03-14
---

# S06: End-to-End Integration

**Keyword index persistence fix + two-run integration test proving the complete M002 memory lifecycle: capture → extract → persist → reload → inject → verify.**

## What Happened

Two tasks, both in `crates/ath-orchestrator/src/memory.rs`:

**T01 — Keyword index persistence fix.** Added `index_path: PathBuf` to `MemoryContext`. After `extract_all` in `post_run_extraction`, keyword index saves to disk via `keywords_guard.save(&memory.index_path)` — fail-soft with `tracing::warn` on error. Updated all 4 `MemoryContext` construction sites to include `index_path`. Added `keyword_index_persisted_after_extraction` test proving the file appears on disk after a run.

**T02 — Two-run end-to-end test.** The milestone definition-of-done test. Run 1: sequenced `MockBackend` with 5 Claude responses (1 task output + 4 extraction JSONs). Assertions verify observations on disk, store entries created, keyword index persisted. Run 2: fresh store/index from same temp dir, `CapturingBackend` wrapping `always_ok`. Asserts captured `AgentRequest.context` contains `<athena_context>` with `<recent_run>` referencing Run 1's content. Cross-run API verification: `store.list()` shows Run 1 entries, keyword search returns results for extracted terms.

## Verification

- `cargo test -p ath-orchestrator -- memory::tests::keyword_index_persisted_after_extraction` — passed
- `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle` — passed
- `cargo test --workspace` — 588 passed, 0 failed, 0 regressions

## Deviations

None.

## Known Limitations

- Memory-aware coordinator runs groups sequentially (D014) — parallel memory-aware execution deferred
- Token estimation uses `word_count * 4 / 3` heuristic (D016) — no tokenizer dependency
- Convention/agent profile merge is append-only (D011) — no structural deduplication across runs
- HNSW graph rebuilt from stored vectors on load (D007) — acceptable at current scale

## Follow-ups

None — this is the final assembly slice for M002.

## Files Created/Modified

- `crates/ath-orchestrator/src/memory.rs` — added `index_path` field to `MemoryContext`, `keywords.save()` in `post_run_extraction`, keyword persistence test, two-run end-to-end test (~200 lines total)

## Forward Intelligence

### What the next slice should know
- M002 is complete. All memory subsystems (store, index, observations, extraction, injection, CLI) are wired together and proven by integration tests.
- The `memory.rs` module in `ath-orchestrator` is the integration hub — all memory-aware orchestration lives there, separate from the existing `phase_runner.rs`.

### What's fragile
- `MockBackend::new(sequenced)` responses in `two_run_end_to_end_memory_lifecycle` must match `MockExtractionLlm::with_all_stages()` JSON shapes exactly — if extraction schemas change, this test breaks.
- `ContextInjector::read_recent_run` looks for `viking://runs/*/summary` URIs — changing the URI naming convention breaks injection.

### Authoritative diagnostics
- `keyword.json` at `memory.index_path` after a run — presence = persistence worked, absence = check logs for `"keyword index save failed"`
- `store.list()` on the Viking store — empty after extraction means extraction failed silently (check `"Extraction stage failed"` in logs)

### What assumptions changed
- None — T01 and T02 went as planned, no surprises.
