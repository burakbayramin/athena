---
phase: 01-foundation
plan: 02
subsystem: types
tags: [serde, json, schemas, validation, thiserror, chrono, uuid]

# Dependency graph
requires:
  - phase: 01-foundation/01-01
    provides: Cargo workspace scaffold with ath-types crate stub
provides:
  - ValidationError enum with fix hints
  - AgentKind enum with provider_name() for type-safe agent identification
  - AgentRequest and AgentResponse inter-agent message types
  - ProjectSpec, GoalSpec, SkillTag with validate() method
  - ReviewVerdict with blocks_progress(), Severity, CodeSuggestion
  - PhaseRecord, TokenUsage, AgentContribution, ReviewAttempt audit trail types
affects: [01-03, 01-04, 02-agents, 03-git, 04-orchestrator, 05-planner, 06-engine, all-future-phases]

# Tech tracking
tech-stack:
  added: []
  patterns: [derive-serialize-deserialize, validate-method-pattern, typed-error-with-hints, serde-rename-all-lowercase]

key-files:
  created:
    - crates/ath-types/src/error.rs
    - crates/ath-types/src/agent.rs
    - crates/ath-types/src/project.rs
    - crates/ath-types/src/review.rs
    - crates/ath-types/src/phase.rs
  modified:
    - crates/ath-types/src/lib.rs

key-decisions:
  - "TokenUsage defined in phase.rs (audit context), AgentResponse uses simple u64 fields for token counts"
  - "All types derive Debug, Clone, Serialize, Deserialize, PartialEq for consistent capabilities"
  - "Severity enum uses serde rename_all lowercase for clean JSON output"
  - "ValidationError convenience constructors (empty_field, invalid_value) for ergonomic error creation"

patterns-established:
  - "validate() returns Result<(), ValidationError> on all schema types that need business rules"
  - "Re-export key types from lib.rs crate root for ergonomic imports"
  - "Inline #[cfg(test)] modules with round-trip serialization tests per source file"
  - "blocks_progress() semantic method on ReviewVerdict for severity-based gating"

requirements-completed: [PLAN-04]

# Metrics
duration: 2min
completed: 2026-03-12
---

# Phase 1 Plan 2: Inter-Agent Schemas Summary

**Typed inter-agent schemas (ProjectSpec, AgentRequest/Response, ReviewVerdict, PhaseRecord) with validation methods and 24 round-trip serialization tests**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-12T10:59:38Z
- **Completed:** 2026-03-12T11:01:46Z
- **Tasks:** 1
- **Files modified:** 6

## Accomplishments
- Complete inter-agent type system with 5 schema modules covering all agent communication boundaries
- Validation methods with typed errors containing actionable fix hints on ProjectSpec and ReviewVerdict
- 24 passing unit tests covering JSON round-trip serialization, validation rejection, and semantic methods
- All types re-exported from crate root for ergonomic `use ath_types::ProjectSpec` imports

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement error types and all inter-agent schemas** - `c63f49c` (feat)

## Files Created/Modified
- `crates/ath-types/src/error.rs` - ValidationError enum with EmptyField/InvalidValue variants and fix hints
- `crates/ath-types/src/agent.rs` - AgentKind enum, AgentRequest, AgentResponse with round-trip tests
- `crates/ath-types/src/project.rs` - ProjectSpec, GoalSpec, SkillTag with validate() and tests
- `crates/ath-types/src/review.rs` - ReviewVerdict, Severity, CodeSuggestion with blocks_progress() and tests
- `crates/ath-types/src/phase.rs` - PhaseRecord, TokenUsage, AgentContribution, ReviewAttempt with tests
- `crates/ath-types/src/lib.rs` - Module declarations and key type re-exports

## Decisions Made
- Defined TokenUsage in phase.rs (audit/cost tracking context) rather than agent.rs; AgentResponse uses simple u64 fields for token counts to avoid circular imports
- Added PartialEq derive to ValidationError (via Clone + PartialEq) to enable assertion-based testing
- Added convenience constructors on ValidationError (empty_field, invalid_value) for ergonomic error creation in validate() methods
- ProjectSpec.validate() checks goal descriptions are non-empty (not just that goals list is populated)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All inter-agent schema types available for import by ath-config (Plan 01-03) and ath-cli (Plan 01-04)
- Validation pattern established for future types to follow
- Round-trip test pattern established as template for downstream crate tests

## Self-Check: PASSED

- All 6 source files verified present on disk
- Commit `c63f49c` (Task 1) verified in git log
- `cargo test -p ath-types` passes all 24 tests
- `cargo check --workspace` passes

---
*Phase: 01-foundation*
*Completed: 2026-03-12*
