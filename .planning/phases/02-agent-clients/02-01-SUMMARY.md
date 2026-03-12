---
phase: 02-agent-clients
plan: 01
subsystem: agents
tags: [async-trait, tokio, circuit-breaker, thiserror, mock-backend]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "ath-types AgentRequest/AgentResponse/AgentKind types, workspace dependency management"
provides:
  - "AgentError enum with retryable classification (8 variants)"
  - "CircuitBreaker state machine (Closed/Open/HalfOpen lifecycle)"
  - "AgentBackend async trait (send, is_available, provider_name)"
  - "MockBackend test double (sequenced, always_ok, failing modes)"
affects: [02-agent-clients, 06-module-isolation, 07-phase-runner]

# Tech tracking
tech-stack:
  added: [async-trait 0.1, tokio 1.x]
  patterns: [circuit-breaker-state-machine, async-trait-for-dyn-dispatch, mock-mode-enum]

key-files:
  created:
    - crates/ath-agents/src/error.rs
    - crates/ath-agents/src/circuit_breaker.rs
    - crates/ath-agents/src/backend.rs
    - crates/ath-agents/src/mock.rs
  modified:
    - crates/ath-agents/src/lib.rs
    - crates/ath-agents/Cargo.toml
    - Cargo.toml

key-decisions:
  - "Renamed AgentError::Unknown.source to .message to avoid thiserror 2.0 #[source] attribute conflict"
  - "Used tokio::time::Instant for CircuitBreaker to enable deterministic testing with start_paused"
  - "MockBackend uses enum MockMode (Sequenced/AlwaysOk/AlwaysFail) instead of trait objects for modes"
  - "AgentError derives Debug only (not Clone) per plan -- errors flow through Result, not stored in collections"
  - "MockBackend::failing takes Fn closure (not value) since AgentError is not Clone"

patterns-established:
  - "Circuit breaker: tokio::time::Instant + start_paused for deterministic time testing"
  - "Mock backends: enum-based mode selection for sequenced/infinite patterns"
  - "async-trait for AgentBackend to support dyn dispatch in Phases 6-7"

requirements-completed: [ORCH-05]

# Metrics
duration: 5min
completed: 2026-03-12
---

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
