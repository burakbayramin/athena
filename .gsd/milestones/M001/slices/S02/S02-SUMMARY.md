---
id: S02
parent: M001
milestone: M001
provides: []
requires: []
affects: []
key_files: []
key_decisions: []
patterns_established: []
observability_surfaces: []
drill_down_paths: []
duration: 
verification_result: passed
completed_at: 
blocker_discovered: false
---
# S02: Agent Clients

**# Phase 2 Plan 1: Agent Contracts Summary**

## What Happened

# Phase 2 Plan 1: Agent Contracts Summary

**AgentError enum with retryable classification, CircuitBreaker state machine with deterministic time testing, AgentBackend async trait, and MockBackend test double with 3 construction modes**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T11:51:41Z
- **Completed:** 2026-03-12T11:56:11Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- AgentError with 8 variants and is_retryable() classification covering all provider error categories
- CircuitBreaker with full Closed/Open/HalfOpen lifecycle and deterministic time-controlled tests
- AgentBackend async trait defining the uniform provider interface (send, is_available, provider_name)
- MockBackend supporting sequenced, always_ok, and failing modes for test orchestration
- 37 unit tests green, 84 workspace tests green, zero compiler warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: AgentError enum and CircuitBreaker state machine** - `0c96bb6` (feat)
2. **Task 2: AgentBackend trait and MockBackend** - `a36226f` (feat)

## Files Created/Modified
- `crates/ath-agents/src/error.rs` - AgentError enum with 8 variants and is_retryable() classification
- `crates/ath-agents/src/circuit_breaker.rs` - CircuitBreaker state machine (Closed/Open/HalfOpen)
- `crates/ath-agents/src/backend.rs` - AgentBackend async trait definition
- `crates/ath-agents/src/mock.rs` - MockBackend with sequenced, always_ok, and failing modes
- `crates/ath-agents/src/lib.rs` - Module declarations and re-exports
- `crates/ath-agents/Cargo.toml` - Added thiserror, async-trait, tokio, chrono, uuid dependencies
- `Cargo.toml` - Added tokio and async-trait to workspace dependencies

## Decisions Made
- Renamed `AgentError::Unknown.source` field to `.message` to avoid conflict with thiserror 2.0's `#[source]` attribute (which expects an `Error` impl)
- Used `tokio::time::Instant` instead of `std::time::Instant` for CircuitBreaker to enable deterministic time testing with `start_paused = true`
- MockBackend uses an internal `MockMode` enum rather than separate struct variants for cleaner pattern matching
- `MockBackend::failing` accepts `impl Fn() -> AgentError` since AgentError is not Clone

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed thiserror 2.0 source field conflict**
- **Found during:** Task 1 (AgentError enum)
- **Issue:** `AgentError::Unknown { source: String }` caused compilation error because thiserror 2.0 interprets `source` as `#[source]` attribute, expecting an `Error` impl
- **Fix:** Renamed field from `source` to `message`
- **Files modified:** crates/ath-agents/src/error.rs
- **Verification:** Compilation succeeds, display format unchanged
- **Committed in:** 0c96bb6 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor field rename for thiserror 2.0 compatibility. No scope creep.

## Issues Encountered
None beyond the auto-fixed thiserror field naming.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All four contract types (AgentError, CircuitBreaker, AgentBackend, MockBackend) are ready for use
- Provider actor implementations (Claude, Gemini, Codex) can now implement AgentBackend
- MockBackend enables testing orchestration logic in Phases 6-7 without real API calls

---
*Phase: 02-agent-clients*
*Completed: 2026-03-12*

# Phase 2 Plan 2: Actor Infrastructure and Provider Implementations Summary

**Tokio actor pattern with genai 0.5 client, exponential backoff with Retry-After honor, and three provider handles (Claude/Gemini/Codex) implementing AgentBackend**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-12T11:59:03Z
- **Completed:** 2026-03-12T12:09:21Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Shared actor infrastructure with ActorMessage, classify_error (mapping genai errors to AgentError), build_genai_client (ConfigStore key injection via AuthResolver), and run_with_retry_and_breaker
- Three provider handles (ClaudeHandle, GeminiHandle, CodexHandle) each implementing AgentBackend with tokio actor, bounded mpsc (32 slots), and per-actor circuit breaker
- Retry logic: manual 3-attempt loop with Retry-After priority from RateLimit errors, exponential backoff fallback (1s/2x/60s/jitter), 5-minute per-request timeout
- Fail-fast API key validation at handle construction (AuthFailed if key missing)
- 59 ath-agents tests green, 106 workspace tests green, zero compiler warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Workspace dependencies and shared actor infrastructure** - `de0ee21` (feat)
2. **Task 2: Claude, Gemini, and Codex actor implementations** - `82b2e64` (feat)

## Files Created/Modified
- `crates/ath-agents/src/actor/mod.rs` - ActorMessage, classify_error, build_genai_client, call_provider, run_with_retry_and_breaker with 11 classify_error tests
- `crates/ath-agents/src/actor/claude.rs` - ClaudeActor + ClaudeHandle implementing AgentBackend with 5 tests
- `crates/ath-agents/src/actor/gemini.rs` - GeminiActor + GeminiHandle implementing AgentBackend with 4 tests
- `crates/ath-agents/src/actor/codex.rs` - CodexActor + CodexHandle implementing AgentBackend with 4 tests
- `crates/ath-agents/src/lib.rs` - Added actor module and re-exports for ClaudeHandle, GeminiHandle, CodexHandle
- `Cargo.toml` - Added genai 0.5 and backon 1.6 to workspace dependencies
- `crates/ath-agents/Cargo.toml` - Added genai, backon to dependencies; reqwest to dev-dependencies
- `Cargo.lock` - Updated with new dependency tree

## Decisions Made
- Used manual retry loop instead of backon's `.retry()` combinator because we need to inspect each error to honor Retry-After headers from RateLimit errors (per CONTEXT.md decision). The backon ExponentialBuilder is still used as an iterator to generate fallback delay values.
- genai's `AuthResolver::from_resolver_fn` receives a closure that captures cloned API keys from ConfigStore and matches on `model_iden.adapter_kind` (Anthropic, Gemini, OpenAI, OpenAIResp) to return the correct key.
- Used `Arc<AtomicBool>` for circuit breaker status flag shared between actor and handle, enabling non-blocking `is_available()` without channel round-trip.
- genai does not expose Retry-After headers from HTTP responses (only body/status in webc::Error), so RateLimit.retry_after is always None from classify_error. The retry_after field can be populated by future middleware or custom header parsing if needed.
- Added reqwest to dev-dependencies for constructing `StatusCode` and `HeaderMap` in classify_error unit tests.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed response value moved after into_first_text()**
- **Found during:** Task 1 (call_provider implementation)
- **Issue:** `ChatResponse::into_first_text()` consumes self, but token usage was read after the move
- **Fix:** Extracted usage.prompt_tokens and usage.completion_tokens before calling into_first_text()
- **Files modified:** crates/ath-agents/src/actor/mod.rs
- **Verification:** cargo check passes
- **Committed in:** de0ee21 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial ordering fix for value consumption. No scope creep.

## Issues Encountered
None beyond the auto-fixed value move ordering.

## User Setup Required
None - no external service configuration required. API keys are loaded from ConfigStore at runtime.

## Next Phase Readiness
- All three provider handles (ClaudeHandle, GeminiHandle, CodexHandle) ready for use via AgentBackend trait
- Phase 6 (Module Isolation) can route tasks to these actors via dyn AgentBackend
- Phase 7 (Phase Runner) can dispatch requests through the uniform trait interface
- Integration tests with real APIs are planned for Phase 2 Plan 3

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 02-agent-clients*
*Completed: 2026-03-12*

# Phase 02 Plan 03: Integration Tests and Agent Layer Verification Summary

**Integration tests proving AgentBackend trait dispatch, dyn dispatch, circuit breaker, and provider handle construction across Claude/Gemini/Codex**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T12:11:00Z
- **Completed:** 2026-03-12T12:16:27Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Integration tests verifying MockBackend through AgentBackend trait in all modes (always_ok, sequenced, failing)
- Dynamic dispatch via Box<dyn AgentBackend> compiles and runs, proving runtime provider selection works
- All three provider handles (ClaudeHandle, GeminiHandle, CodexHandle) reject missing API keys with AuthFailed
- Circuit breaker behavior validated through integration tests
- Human review approved the complete agent client layer (Plans 01-03)

## Task Commits

Each task was committed atomically:

1. **Task 1: Integration tests for agent layer** - `34b2ff4` (test)
2. **Task 2: Human verification of complete agent layer** - checkpoint approved, no code changes

**Plan metadata:** `d2e3ed1` (docs: complete plan)

## Files Created/Modified
- `crates/ath-agents/tests/integration.rs` - Integration tests for AgentBackend trait, MockBackend modes, dyn dispatch, circuit breaker, and provider handle construction

## Decisions Made
- Integration tests use MockBackend exclusively -- no real API calls needed, keeping tests fast and deterministic

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Complete agent client layer is tested and verified
- AgentBackend trait interface ready for orchestrator integration (Phase 03)
- Box<dyn AgentBackend> dispatch proven for runtime provider selection in Phase 06-07
- All provider handles construct correctly when API keys are present (verified by unit tests)

---
*Phase: 02-agent-clients*
*Completed: 2026-03-12*

## Self-Check: PASSED
- FOUND: crates/ath-agents/tests/integration.rs
- FOUND: commit 34b2ff4
- FOUND: .planning/phases/02-agent-clients/02-03-SUMMARY.md
