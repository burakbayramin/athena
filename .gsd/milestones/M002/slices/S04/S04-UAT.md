# S04: Orchestrator Integration (Observation + Injection) — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All integration points are proven by automated tests with mock agents — no live API calls or human judgment needed to verify the wiring

## Preconditions

- Rust toolchain installed with `cargo` available
- Working directory is the athena project root
- `cargo check --workspace` passes clean

## Smoke Test

Run `cargo test -p ath-orchestrator -- memory` — all 11 tests should pass. This confirms the memory subsystem is wired into the orchestrator and functioning.

## Test Cases

### 1. ContextInjector builds budget-constrained context

1. Run `cargo test -p ath-memory -- inject`
2. **Expected:** 13 tests pass, covering empty store, project identity population, keyword search results, per-section and total budget enforcement, truncation at word boundaries, XML wrapper formatting, agent notes via keyword, recent run selection

### 2. Observation capture during phase execution

1. Run `cargo test -p ath-orchestrator -- memory_context_records_observations`
2. **Expected:** Test passes — runs a single-phase plan via `run_plan_with_memory`, reads back the JSONL observation file via `ObservationReader`, verifies AgentRequest, AgentResponse, and ReviewVerdict events exist with correct phase_id and run_id

### 3. Context injection into agent requests

1. Run `cargo test -p ath-orchestrator -- memory_context_injects_context_from_store`
2. **Expected:** Test passes — pre-populates VikingStore with project identity at `viking://project/identity`, runs a phase, inspects captured `AgentRequest.context` field, confirms it contains `<athena_context>`, `<project>`, and the project identity text

### 4. Fail-soft on memory errors

1. Run `cargo test -p ath-orchestrator -- memory_errors_do_not_fail_run`
2. **Expected:** Test passes — uses a broken observations path (`Z:\nonexistent\...`), confirms the run completes `Ok(...)` despite the observation flush failure

### 5. BackendLlmAdapter bridges AgentBackend to ExtractionLlm

1. Run `cargo test -p ath-orchestrator -- backend_llm_adapter`
2. **Expected:** 3 tests pass (`bridges_correctly`, `forwards_prompt`, `maps_error_to_memory_error`) — adapter handles both schema and no-schema extraction calls, forwards prompts correctly, maps AgentError to MemoryError

### 6. Existing orchestrator tests unaffected

1. Run `cargo test -p ath-orchestrator`
2. **Expected:** 146 tests pass (135 existing + 11 memory). Zero existing tests modified or broken.

### 7. Workspace compiles clean

1. Run `cargo check --workspace`
2. **Expected:** Compiles with no errors. Only pre-existing warnings in `observe/buffer.rs` (dead_code for assert_sync/assert_send).

## Edge Cases

### Empty store produces empty context

1. Run `cargo test -p ath-memory -- empty_store_produces_empty_context`
2. **Expected:** `InjectedContext.is_empty()` returns true, `estimated_tokens` is 0, `sections_included` is empty

### Zero token budget

1. Run `cargo test -p ath-memory -- truncate_to_budget_zero_budget`
2. **Expected:** Returns empty string — no panic, no overflow

### Oversized content truncated at word boundaries

1. Run `cargo test -p ath-memory -- per_section_budget_enforced_oversized_content_truncated`
2. **Expected:** Content exceeding section budget is truncated cleanly at a word boundary, not mid-word

### Missing URIs produce empty sections (fail-soft)

1. Run `cargo test -p ath-memory -- missing_uris_produce_empty_sections_fail_soft`
2. **Expected:** Store read failures for individual sections produce empty sections rather than propagating errors — other sections still populate normally

### Inject context returns None on empty store

1. Run `cargo test -p ath-orchestrator -- inject_context_returns_none_on_empty_store`
2. **Expected:** When store has no relevant content, `inject_context` returns `None` — no context field set on the agent request

## Failure Signals

- Any test in `cargo test -p ath-orchestrator -- memory` failing — memory integration is broken
- Any of the 135 existing orchestrator tests failing — backward compatibility violated
- `cargo check --workspace` producing new errors — dependency or type mismatch introduced
- `memory_errors_do_not_fail_run` failing — fail-soft behavior broken, memory errors propagating to orchestrator

## Not Proven By This UAT

- Full two-run end-to-end lifecycle (first run captures → extracts → second run gets injected context) — deferred to S06
- Memory-aware parallel execution — parallel groups run sequentially when memory is enabled
- Real LLM extraction quality — extraction pipeline tested with mock responses, not live API calls
- CLI commands for memory inspection — deferred to S05
- Config file loading for memory settings — deferred to S05
- Observation capture for review requests — reviewer dispatch bypasses memory-aware path

## Notes for Tester

- All tests use `MockBackend` — no API keys needed
- Integration tests create temp directories that are cleaned up on drop
- To see tracing output, prefix commands with `RUST_LOG=info` (e.g., `RUST_LOG=info cargo test -p ath-orchestrator -- memory_context_injects --nocapture`)
- The 3 pre-existing warnings in `ath-memory` about dead_code in `observe/buffer.rs` are benign compile-time assertions, not test failures
