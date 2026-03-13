---
id: S04
parent: M002
milestone: M002
provides:
  - ContextInjector in ath-memory — budget-constrained XML prompt context from VikingStore + KeywordIndex
  - MemoryContext struct threading memory state through orchestrator lifecycle
  - BackendLlmAdapter bridging AgentBackend → ExtractionLlm for post-run extraction
  - Memory-aware phase execution with observation recording and context injection (run_plan_with_memory)
  - Post-run extraction pipeline triggered as fail-soft epilogue
  - 11 integration tests proving observation capture, context injection, fail-soft resilience, and LLM adapter bridging
requires:
  - slice: S01
    provides: VikingStore + KeywordIndex for memory reads and keyword search
  - slice: S02
    provides: ObservationBuffer + ObservationType for capturing runtime events
  - slice: S03
    provides: MemoryExtractor + ExtractionLlm trait for post-run extraction pipeline
affects:
  - S06
key_files:
  - crates/ath-memory/src/inject/mod.rs
  - crates/ath-memory/src/inject/injector.rs
  - crates/ath-orchestrator/src/memory.rs
  - crates/ath-orchestrator/Cargo.toml
key_decisions:
  - Memory-aware code lives entirely in memory.rs — phase_runner.rs untouched, zero risk to 135 existing tests (D013)
  - Parallel groups run sequentially when memory enabled — deterministic observation ordering (D014)
  - XML wrapper format with <athena_context> and named section tags for injected context (D015)
  - Token estimation uses word_count * 4 / 3 heuristic — swappable later (D016)
patterns_established:
  - Fail-soft memory wrappers — record_observation and inject_context never return errors, log warnings
  - BackendLlmAdapter — thin async wrapper mapping AgentBackend::send to ExtractionLlm::complete
  - MemoryContext as opt-in bundle — all memory state in one struct, threaded into run_plan_with_memory
  - Section-based progressive disclosure with per-section budget caps and total budget enforcement
  - CapturingBackend test pattern — wraps AgentBackend to intercept requests for post-hoc verification
observability_surfaces:
  - tracing::info "Context built" with tokens and sections_included
  - tracing::info "Observation buffer flushed to disk" with run_id and count
  - tracing::info "Extraction pipeline completed" with succeeded/failed counts
  - tracing::warn "Memory operation failed" / "Context section skipped" for any subsystem error
  - InjectedContext.sections_included and .estimated_tokens for programmatic inspection
  - Observation JSONL files at observations_root/<run_id>.jsonl
drill_down_paths:
  - .gsd/milestones/M002/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M002/slices/S04/tasks/T03-SUMMARY.md
duration: ~65min
verification_result: passed
completed_at: 2026-03-14
---

# S04: Orchestrator Integration (Observation + Injection)

**Memory subsystem wired into orchestrator — phases capture observations, agent prompts receive budget-constrained context injection, post-run extraction produces memory entries, all fail-soft.**

## What Happened

Three tasks built the integration layer between the memory subsystem (S01–S03) and the orchestrator:

**T01** created `ContextInjector` in `ath-memory` — reads VikingStore + KeywordIndex to assemble a prompt context string within configurable token budgets. Four sections assembled in priority order: project identity, semantic search results, agent notes, recent run summary. Each section capped to its per-section budget and then against remaining total. Output wrapped in `<athena_context>` XML with named section tags. 13 unit tests.

**T02** built the orchestrator integration in a new `memory.rs` module. `MemoryContext` bundles all memory state (buffer, store, index, configs). `BackendLlmAdapter` bridges `AgentBackend` → `ExtractionLlm` for extraction. `run_plan_with_memory` mirrors the existing `run_plan_with_progress` lifecycle but adds hooks: context injection before each agent call, observation recording after each response/review/retry, and post-run extraction as a fail-soft epilogue. Existing `run_phase`/`run_plan` signatures unchanged — 135 existing tests pass without modification.

**T03** added the risk-retiring integration tests. `CapturingBackend` wrapper intercepts agent requests for post-hoc inspection. Tests prove: observations written to JSONL and readable via `ObservationReader`, context injection populates `AgentRequest.context` with store content, broken store paths don't crash the run, and `BackendLlmAdapter` correctly bridges both schema/no-schema extraction calls.

## Verification

- `cargo test -p ath-memory -- inject` — 13/13 pass (ContextInjector unit tests)
- `cargo test -p ath-orchestrator` — 146/146 pass (135 existing + 11 memory)
- `cargo test -p ath-orchestrator -- memory` — 11/11 pass (observation capture, context injection, fail-soft, adapter)
- `cargo check --workspace` — clean (only pre-existing warnings in observe/buffer.rs)

## Deviations

- Plan suggested modifying `phase_runner.rs` and `coordinator.rs` with memory hooks — instead all memory-aware code lives in `memory.rs` with `pub(crate)` field access. More code duplication but zero risk to existing tests.
- Memory-aware coordinator runs parallel groups sequentially (plan didn't specify) — simplifies observation ordering.
- Review requests not recorded as separate AgentRequest observations — the reviewer backend runs outside the memory-aware dispatch path. Tests assert ≥3 observations (request + response + review verdict) which matches actual behavior.

## Known Limitations

- `merge_contribution` and `write_files_to_dir` helpers duplicated between `phase_runner.rs` and `memory.rs` — deferred extraction to avoid modifying phase_runner.rs
- Memory-aware parallel execution not implemented — phases in parallel groups run sequentially when memory is enabled (non-memory path retains full parallelism)
- Tracing subscribers not initialized in tests — observability signals are wired for runtime, verified via behavior assertions in tests rather than log capture

## Follow-ups

- S06 will prove the full two-run end-to-end lifecycle — this slice proved the individual hooks work
- Extract duplicated helpers (`merge_contribution`, `write_files_to_dir`) to shared utility if memory.rs diverges further

## Files Created/Modified

- `crates/ath-memory/src/inject/mod.rs` — module root with re-exports
- `crates/ath-memory/src/inject/injector.rs` — ContextInjector, InjectedContext, InjectionConfig, helpers, 13 tests
- `crates/ath-memory/src/lib.rs` — added pub mod inject and re-exports
- `crates/ath-orchestrator/src/memory.rs` — MemoryContext, BackendLlmAdapter, memory-aware execution, 11 tests
- `crates/ath-orchestrator/Cargo.toml` — added ath-memory and tracing dependencies
- `crates/ath-orchestrator/src/lib.rs` — added pub mod memory
- `crates/ath-orchestrator/src/coordinator.rs` — 3 struct fields changed to pub(crate)

## Forward Intelligence

### What the next slice should know
- `MemoryContext` is the single entry point for all memory-aware orchestration — construct it with a store, index, buffer, and configs, pass to `run_plan_with_memory`
- `InjectionConfig` defaults: total=4000, project=200, semantic=2500, agent=300, run=500 tokens — these should be exposed via config.toml in S05
- The extraction pipeline needs a real `AgentBackend` to work — `BackendLlmAdapter` selects from the coordinator's registry by discriminant matching (Claude > Gemini > Codex priority)

### What's fragile
- `memory.rs` duplicates the phase execution loop from `phase_runner.rs` — if phase_runner changes (new states, different flow), memory.rs must be updated in lockstep
- `BackendLlmAdapter` assumes at least one backend in the registry is usable for extraction — if the registry is empty or all backends are circuit-broken, extraction silently fails

### Authoritative diagnostics
- `cargo test -p ath-orchestrator -- memory` — the 11 tests here are the ground truth for memory integration health
- Observation JSONL files at `<observations_root>/<run_id>.jsonl` — raw event log for any run
- `InjectedContext.sections_included` — programmatic check for which sections were populated

### What assumptions changed
- Assumed phase_runner.rs would be modified with Option<&MemoryContext> — instead kept all code in memory.rs for isolation, which means duplication but better backward compatibility
- Assumed review requests would be captured as observations — they aren't, because reviewer dispatch doesn't go through the memory-aware path
