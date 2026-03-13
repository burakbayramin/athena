---
phase: 06-module-isolation
plan: 02
subsystem: orchestration
tags: [routing, agent-assignment, majority-vote, fallback, tiebreaking]

# Dependency graph
requires:
  - phase: 06-module-isolation
    provides: "Skill taxonomy routing table, IsolationError, TaskSpec.assigned_agent"
  - phase: 01-foundation
    provides: "AgentKind enum, SkillTag newtype"
  - phase: 05-phase-decomposition
    provides: "TaskSpec, PhaseSpec types"
provides:
  - "route_task function with majority vote, priority tiebreaking, and fallback"
  - "assign_all_tasks batch routing for entire PhaseSpec"
  - "RoutingDecision struct with agent and rationale for verbose output"
affects: [06-module-isolation, 07-review-engine, 10-parallel-execution]

# Tech tracking
tech-stack:
  added: []
  patterns: [majority-vote-routing, discriminant-based-counting, fail-fast-batch]

key-files:
  created:
    - crates/ath-orchestrator/src/router.rs
  modified: []

key-decisions:
  - "Discriminant-based vote counting via std::mem::discriminant for variant-only comparison"
  - "Rationale format: Tags [tag1, tag2] -> Claude (N votes), Gemini (M votes), Codex (K votes)"
  - "Fail-fast in assign_all_tasks -- first routing error stops batch processing"

patterns-established:
  - "Majority vote routing with sorted candidates by (vote_count desc, priority asc)"
  - "Closure-based availability check for circuit-breaker-aware fallback"
  - "assign_all_tasks mutates PhaseSpec in place and returns decisions for diagnostics"

requirements-completed: [ORCH-02]

# Metrics
duration: 3min
completed: 2026-03-13
---

# Phase 6 Plan 2: Agent Router Summary

**Majority-vote task-to-agent router with priority tiebreaking, circuit-breaker fallback, and batch PhaseSpec assignment**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-13T06:03:50Z
- **Completed:** 2026-03-13T06:06:49Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- route_task assigns agents via majority vote across skill tags with discriminant-based counting
- Priority tiebreaking (Claude > Gemini > Codex) resolves equal vote counts
- Circuit-breaker-aware fallback skips unavailable agents and picks next-best candidate
- assign_all_tasks enriches all TaskSpec.assigned_agent fields in a PhaseSpec with fail-fast error handling
- Human-readable routing rationale generated for each decision

## Task Commits

Each task was committed atomically:

1. **Task 1: route_task with majority vote, tiebreak, and fallback** - `746b919` (feat)
2. **Task 2: assign_all_tasks batch routing for PhaseSpec** - `13c7468` (feat)

## Files Created/Modified
- `crates/ath-orchestrator/src/router.rs` - route_task, assign_all_tasks, RoutingDecision with 14 tests

## Decisions Made
- Discriminant-based vote counting (std::mem::discriminant) ensures Claude("opus-4") and Claude("sonnet-4") count as same provider
- Rationale string format matches plan spec: "Tags [...] -> Claude (N votes), Gemini (M votes), Codex (K votes)"
- assign_all_tasks uses fail-fast pattern -- propagates first error immediately rather than collecting all errors
- Closure-based availability check (`impl Fn(&AgentKind) -> bool`) decouples router from circuit breaker implementation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Rust toolchain (cargo) not available in execution environment; code verified structurally against established patterns from phases 1-6

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Router ready for IsolationManager integration
- RoutingDecision.rationale available for --verbose CLI output
- assign_all_tasks ready to be called before phase dispatch

## Self-Check: PASSED

All key files verified present. Both task commits (746b919, 13c7468) confirmed in git log.

---
*Phase: 06-module-isolation*
*Completed: 2026-03-13*
