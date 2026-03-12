---
phase: 05-phase-decomposition
plan: 02
subsystem: planner
tags: [dag-validation, llm-decomposition, retry-loop, structured-output, contract-checking]

# Dependency graph
requires:
  - phase: 05-phase-decomposition
    provides: "ExecutionPlan/PhaseSpec/TaskSpec types, topological_sort, compute_parallel_groups, critical_path_length, DecomposeError"
  - phase: 04-input-parsing
    provides: "parse_to_project_spec retry pattern, AgentRequest with json_schema, MockBackend"
provides:
  - "validate_plan function catching 5 hard error types with transitive contract satisfaction"
  - "PlanWarning enum for non-blocking warnings (LargePhase, LongCriticalPath)"
  - "DECOMPOSE_SYSTEM_PROMPT and execution_plan_json_schema for LLM structured output"
  - "build_decompose_request with retry feedback"
  - "decompose_project_spec entry point with 3-attempt retry loop and computed DAG fields"
affects: [05-phase-decomposition, 06-module-isolation, 07-review-engine, 08-cli-progress]

# Tech tracking
tech-stack:
  added: []
  patterns: [validate-then-compute, transitive-closure-bfs, format-validation-errors-cap]

key-files:
  created:
    - crates/ath-planner/src/decompose/validate.rs
    - crates/ath-planner/src/decompose/prompt.rs
  modified:
    - crates/ath-planner/src/decompose/mod.rs

key-decisions:
  - "BFS transitive closure for contract satisfaction -- Phase C can consume contracts from Phase A via Phase B"
  - "RawPlanResponse wrapper struct for LLM deserialization -- only phases array, computed fields added after validation"
  - "Validation error feedback capped at 5 errors to avoid prompt bloat on retry"

patterns-established:
  - "validate_plan returns Result<Vec<PlanWarning>, Vec<ValidationError>> -- hard errors block, warnings pass through"
  - "decompose_project_spec follows parse_to_project_spec retry pattern exactly -- proven for all LLM interactions"

requirements-completed: [PLAN-01, PLAN-02]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 5 Plan 02: DAG Validation, Decomposition Prompt, and Retry Loop Summary

**validate_plan with 5 hard error types including transitive contract checking, LLM decomposition prompt with JSON schema, and decompose_project_spec entry point with 3-attempt retry loop**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T21:23:27Z
- **Completed:** 2026-03-12T21:28:25Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- validate_plan catches all 5 hard error types: empty phases, missing dependency targets, circular deps, orphaned goals, unsatisfied contracts
- Transitive contract satisfaction via BFS dependency closure (Phase C can consume from Phase A through Phase B)
- PlanWarning enum with LargePhase (>5 tasks) and LongCriticalPath (>7) non-blocking warnings
- DECOMPOSE_SYSTEM_PROMPT with full decomposition rules and execution_plan_json_schema for structured output
- decompose_project_spec follows proven retry-with-feedback pattern: parse -> validate -> retry or return
- 28 total decompose tests passing (9 validate + 4 prompt + 9 dag + 6 entry point)

## Task Commits

Each task was committed atomically:

1. **Task 1: Validation logic and decomposition prompt** - `8bd2eba` (feat)
2. **Task 2: decompose_project_spec retry loop** - `c73dd3d` (feat)

## Files Created/Modified
- `crates/ath-planner/src/decompose/validate.rs` - validate_plan function with 5 hard errors + 2 warning types, PlanWarning enum
- `crates/ath-planner/src/decompose/prompt.rs` - DECOMPOSE_SYSTEM_PROMPT, execution_plan_json_schema, build_decompose_request
- `crates/ath-planner/src/decompose/mod.rs` - decompose_project_spec entry point with retry loop, RawPlanResponse, format_validation_errors

## Decisions Made
- BFS transitive closure for contract validation: Phase C depending on Phase B (which depends on Phase A) can consume contracts from Phase A
- RawPlanResponse wrapper struct deserializes only phases from LLM; execution_order/parallel_groups/critical_path_length computed after validation
- Validation error feedback capped at 5 errors with "... and N more" to avoid prompt bloat on retry attempts
- AgentKind::Claude("decompose") used for decomposition requests (distinct from "opus-4" used in input parsing)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Full decomposition pipeline ready: ProjectSpec -> LLM -> parse -> validate -> compute DAG fields -> ExecutionPlan
- Plan 05-03 can now build the high-level orchestration that ties input parsing to decomposition
- All 227 workspace tests pass with no regressions

---
*Phase: 05-phase-decomposition*
*Completed: 2026-03-12*
