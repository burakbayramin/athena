---
estimated_steps: 5
estimated_files: 6
---

# T02: Wire observation recording, context injection, and extraction into orchestrator

**Slice:** S04 — Orchestrator Integration (Observation + Injection)
**Milestone:** M002

## Description

Thread the memory subsystem into `ath-orchestrator` while keeping all existing function signatures unchanged. This task introduces a `memory.rs` module with `MemoryContext` (the optional config struct that threads memory through orchestrator calls), `BackendLlmAdapter` (bridges `AgentBackend` → `ExtractionLlm`), and new `_with_memory` entry points on the coordinator and phase runner.

The critical constraint: all ~39 existing tests must pass without modification. Memory is opt-in via `MemoryContext` — when absent, behavior is identical to the pre-memory code path.

## Steps

1. Add `ath-memory` and `tracing` dependencies to `crates/ath-orchestrator/Cargo.toml`.

2. Create `crates/ath-orchestrator/src/memory.rs` with:
   - `MemoryContext` struct holding:
     - `buffer: Arc<ObservationBuffer>` for observation recording
     - `injector: Option<ContextInjector<'static>>` or store/keywords refs for context injection (needs careful lifetime management — may need owned versions or `Arc` wrapping)
     - `store: Arc<VikingStore>` and `keywords: Arc<std::sync::Mutex<KeywordIndex>>` for extraction
     - `observations_root: PathBuf` for observation persistence
     - `run_id: uuid::Uuid` for correlating observations
   - `BackendLlmAdapter` struct wrapping `Arc<dyn AgentBackend>` + `AgentKind`:
     - Implement `ExtractionLlm` trait: constructs an `AgentRequest` with the extraction prompt, calls `backend.send()`, returns `response.content`
     - Error mapping: `AgentError` → `MemoryError::ExtractionError`
   - Helper function `record_observation(buffer: &ObservationBuffer, event: ObservationType)` — wraps the call in a warn-on-error handler so memory never crashes the orchestrator
   - Helper function `inject_context(store: &VikingStore, keywords: &KeywordIndex, query: &str, config: &InjectionConfig) -> Option<String>` — calls `ContextInjector::build_context`, returns `None` on empty/error with `tracing::warn`

3. Modify `crates/ath-orchestrator/src/coordinator.rs`:
   - Add `pub async fn run_plan_with_memory(&self, plan: &ExecutionPlan, observer: Option<SharedProgressObserver>, memory: MemoryContext) -> Result<Vec<PhaseRecord>, PhaseRunnerError>` method
   - This method: calls the memory-aware phase runner for each phase, wraps observation buffer flush + extraction in a post-run block
   - Post-run: flush observation buffer to disk, call `MemoryExtractor::extract_all` via `BackendLlmAdapter`, log `ExtractionResult` — all in a fail-soft `if let Err(e)` wrapper
   - Pick the first available backend from the registry for extraction calls

4. Add memory-aware wrappers in `crates/ath-orchestrator/src/phase_runner.rs` (or keep them in `memory.rs` to avoid modifying phase_runner.rs at all — prefer the approach that minimizes changes to existing files):
   - Before each `AgentRequest` construction: call `inject_context` and set `context` field
   - After each `AgentResponse`: record `ObservationType::AgentRequest` and `ObservationType::AgentResponse` observations
   - After review verdicts: record `ObservationType::ReviewVerdict`
   - On retry: record `ObservationType::RetryStarted`
   - Strategy decision: either (a) add a new `execute_phase_tasks_with_memory` that wraps `execute_phase_tasks_with_progress` with pre/post hooks, or (b) modify `execute_phase_tasks_with_progress` to accept `Option<&MemoryContext>` with a default path — choose whichever keeps existing tests passing with zero changes

5. Export `memory` module from `crates/ath-orchestrator/src/lib.rs`.

## Must-Haves

- [ ] `ath-orchestrator` compiles with `ath-memory` dependency
- [ ] `BackendLlmAdapter` implements `ExtractionLlm` trait correctly
- [ ] `MemoryContext` struct provides all memory state needed by orchestrator
- [ ] Observation recording hooks exist at all lifecycle points (AgentRequest, AgentResponse, ReviewVerdict, RetryStarted)
- [ ] Context injection populates `AgentRequest.context` from `ContextInjector`
- [ ] Post-run extraction triggered from coordinator with fail-soft error handling
- [ ] All existing `cargo test -p ath-orchestrator` tests pass unchanged

## Verification

- `cargo test -p ath-orchestrator` — all existing tests pass (zero modifications to existing tests)
- `cargo check --workspace` — clean compilation

## Observability Impact

- Signals added: `tracing::info` for context injection stats, `tracing::warn` for memory failures, `tracing::info` for extraction completion
- How a future agent inspects this: grep orchestrator logs for "Memory operation failed", "Context injection", "Extraction pipeline completed"
- Failure state exposed: memory errors logged with full error context but never propagated to `PhaseRunnerError`

## Inputs

- T01 output — `ContextInjector`, `InjectedContext`, `InjectionConfig` from `ath-memory::inject`
- `crates/ath-memory/src/observe/buffer.rs` — `ObservationBuffer` API (`record()`, `flush()`)
- `crates/ath-memory/src/observe/types.rs` — `ObservationType` variants
- `crates/ath-memory/src/extract/pipeline.rs` — `MemoryExtractor::extract_all` signature
- `crates/ath-memory/src/extract/types.rs` — `ExtractionLlm` trait signature
- `crates/ath-agents/src/backend.rs` — `AgentBackend::send()` signature
- S02 Summary — `ObservationBuffer` is `Send + Sync`, poison-recovering
- S03 Summary — `ExtractionResult` reporting pattern, per-stage error isolation

## Expected Output

- `crates/ath-orchestrator/Cargo.toml` — added `ath-memory` and `tracing` dependencies
- `crates/ath-orchestrator/src/memory.rs` — `MemoryContext`, `BackendLlmAdapter`, helper functions
- `crates/ath-orchestrator/src/coordinator.rs` — added `run_plan_with_memory` method
- `crates/ath-orchestrator/src/phase_runner.rs` — possibly modified for injection/observation hooks (or hooks may live entirely in `memory.rs`)
- `crates/ath-orchestrator/src/lib.rs` — export `memory` module
