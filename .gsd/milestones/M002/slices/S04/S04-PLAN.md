# S04: Orchestrator Integration (Observation + Injection)

**Goal:** The orchestrator captures observations during phase runs and injects relevant memory context into agent prompts within a configurable token budget.
**Demo:** An integration test proves: (1) running a phase produces observation events, (2) after extraction, a subsequent agent call receives injected context from prior memory, (3) token budget is respected, (4) existing orchestrator tests pass unchanged.

## Must-Haves

- `ContextInjector` in `ath-memory` that reads `VikingStore` + `KeywordIndex`, builds a context string within a configurable token budget, with section-based progressive disclosure
- `ExtractionLlm` adapter that wraps `AgentBackend` for use in the extraction pipeline
- Observation recording hooks in `execute_phase_tasks_with_progress` that emit `ObservationType` events into an `ObservationBuffer`
- Context injection wiring that populates `AgentRequest.context` from `ContextInjector` before agent calls
- Post-run extraction hook in coordinator that calls `MemoryExtractor::extract_all` after `run_plan` completes
- All memory operations are fail-soft — errors are logged, never fail the orchestrator run
- All ~39 existing orchestrator tests pass without modification (backward-compatible signatures)

## Proof Level

- This slice proves: integration — memory subsystem wired into orchestrator lifecycle
- Real runtime required: no (mock agents sufficient)
- Human/UAT required: no

## Verification

- `cargo test -p ath-memory -- inject` — ContextInjector unit tests (budget enforcement, section ordering, empty store, keyword search integration)
- `cargo test -p ath-orchestrator` — all existing tests still pass (backward compatibility)
- `cargo test -p ath-orchestrator -- memory` — new integration tests proving observation capture + context injection
- `cargo check --workspace` — no warnings, no regressions
- Diagnostic check: verify `tracing::warn` emitted for memory errors by inspecting test output with `RUST_LOG=warn` — grep for "Memory operation failed" or "Context injection skipped" in test stderr to confirm fail-soft behavior surfaces structured warnings

## Observability / Diagnostics

- Runtime signals: `tracing::info` on context injection (tokens injected, sections included), `tracing::warn` on memory errors (observation write failure, injection failure), `tracing::info` on extraction completion with succeeded/failed counts
- Inspection surfaces: `.ath/memory/observations/<run-uuid>.jsonl` for captured observations, `ExtractionResult.succeeded/failed` for extraction diagnostics
- Failure visibility: Memory errors converted to `tracing::warn` with full error context — grep for "Memory operation failed" or "Context injection skipped"
- Redaction constraints: none (no secrets in memory content)

## Integration Closure

- Upstream surfaces consumed: `VikingStore` + `KeywordIndex` from S01, `ObservationBuffer` + `ObservationType` from S02, `MemoryExtractor` + `ExtractionLlm` from S03
- New wiring introduced: `ath-memory::inject::ContextInjector`, `ath-orchestrator` dependency on `ath-memory`, `MemoryContext` optional config struct threading memory through orchestrator
- What remains before the milestone is truly usable end-to-end: S05 (CLI + config), S06 (full two-run end-to-end test)

## Tasks

- [x] **T01: Build ContextInjector in ath-memory** `est:45m`
  - Why: The injector is the core new component — it reads from the store + keyword index and builds a budget-constrained context string. Lives entirely in `ath-memory` with no orchestrator dependency. Must be proven independently before wiring.
  - Files: `crates/ath-memory/src/inject/mod.rs`, `crates/ath-memory/src/inject/injector.rs`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/src/error.rs`
  - Do: Create `inject` submodule. Implement `ContextInjector` with `build_context(query: &str, token_budget: usize) -> InjectedContext`. Sections: project identity (from `viking://project/*`), semantic search results (keyword hits joined with store reads), agent notes, recent run summary. Token budget enforced via word-count heuristic (`words * 4 / 3`). `InjectedContext` has `text: String` and `estimated_tokens: usize`. Add `InjectionConfig` with per-section budget allocations (defaults from spec: total=4000, project=200, semantic=2500, agent=300, recent=500). All reads are fail-soft — missing URIs produce empty sections, not errors.
  - Verify: `cargo test -p ath-memory -- inject` — budget enforcement, section ordering, empty store, keyword hit integration, partial store (some URIs missing)
  - Done when: `ContextInjector` builds context strings within budget from store+keyword data, with ≥8 unit tests passing

- [x] **T02: Wire observation recording, context injection, and extraction into orchestrator** `est:60m`
  - Why: This is the integration task — threads memory through the orchestrator without breaking existing signatures. Covers observation hooks, context injection, ExtractionLlm adapter, and post-run extraction.
  - Files: `crates/ath-orchestrator/Cargo.toml`, `crates/ath-orchestrator/src/memory.rs`, `crates/ath-orchestrator/src/phase_runner.rs`, `crates/ath-orchestrator/src/coordinator.rs`, `crates/ath-orchestrator/src/lib.rs`
  - Do: (1) Add `ath-memory` + `tracing` deps to orchestrator. (2) Create `memory.rs` module with: `MemoryContext` struct holding `Arc<ObservationBuffer>`, `ContextInjector` ref, and config; `BackendLlmAdapter` implementing `ExtractionLlm` by wrapping an `AgentBackend`. (3) Add `_with_memory` variants: `execute_phase_tasks_with_memory` wraps `execute_phase_tasks_with_progress` to record observations (`AgentRequest`, `AgentResponse`, `ReviewVerdict`, `RetryStarted` events) and inject context before each `AgentRequest` construction. (4) Add `run_phase_with_memory` that delegates to `run_phase_with_progress` but wraps the task execution with memory hooks. (5) Add `AgentCoordinator::run_plan_with_memory` that calls the memory-aware phase runner and triggers `extract_all` post-run. (6) Existing `run_phase`/`run_plan` signatures unchanged — they remain the no-memory path. All memory errors wrapped in `tracing::warn`, never propagated as `PhaseRunnerError`.
  - Verify: `cargo test -p ath-orchestrator` — all existing tests pass unchanged. `cargo check --workspace` clean.
  - Done when: New `_with_memory` entry points compile, existing tests pass, `BackendLlmAdapter` bridges `AgentBackend` → `ExtractionLlm`

- [x] **T03: Integration test proving observation capture and context injection** `est:45m`
  - Why: Proves the slice's demo claim — that a phase run produces observations and a subsequent call gets injected context. This is the risk-retiring test.
  - Files: `crates/ath-orchestrator/src/memory.rs` (test module)
  - Do: Write integration tests in the `memory` module: (1) `memory_context_records_observations` — run a phase via `run_plan_with_memory` with MockBackend, verify `ObservationBuffer` contains `AgentRequest` and `AgentResponse` events with correct phase_id. (2) `memory_context_injects_context_from_store` — pre-populate `VikingStore` with a project identity entry, run a phase via memory-aware path, verify the `AgentRequest` received by MockBackend has `context: Some(...)` containing the store content. (3) `memory_errors_do_not_fail_run` — inject a broken store path, verify run completes successfully with `tracing::warn` (memory is fail-soft). (4) `backend_llm_adapter_bridges_to_extraction_llm` — unit test proving the adapter correctly wraps `AgentBackend::send()`.
  - Verify: `cargo test -p ath-orchestrator -- memory` — all 4+ tests pass
  - Done when: Integration tests prove observation recording, context injection, fail-soft behavior, and LLM adapter bridging

## Files Likely Touched

- `crates/ath-memory/src/inject/mod.rs`
- `crates/ath-memory/src/inject/injector.rs`
- `crates/ath-memory/src/lib.rs`
- `crates/ath-memory/src/error.rs`
- `crates/ath-orchestrator/Cargo.toml`
- `crates/ath-orchestrator/src/memory.rs`
- `crates/ath-orchestrator/src/phase_runner.rs`
- `crates/ath-orchestrator/src/coordinator.rs`
- `crates/ath-orchestrator/src/lib.rs`
