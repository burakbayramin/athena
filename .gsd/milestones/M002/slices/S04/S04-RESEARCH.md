# S04: Orchestrator Integration (Observation + Injection) — Research

**Date:** 2026-03-14

## Summary

This slice bridges `ath-memory` into `ath-orchestrator` — the two subsystems that currently don't know each other. Two things need to happen: (1) the orchestrator emits observation events into an `ObservationBuffer` during `run_phase`, and (2) a `ContextInjector` reads from the Viking store + keyword index to populate the `AgentRequest.context` field before each agent call. Post-run extraction is triggered from the coordinator after `run_plan` completes.

The integration surface is well-bounded. `AgentRequest` already has an `Option<String>` `context` field that's always set to `None` today — the injector populates it. Observation recording maps 1:1 to existing `ProgressEvent` lifecycle points in `run_phase_with_progress`. The main risk is keeping the orchestrator's clean separation intact while threading memory dependencies through it.

The approach should add an `inject` module to `ath-memory` (ContextInjector lives in the memory crate, not the orchestrator) and then add thin integration hooks in the orchestrator that are optional — if no memory layer is provided, behavior is unchanged. This keeps `ath-orchestrator` loosely coupled: it takes an optional memory context, not a hard dependency on memory infrastructure.

## Recommendation

**Build two new pieces, wire them with optional parameters:**

1. **`ath-memory::inject::ContextInjector`** — reads from `VikingStore` + `KeywordIndex`, builds a context string within a token budget. Lives entirely in `ath-memory`. Configurable budget sections (project identity, user instructions, semantic results, agent notes, recent run).

2. **Orchestrator hooks** — modify `execute_phase_tasks_with_progress` and `run_phase_with_progress` signatures to accept optional observation/memory parameters. Observation recording wraps existing `ProgressEvent` emit points. Context injection happens before `AgentRequest` construction (where `context: None` is today).

3. **Post-run extraction** — `AgentCoordinator::run_plan_with_progress` gets an optional post-run hook that calls `MemoryExtractor::extract_all`. The `ExtractionLlm` adapter bridges `AgentBackend` → `ExtractionLlm` trait.

Use `Option`-based parameters so existing tests pass without modification and the memory layer is opt-in.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Token counting for budget | Simple `str.split_whitespace().count()` heuristic | At this stage, word-count ÷ 0.75 ≈ tokens is sufficient. Avoid tiktoken dependency. Can swap later. |
| Observation event mapping | `ObservationType` enum from S02 | Already has all 7 variants matching orchestrator lifecycle events exactly |
| Store/index access | `VikingStore`, `KeywordIndex` from S01 | Already tested with persistence; just need to query from injector |
| LLM abstraction | `ExtractionLlm` trait from S03 | Provider-agnostic; just need a thin adapter wrapping `AgentBackend` |

## Existing Code and Patterns

- `crates/ath-orchestrator/src/phase_runner.rs` — `execute_phase_tasks_with_progress` builds `AgentRequest` with `context: None` at line ~380. This is the injection point. Also emits `ProgressEvent::TaskStarted`, `TaskCompleted`, `ReviewStarted`, etc. — these are the observation hook points.
- `crates/ath-orchestrator/src/coordinator.rs` — `AgentCoordinator::run_plan_with_progress` is the top-level entry point. Post-run extraction hooks after the `Ok(results)` return path.
- `crates/ath-orchestrator/src/progress.rs` — `SharedProgressObserver` pattern (trait object behind `Arc`) is the established pattern for optional callbacks. Memory hooks should follow the same `Option<Arc<dyn Trait>>` pattern.
- `crates/ath-types/src/agent.rs` — `AgentRequest.context: Option<String>` already exists and is used by no one. The injector populates this.
- `crates/ath-memory/src/observe/buffer.rs` — `ObservationBuffer` is `Send + Sync` with `Mutex`-guarded internals. Safe to pass into async orchestrator code via `Arc`.
- `crates/ath-memory/src/observe/types.rs` — `ObservationType` has 7 variants with `phase_id: Option<u32>` on each. Maps directly to orchestrator lifecycle.
- `crates/ath-memory/src/extract/types.rs` — `ExtractionLlm` trait: `async fn complete(&self, prompt: &str, json_schema: Option<&serde_json::Value>) -> Result<String, MemoryError>`. The adapter wraps `AgentBackend::send()`.
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::search(query, top_k) -> Vec<KeywordHit>` with `KeywordHit { uri: String, score: f32 }`. The injector joins hits with `VikingStore::read()`.
- `crates/ath-orchestrator/Cargo.toml` — currently no dependency on `ath-memory` or `tracing`. Adding `ath-memory` as optional dep behind a feature flag is one approach, but may be overengineered — a simple required dep is fine since memory is a core M002 feature.

## Constraints

- **`ath-orchestrator` has no `tracing` dependency** — observation hooks should use the `ObservationBuffer::record()` API directly, not tracing spans. Or add `tracing` as a dep (already workspace-level).
- **`MemoryIndex` requires `&mut self`** — if the injector needs vector search, it must hold a `Mutex<MemoryIndex>`. But S04 spec says keyword fallback only (no embedding API yet), so `KeywordIndex` (also `&mut self` for add, but `&self` for search) suffices.
- **`KeywordIndex::search` takes `&self`** — safe for concurrent reads. No mutex needed for injection reads.
- **`VikingStore` methods take `&self`** — safe for concurrent reads. No mutex needed.
- **`MemoryExtractor` takes `&'a mut KeywordIndex`** — extraction is a post-run sequential operation, so `&mut` access is fine (no concurrent use during extraction).
- **`execute_phase_tasks` is called within `run_phase_with_progress` loop** — observation recording must happen at the same call sites, not in a separate layer.
- **`PhaseRunnerError` is `Clone + PartialEq`** — new error variants must satisfy these bounds. Memory errors are `!Clone` (contain `std::io::Error`), so we should convert to string before wrapping.
- **Existing tests construct `AgentRegistry` and call `run_phase` directly** — signature changes must be backward-compatible or all ~20 tests need updating. Prefer adding a new method or using default parameters.
- **`run_phase`/`run_phase_with_progress` take generic closures for `write_files` and `available`** — adding more closure params would make signatures unwieldy. Better to use a config/context struct.

## Common Pitfalls

- **Breaking existing tests with signature changes** — There are ~20 tests in `phase_runner.rs` and ~10 in `coordinator.rs` that call `run_phase` or `AgentCoordinator::run_plan`. Adding required parameters would break all of them. Solution: use an optional `MemoryContext` struct parameter, or add new `_with_memory` variants alongside existing functions.
- **Making memory a hard failure path** — Memory errors during observation recording or context injection should never fail the orchestrator run. All memory operations should be wrapped in `if let Err(e) = ... { tracing::warn!(...) }` patterns. D012 already establishes this principle for extraction.
- **Token budget off-by-one** — Word-count heuristic is approximate. Over-counting is better than under-counting (safer to inject slightly less than blow the budget). Use `words * 4/3` as the token estimate, which slightly overestimates.
- **Circular dependency** — `ath-orchestrator` → `ath-memory` → `ath-types` and `ath-orchestrator` → `ath-types` is fine (diamond, not cycle). But `ath-memory` must NOT depend on `ath-orchestrator`.
- **Thread safety for ObservationBuffer in parallel dispatch** — `ObservationBuffer` is `Send + Sync` by design (Mutex internals). Wrapping in `Arc` for `JoinSet::spawn` is straightforward. The buffer tolerates lock poisoning.

## Open Risks

- **Signature churn in `run_phase_with_progress`** — This function already has 6 parameters. Adding memory context as parameter #7 is getting unwieldy. A `PhaseRunnerConfig` struct might be cleaner but is a larger refactor. Need to decide: another parameter, or wrap everything in a struct.
- **Token budget accuracy** — Word-count heuristic may under- or over-estimate tokens for code-heavy content. Acceptable for M002 scope; can add tiktoken-like counting later.
- **ExtractionLlm adapter needs an AgentKind to select which backend to use** — The adapter must pick a specific agent (e.g., Claude) for extraction calls. This means either hardcoding a preference or making it configurable. Leaning toward taking the first available agent from the registry.
- **Integration test complexity** — Proving "phase run produces observations AND subsequent call gets injected context" requires either a two-phase test (run → extract → inject → verify) or carefully staged mocks. The `MockBackend` infrastructure is well-established, so mocking is feasible.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust | (core language, no skill needed) | N/A |

No external frameworks or services are involved in this slice — it's pure Rust internal wiring between existing crates. No skills to discover.

## Sources

- `docs/specs/memory-layer.md` §4.5 — Context Injector spec with `InjectedContext`, progressive disclosure strategy, and token budget defaults
- `docs/specs/memory-layer.md` §10 — Token budget config (`total_budget=4000`, section allocations)
- S01 Summary — `VikingStore`, `KeywordIndex`, `MemoryIndex` APIs and constraints (`&mut self` on index, `&self` on store and keyword search)
- S02 Summary — `ObservationBuffer` is `Send + Sync`, `ObservationType` has 7 variants, `ObservationReader::read_run()` entry point
- S03 Summary — `ExtractionLlm` trait boundary, `MemoryExtractor::extract_all(run_id, observations_root)` signature, per-stage error isolation
- D010 — `ExtractionLlm` defined in `ath-memory`, not `ath-agents`; downstream (this slice) provides real implementation
- D012 — `extract_all` isolates per-stage errors; failed stages don't block others
