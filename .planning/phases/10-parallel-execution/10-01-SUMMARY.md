---
phase: 10-parallel-execution
plan: "01"
subsystem: orchestrator
tags: [tokio, JoinSet, parallel, async, isolation]

requires:
  - phase: 07-orchestrator-pipeline
    provides: AgentCoordinator, run_plan, PhaseRunner pipeline
  - phase: 10-parallel-execution
    provides: Wave 0 test stubs defining parallel execution contract
provides:
  - Parallel group dispatch via tokio JoinSet in AgentCoordinator
  - Pre-dispatch isolation validation integrated into coordinator
  - IsolationViolation and ParallelPhaseFailed error variants
  - Commit gate pattern for serialized write+git operations
affects: [10-parallel-execution]

tech-stack:
  added: []
  patterns: [joinset-fan-out, commit-gate-serialization, arc-registry]

key-files:
  created: []
  modified:
    - crates/ath-orchestrator/src/error.rs
    - crates/ath-orchestrator/src/coordinator.rs
    - crates/ath-orchestrator/src/progress.rs

key-decisions:
  - "Registry field changed from AgentRegistry to Arc<AgentRegistry> for safe sharing across spawned tasks"
  - "Empty parallel_groups falls back to single-phase groups from execution_order for backward compatibility"
  - "File writes rely on isolation check for disjointness rather than commit gate serialization"
  - "Group results sorted by phase_id for deterministic ordering across runs"

patterns-established:
  - "JoinSet fan-out with fail-fast abort_all on first error"
  - "Pre-dispatch isolation gate before any parallel dispatch"
  - "write_files_to_dir extracted helper for reuse across dispatch paths"

requirements-completed: [ORCH-04]

duration: 6min
completed: 2026-03-13
---

# Phase 10 Plan 01: Parallel Group Dispatch Summary

**Coordinator refactored to dispatch parallel-eligible phases concurrently via tokio JoinSet, with pre-dispatch isolation validation and deterministic result ordering**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-13T12:58:26Z
- **Completed:** 2026-03-13T13:04:26Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- IsolationViolation and ParallelPhaseFailed error variants with hint methods and unit tests
- AgentCoordinator refactored from sequential execution_order loop to parallel_groups iteration
- Single-phase groups run directly without JoinSet overhead; multi-phase groups use JoinSet fan-out
- Pre-dispatch check_isolation call blocks execution when file ownership conflicts exist
- All 131 non-Wave-0 tests pass; 4 Wave 0 stubs remain as expected (plan 02 responsibility)
- No new clippy warnings introduced

## Task Commits

Each task was committed atomically:

1. **Task 1: Add parallel execution error variants** - `493ddce` (feat)
2. **Task 2: Refactor coordinator for parallel group dispatch** - `17a9009` (feat)

## Files Created/Modified
- `crates/ath-orchestrator/src/error.rs` - Added IsolationViolation and ParallelPhaseFailed variants with hints and tests
- `crates/ath-orchestrator/src/coordinator.rs` - Refactored run_plan_with_progress for parallel group dispatch via JoinSet
- `crates/ath-orchestrator/src/progress.rs` - Updated make_plan test helper to populate parallel_groups

## Decisions Made
- Changed `AgentCoordinator.registry` field type from `AgentRegistry` to `Arc<AgentRegistry>` for safe sharing across spawned JoinSet tasks. Non-breaking since callers pass ownership via `new()`.
- When `parallel_groups` is empty, fall back to treating each `execution_order` entry as a single-phase group. This maintains backward compatibility with existing code that constructs plans without explicit parallel groups.
- File writes in parallel phases rely on the isolation check's guarantee of disjoint file ownership rather than serializing through the commit gate. The commit gate is available for future git commit serialization.
- Group results are sorted by `phase_id` before extending the results vec, ensuring deterministic ordering regardless of task completion order.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Parallel dispatch infrastructure is complete
- Wave 0 test stubs exercise the new parallel paths before hitting todo!()
- Plan 02 will fill in the test stubs with full assertions for concurrent overlap, timing, isolation, and metadata
- Plan 03 will add integration tests and edge cases

---
*Phase: 10-parallel-execution*
*Completed: 2026-03-13*
