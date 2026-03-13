---
id: T02
parent: S04
milestone: M002
provides:
  - MemoryContext struct threading memory state through orchestrator
  - BackendLlmAdapter bridging AgentBackend → ExtractionLlm
  - Memory-aware phase execution with observation recording and context injection
  - Post-run extraction pipeline triggered from coordinator
  - run_plan_with_memory entry point on AgentCoordinator
key_files:
  - crates/ath-orchestrator/src/memory.rs
  - crates/ath-orchestrator/Cargo.toml
  - crates/ath-orchestrator/src/lib.rs
  - crates/ath-orchestrator/src/coordinator.rs
key_decisions:
  - All memory-aware code lives in memory.rs — phase_runner.rs and coordinator.rs untouched beyond field visibility change
  - Memory-aware phase execution duplicates the run_phase loop with memory hooks rather than modifying existing functions — preserves all 135 existing tests unchanged
  - Coordinator fields changed to pub(crate) to allow memory.rs impl block on AgentCoordinator
  - Memory-aware coordinator runs phases sequentially within parallel groups — observation ordering is deterministic, parallel memory-aware execution deferred
  - Extraction backend selection uses AgentKind discriminant matching with Claude/Gemini/Codex priority order
patterns_established:
  - Fail-soft memory wrappers — record_observation and inject_context never return errors, log warnings instead
  - BackendLlmAdapter pattern — thin async wrapper mapping AgentBackend::send to ExtractionLlm::complete with MemoryError mapping
  - MemoryContext as opt-in struct — all memory state bundled into one struct, threaded by value into run_plan_with_memory
  - Post-run extraction as fail-soft epilogue — flush then extract, all errors logged but never propagated to PhaseRunnerError
observability_surfaces:
  - tracing::info on "Observation buffer flushed to disk" with run_id and count
  - tracing::info on "Context injection" with tokens and sections
  - tracing::info on "Extraction pipeline completed" with succeeded/failed counts
  - tracing::warn on "Memory operation failed" for any memory subsystem error
  - tracing::info on "Context injection skipped" when no relevant context found
  - Observation JSONL files at observations_root/<run_id>.jsonl
duration: 25min
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Wire observation recording, context injection, and extraction into orchestrator

**Built `MemoryContext`, `BackendLlmAdapter`, memory-aware phase execution, and `run_plan_with_memory` coordinator entry point — all 135 existing tests pass unchanged, 9 new memory tests pass.**

## What Happened

Created `crates/ath-orchestrator/src/memory.rs` containing:

1. **`MemoryContext`** — bundles `ObservationBuffer`, `VikingStore`, `KeywordIndex` (Arc<Mutex>), observations root path, run ID, and injection/extraction configs into one opt-in struct.

2. **`BackendLlmAdapter`** — implements `ExtractionLlm` by wrapping `Arc<dyn AgentBackend>` + `AgentKind`, constructing `AgentRequest`s for extraction prompts and mapping `AgentError` → `MemoryError::ExtractionError`.

3. **Helper functions** — `record_observation` (infallible wrapper around buffer.record()), `inject_context` (builds context via `ContextInjector`, returns `Option<String>`, logs on empty/error).

4. **`run_phase_with_memory`** — mirrors the existing `run_phase_with_progress` lifecycle but adds memory hooks: before each agent call injects context into `AgentRequest.context`, after each response records `AgentRequest`/`AgentResponse` observations, after review records `ReviewVerdict`, on retry records `RetryStarted`.

5. **`AgentCoordinator::run_plan_with_memory`** — impl block extension on the coordinator that mirrors `run_plan_with_progress` but uses `run_phase_with_memory` for each phase, then calls `post_run_extraction` (flushes buffer to disk, runs `MemoryExtractor::extract_all` via `BackendLlmAdapter`).

The design choice to duplicate the phase execution loop rather than modify existing functions was deliberate — it keeps all 135 existing tests passing with zero changes to existing code. Only the coordinator struct fields were changed from private to `pub(crate)`.

## Verification

- `cargo test -p ath-orchestrator` — **144 passed** (135 existing + 9 new), 0 failed
- `cargo test -p ath-orchestrator -- memory` — **9 passed** (unit + integration tests for memory module)
- `cargo test -p ath-memory -- inject` — **13 passed** (ContextInjector tests from T01)
- `cargo check --workspace` — clean, no new warnings
- Observation file verified on disk in `memory_run_plan_produces_observations` test — contains AgentRequest, AgentResponse, and ReviewVerdict events

## Diagnostics

- Grep for "Memory operation failed" in orchestrator logs to find any memory subsystem failures
- Grep for "Context injection" to see what context was injected (tokens, sections)
- Grep for "Extraction pipeline completed" to see extraction results (succeeded/failed counts)
- Grep for "Observation buffer flushed" to verify observations were persisted
- Check `.ath/memory/observations/<run-uuid>.jsonl` for raw observation events
- `ExtractionResult.succeeded/failed` vectors report per-stage extraction diagnostics

## Deviations

- Plan suggested potentially modifying `phase_runner.rs` with `Option<&MemoryContext>` parameter — chose to keep all hooks in `memory.rs` instead, which means more code duplication but zero risk to existing tests
- Memory-aware coordinator runs phases sequentially within parallel groups (plan didn't specify this constraint) — simplifies observation ordering at the cost of losing parallelism when memory is enabled

## Known Issues

- `merge_contribution` helper is duplicated between `phase_runner.rs` and `memory.rs` — could be extracted to a shared utility but deferred to avoid modifying phase_runner.rs
- `write_files_to_dir` similarly duplicated — same reasoning
- Memory-aware parallel execution is not implemented — phases in parallel groups run sequentially when memory is enabled

## Files Created/Modified

- `crates/ath-orchestrator/src/memory.rs` — new module: MemoryContext, BackendLlmAdapter, memory-aware phase execution, coordinator extension, 9 tests
- `crates/ath-orchestrator/Cargo.toml` — added `ath-memory` and `tracing` dependencies
- `crates/ath-orchestrator/src/lib.rs` — added `pub mod memory` export
- `crates/ath-orchestrator/src/coordinator.rs` — changed 3 struct fields from private to `pub(crate)`
