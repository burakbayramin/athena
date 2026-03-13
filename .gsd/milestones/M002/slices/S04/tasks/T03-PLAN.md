---
estimated_steps: 4
estimated_files: 2
---

# T03: Integration test proving observation capture and context injection

**Slice:** S04 — Orchestrator Integration (Observation + Injection)
**Milestone:** M002

## Description

Write the integration tests that prove the slice's demo claim: a phase run produces observations, and a subsequent call receives injected context. These tests exercise the full `MemoryContext` → orchestrator → observation/injection path using `MockBackend` infrastructure already established in the orchestrator test suite.

This is the risk-retiring test for the "integration complexity" proof strategy item in the milestone roadmap.

## Steps

1. Add integration tests in `crates/ath-orchestrator/src/memory.rs` (test module) using the existing `MockBackend` pattern from `phase_runner.rs` tests:

2. Test `memory_context_records_observations`:
   - Create a `MemoryContext` with an `ObservationBuffer`
   - Run a single-phase plan via `run_plan_with_memory` with a `MockBackend`
   - After completion, inspect the observation buffer (or flush + read from JSONL)
   - Assert: buffer contains `AgentRequest` and `AgentResponse` observation events
   - Assert: phase_id is set correctly on observations
   - Assert: observation count matches expected lifecycle events (at minimum: 1 request + 1 response per task, 1 review request + 1 review response)

3. Test `memory_context_injects_context_from_store`:
   - Create a `VikingStore` in a temp dir, write a project identity entry at `viking://project/identity`
   - Create a `KeywordIndex`, add the project identity entry
   - Build a `MemoryContext` with these
   - Run a phase via `run_plan_with_memory`
   - Use a `MockBackend` that captures the `AgentRequest` it receives
   - Assert: the captured `AgentRequest.context` is `Some(...)` and contains text from the project identity entry

4. Test `memory_errors_do_not_fail_run` + `backend_llm_adapter_bridges_correctly`:
   - Error resilience test: construct a `MemoryContext` with a store pointing at a non-existent/broken path, run a phase, assert it completes `Ok(...)` (memory failures are swallowed)
   - Adapter unit test: create a `BackendLlmAdapter` wrapping a `MockBackend`, call `complete()`, verify it returns the mock response content

## Must-Haves

- [ ] Observation recording test proves events are captured during phase execution
- [ ] Context injection test proves `AgentRequest.context` is populated from store data
- [ ] Fail-soft test proves memory errors don't crash the orchestrator run
- [ ] LLM adapter test proves `AgentBackend` → `ExtractionLlm` bridge works
- [ ] All tests use `MockBackend` (no real LLM calls)

## Verification

- `cargo test -p ath-orchestrator -- memory` — all 4+ tests pass
- `cargo test -p ath-orchestrator` — full suite still passes (no regression)
- `cargo test --workspace` — workspace-wide clean

## Inputs

- T02 output — `MemoryContext`, `BackendLlmAdapter`, `run_plan_with_memory`, observation/injection helpers
- `crates/ath-orchestrator/src/phase_runner.rs` test helpers — `MockBackend`, `make_task_spec`, `mock_response_for_task`, `make_passing_verdict_json` etc.
- `crates/ath-memory/src/observe/buffer.rs` — `ObservationBuffer::new()`, `ObservationBuffer::len()`
- `crates/ath-memory/src/store.rs` — `VikingStore::new(root)`, `VikingStore::write(content)`
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::new()`, `KeywordIndex::add(uri, text)`

## Observability Impact

This task is test-only — no new runtime signals are added. Existing signals verified by these tests:

- **Observation JSONL files**: Tests assert that `.ath/memory/observations/<run-uuid>.jsonl` is created and contains `AgentRequest`, `AgentResponse`, and `ReviewVerdict` events after a phase run. A future agent debugging observation capture should run `cargo test -p ath-orchestrator -- memory_context_records_observations` and inspect the assertion output.
- **Context injection tracing**: The `inject_context` helper emits `tracing::info` with `tokens` and `sections` fields. Tests verify the context string is populated in `AgentRequest.context`. Grep test output with `RUST_LOG=info` for "Context injection" to confirm injection path works.
- **Fail-soft warnings**: The `memory_errors_do_not_fail_run` test verifies that a broken store produces `Ok(...)` rather than an error. The underlying code emits `tracing::warn` with "Memory operation failed" — verify with `RUST_LOG=warn cargo test -p ath-orchestrator -- memory_errors`.
- **Inspection**: To check test health, run `cargo test -p ath-orchestrator -- memory` — all tests should pass. Failures indicate a regression in the observation/injection pipeline.

## Expected Output

- `crates/ath-orchestrator/src/memory.rs` — test module with ≥4 integration tests
- All tests green: `cargo test -p ath-orchestrator -- memory`
