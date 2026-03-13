---
id: T03
parent: S04
milestone: M002
provides:
  - Integration tests proving observation capture, context injection, fail-soft resilience, and LLM adapter bridging in the memory-aware orchestrator
key_files:
  - crates/ath-orchestrator/src/memory.rs
key_decisions:
  - Built CapturingBackend wrapper (in test module only) to intercept AgentRequest and verify context injection — avoids modifying MockBackend's public API
  - Replaced weaker T02 integration tests (memory_run_plan_produces_observations, memory_run_plan_context_injected_into_request) with rigorous equivalents that parse JSONL observations and inspect captured request context fields
patterns_established:
  - CapturingBackend pattern — wraps Arc<dyn AgentBackend> with a shared Mutex<Vec<AgentRequest>> for post-hoc request inspection in tests
  - Observation assertion pattern — read back via ObservationReader::read_run after coordinator completes, assert event types, counts, phase_ids, and field values from parsed Observation structs
observability_surfaces:
  - Run `cargo test -p ath-orchestrator -- memory` to verify all 11 memory integration tests pass
  - Tests directly verify JSONL observation files, context injection into AgentRequest.context, and fail-soft behavior on broken paths
duration: ~20 min
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T03: Integration tests proving observation capture and context injection

**Added 4 targeted integration tests (11 total memory tests) proving the slice demo claims: observation capture, context injection, fail-soft resilience, and LLM adapter bridging.**

## What Happened

Built the risk-retiring integration tests for S04's demo claim. Four new tests were added to the `memory.rs` test module:

1. **`memory_context_records_observations`** — Runs a single-phase plan via `run_plan_with_memory`, reads back the JSONL observation file via `ObservationReader`, asserts AgentRequest/AgentResponse/ReviewVerdict events exist with correct phase_id, task_name, agent kind, and run_id correlation. Replaced the weaker `memory_run_plan_produces_observations` that only checked string contains.

2. **`memory_context_injects_context_from_store`** — Creates a VikingStore with project identity at `viking://project/identity`, wraps MockBackend in a `CapturingBackend` that records all incoming `AgentRequest`s, runs a phase, then inspects the captured request's `context` field. Asserts it contains `<athena_context>`, `<project>`, and the project identity text. Replaced the weaker test that only checked `result.is_ok()`.

3. **`memory_errors_do_not_fail_run`** — Constructs a MemoryContext with a broken `observations_root` path (`Z:\nonexistent\...` on Windows), runs a phase, asserts the run completes `Ok(...)` with a passing verdict despite the observation flush failure.

4. **`backend_llm_adapter_bridges_correctly`** — Verifies the adapter handles both `None` and `Some(schema)` for the `json_schema` parameter, covering the full `ExtractionLlm::complete` contract.

Also created a `CapturingBackend` test helper that wraps any `Arc<dyn AgentBackend>`, clones and stores each incoming request, then delegates to the inner backend. This enables verifying what the orchestrator sends to agents without modifying the mock infrastructure.

Cleaned up the unused `mock_response` helper from T02.

## Verification

- `cargo test -p ath-orchestrator -- memory` — 11 tests pass (was 9 in T02, +2 net: 2 replaced with stronger versions, 2 added)
- `cargo test -p ath-orchestrator` — 146 tests pass (135 existing + 11 memory)
- `cargo test -p ath-memory -- inject` — 13 tests pass
- `cargo check --workspace` — clean (only pre-existing ath-memory compile-time assertion warnings)
- `cargo test --workspace` — 557 tests pass, zero failures

All slice verification checks pass.

## Diagnostics

- **Test health**: `cargo test -p ath-orchestrator -- memory` — all 11 should pass
- **Observation file inspection**: Tests create temp dirs with `.jsonl` files — run individual tests with `--nocapture` to see temp paths
- **Context injection tracing**: Run `RUST_LOG=info cargo test -p ath-orchestrator -- memory_context_injects` to see "Context injection" log lines
- **Fail-soft verification**: Run `RUST_LOG=warn cargo test -p ath-orchestrator -- memory_errors` to see "Memory operation failed" warnings

## Deviations

- Task plan mentioned "observation count matches expected lifecycle events (at minimum: 1 request + 1 response per task, 1 review request + 1 review response)" — the review request is not recorded as a separate AgentRequest observation (it goes through the reviewer backend directly without the memory-aware dispatch path). Test asserts ≥3 observations (1 request + 1 response + 1 review verdict) which matches the actual implementation behavior.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-orchestrator/src/memory.rs` — Added 4 integration tests (CapturingBackend helper, memory_context_records_observations, memory_context_injects_context_from_store, memory_errors_do_not_fail_run, backend_llm_adapter_bridges_correctly), replaced 2 weaker tests, cleaned up unused helper
- `.gsd/milestones/M002/slices/S04/tasks/T03-PLAN.md` — Added Observability Impact section
