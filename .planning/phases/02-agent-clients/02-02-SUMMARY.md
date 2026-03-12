---
phase: 02-agent-clients
plan: 02
subsystem: agents
tags: [genai, backon, tokio-actor, circuit-breaker, retry, exponential-backoff]

# Dependency graph
requires:
  - phase: 02-agent-clients
    provides: "AgentError, CircuitBreaker, AgentBackend trait, MockBackend from Plan 01"
  - phase: 01-foundation
    provides: "ConfigStore with API keys and model defaults, AgentRequest/AgentResponse types"
provides:
  - "Shared actor infrastructure: ActorMessage, classify_error(), build_genai_client(), run_with_retry_and_breaker()"
  - "ClaudeHandle implementing AgentBackend with tokio actor"
  - "GeminiHandle implementing AgentBackend with tokio actor"
  - "CodexHandle implementing AgentBackend with tokio actor"
  - "Error classification mapping genai errors to AgentError variants"
  - "Retry logic honoring Retry-After with exponential backoff fallback"
affects: [02-agent-clients, 06-module-isolation, 07-phase-runner]

# Tech tracking
tech-stack:
  added: [genai 0.5.3, backon 1.6]
  patterns: [tokio-actor-with-handle, auth-resolver-from-config, manual-retry-with-backoff-iterator, classify-error-pattern]

key-files:
  created:
    - crates/ath-agents/src/actor/mod.rs
    - crates/ath-agents/src/actor/claude.rs
    - crates/ath-agents/src/actor/gemini.rs
    - crates/ath-agents/src/actor/codex.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/ath-agents/Cargo.toml
    - crates/ath-agents/src/lib.rs

key-decisions:
  - "Manual retry loop instead of backon Retryable combinator to honor Retry-After from RateLimit errors"
  - "genai AuthResolver::from_resolver_fn with closure capturing cloned ConfigStore keys"
  - "AtomicBool flag for lightweight non-blocking is_available() check without channel round-trip"
  - "reqwest added to dev-dependencies for constructing test StatusCodes in classify_error tests"

patterns-established:
  - "Actor pattern: struct with mpsc::Receiver + Handle with mpsc::Sender, spawned via tokio::spawn"
  - "classify_error: genai::Error -> match on variant -> AgentError (centralized error classification)"
  - "Retry-After priority: check RateLimit.retry_after first, fall back to backon ExponentialBuilder iterator"

requirements-completed: [ORCH-05]

# Metrics
duration: 10min
completed: 2026-03-12
---

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
