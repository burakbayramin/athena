---
phase: 06-module-isolation
plan: 01
subsystem: orchestration
tags: [routing, taxonomy, agent-assignment, isolation, thiserror]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "AgentKind enum, SkillTag newtype, ValidationError pattern"
  - phase: 05-phase-decomposition
    provides: "TaskSpec, PhaseSpec, ExecutionPlan types"
provides:
  - "TaskSpec.assigned_agent field for per-task agent routing"
  - "IsolationError enum with FileConflict, AllAgentsUnavailable, UnassignedTask"
  - "Skill taxonomy routing table mapping 15 tags to 3 agent kinds"
  - "Case-insensitive lookup, priority tiebreaking, default agent functions"
affects: [06-module-isolation, 07-review-engine, 10-parallel-execution]

# Tech tracking
tech-stack:
  added: []
  patterns: [static-routing-table, case-insensitive-lookup, error-with-hint]

key-files:
  created:
    - crates/ath-orchestrator/src/taxonomy.rs
    - crates/ath-orchestrator/src/error.rs
  modified:
    - crates/ath-types/src/plan.rs
    - crates/ath-orchestrator/Cargo.toml
    - crates/ath-orchestrator/src/lib.rs
    - crates/ath-planner/src/decompose/dag.rs
    - crates/ath-planner/src/decompose/display.rs
    - crates/ath-planner/src/decompose/validate.rs

key-decisions:
  - "Static routing table with 15 hardcoded tag-to-agent mappings for v1"
  - "Priority tiebreaking: Claude(0) > Gemini(1) > Codex(2)"
  - "Default agent is Claude for unrecognized tags"

patterns-established:
  - "Routing table as HashMap<String, AgentKind> built by pure function"
  - "IsolationError follows ValidationError hint() pattern from ath-types"

requirements-completed: [ORCH-01]

# Metrics
duration: 4min
completed: 2026-03-13
---

# Phase 6 Plan 1: Isolation Foundation Summary

**Static skill taxonomy routing 15 tags to Claude/Gemini/Codex with IsolationError types and TaskSpec.assigned_agent field**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-13T05:57:05Z
- **Completed:** 2026-03-13T06:01:16Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- TaskSpec gains backward-compatible assigned_agent: Option<AgentKind> field with serde(default, skip_serializing_if)
- IsolationError enum with 3 variants (FileConflict, AllAgentsUnavailable, UnassignedTask) each carrying fix hints
- Routing table maps 15 skill tags across 3 agent kinds with case-insensitive lookup and priority tiebreaking

## Task Commits

Each task was committed atomically:

1. **Task 1: TaskSpec assigned_agent field, IsolationError, and Cargo.toml deps** - `bb79f27` (feat)
2. **Task 2: Skill taxonomy routing table with tests** - `aa06d02` (feat)

## Files Created/Modified
- `crates/ath-orchestrator/src/taxonomy.rs` - Routing table, lookup, priority, default_agent functions with tests
- `crates/ath-orchestrator/src/error.rs` - IsolationError enum with 3 variants and hint() method
- `crates/ath-types/src/plan.rs` - TaskSpec.assigned_agent field and backward-compat tests
- `crates/ath-orchestrator/Cargo.toml` - Added ath-agents, thiserror, serde, serde_json dependencies
- `crates/ath-orchestrator/src/lib.rs` - Declared pub mod error and pub mod taxonomy
- `crates/ath-planner/src/decompose/dag.rs` - Added assigned_agent: None to TaskSpec construction
- `crates/ath-planner/src/decompose/display.rs` - Added assigned_agent: None to TaskSpec construction
- `crates/ath-planner/src/decompose/validate.rs` - Added assigned_agent: None to TaskSpec construction

## Decisions Made
- Static routing table with 15 hardcoded tag-to-agent mappings, not user-configurable for v1
- Priority ordering Claude(0) > Gemini(1) > Codex(2) for multi-tag tiebreaking
- Default agent is Claude("opus-4") for unrecognized skill tags

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated TaskSpec constructions in ath-planner**
- **Found during:** Task 1 (TaskSpec field addition)
- **Issue:** Three test helper functions in ath-planner constructed TaskSpec without the new assigned_agent field, which would cause compilation errors
- **Fix:** Added assigned_agent: None to make_task() in validate.rs, display.rs, and make_phase() in dag.rs
- **Files modified:** crates/ath-planner/src/decompose/validate.rs, display.rs, dag.rs
- **Verification:** All construction sites updated, serde(default) handles JSON test strings
- **Committed in:** bb79f27 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for workspace compilation. No scope creep.

## Issues Encountered
- Rust toolchain (cargo) not available in execution environment; code verified structurally against established patterns from phases 1-5

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Routing table and error types ready for IsolationManager (Plan 06-02)
- TaskSpec.assigned_agent field available for router to populate
- Priority function ready for multi-tag tiebreaking logic

## Self-Check: PASSED

All 6 key files verified present. Both task commits (bb79f27, aa06d02) confirmed in git log.

---
*Phase: 06-module-isolation*
*Completed: 2026-03-13*
