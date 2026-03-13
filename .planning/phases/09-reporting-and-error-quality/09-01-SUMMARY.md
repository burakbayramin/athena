---
phase: 09-reporting-and-error-quality
plan: 01
subsystem: reporting
tags: [run-report, latest-run, report-artifact, phase-id, report-command]

# Dependency graph
requires:
  - phase: 07-phase-runner-and-review plan 04
    provides: "AgentCoordinator run results and project-local filesystem writes"
  - phase: 08-cli-and-progress plan 01
    provides: "`ath report` CLI surface with explicit-target parsing"
  - phase: 08-cli-and-progress plan 04
    provides: "Project-local `.ath` artifact storage pattern"
provides:
  - "Typed `RunReport` and `RunTotals` schemas combining routed plans with completed phase records"
  - "Stable `phase_id` joins from execution records back to routed plan metadata"
  - "Project-local `.ath/runs/<run-id>/report.json` artifacts plus `latest.txt` pointer"
  - "Real `ath report` loading for latest-run and explicit saved-run targets"
affects: [09-reporting-and-error-quality]

# Tech tracking
tech-stack:
  added: [chrono, uuid]
  patterns: [saved-run-artifact, latest-run-pointer, stable-phase-join]

key-files:
  created:
    - crates/ath-types/src/report.rs
  modified:
    - crates/ath-types/src/lib.rs
    - crates/ath-types/src/phase.rs
    - crates/ath-orchestrator/src/phase_runner.rs
    - crates/ath-cli/Cargo.toml
    - crates/ath-cli/src/main.rs
    - crates/ath-cli/src/report.rs
    - crates/ath-cli/src/run.rs
    - Cargo.lock

key-decisions:
  - "Persist the full routed `ExecutionPlan` alongside `PhaseRecord`s so later report rendering never needs to re-plan"
  - "Join routed phases to execution results through numeric `phase_id` rather than matching on human-readable phase names"
  - "Use a project-local `latest.txt` pointer under `.ath/runs` so `ath report` resolves the newest saved run without live in-memory state"
  - "Write the report artifact on the successful run path before the terminal reporter finishes"

patterns-established:
  - "Saved Athena artifacts live under `.ath/` and are readable without contacting any provider"
  - "Report persistence follows a typed schema contract owned by `ath-types`, while CLI code handles filesystem layout and rendering"

requirements-completed: []

# Metrics
duration: 7min
completed: 2026-03-13
---

# Phase 9 Plan 1: Run Report Artifact Summary

**Athena now persists a typed run-report artifact for every successful run and `ath report` can load either the latest saved run or an explicit saved target without re-execution**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-13T11:14:22Z
- **Completed:** 2026-03-13T11:21:42Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Added `RunReport` and `RunTotals` in `ath-types`, including round-trip tests for saved report artifacts
- Extended `PhaseRecord` with stable `phase_id` joins and populated that field from the orchestrator's phase runner
- Persisted successful runs under `.ath/runs/<run-id>/report.json` with a `latest.txt` pointer for default `ath report` resolution
- Replaced the `ath report` placeholder path with real artifact loading and human-readable rendering of saved review outcomes and totals

## Task Commits

Each TDD task landed as a red/green pair:

1. **Task 1: Add typed run-report schemas and stable phase joins for report persistence** - `607b48e` (test), `9b606c6` (feat)
2. **Task 2: Persist run reports under `.ath/runs` and wire latest-run resolution into `ath report`** - `5dfe9cd` (test), `a08b0ed` (feat)

## Files Created/Modified
- `crates/ath-types/src/report.rs` - New run-report schema with aggregate totals and serde round-trip tests
- `crates/ath-types/src/lib.rs` - Exports the report types for downstream crates
- `crates/ath-types/src/phase.rs` - Adds stable `phase_id` support for report joins
- `crates/ath-orchestrator/src/phase_runner.rs` - Populates `phase_id` on completed phase records
- `crates/ath-cli/src/report.rs` - Adds report building, persistence, loading, latest-run resolution, and rendering
- `crates/ath-cli/src/run.rs` - Persists a saved report after successful execution
- `crates/ath-cli/src/main.rs` - Updates CLI tests for the real report target contract
- `crates/ath-cli/Cargo.toml` - Adds CLI access to `chrono` and test-only `uuid`
- `Cargo.lock` - Records the CLI dependency graph update

## Decisions Made
- The persisted artifact stores routed plan metadata and completed execution data together so Phase 9 can build richer reporting without replaying live state
- Latest-run lookup is project-local and file-backed, not global process state
- Explicit report targets accept either a run id, a run directory, or a direct report file path

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `run.rs` initially moved `output_dir` into the coordinator before report persistence; cloning the path kept the successful-run write hook simple
- The new report tests required explicit `chrono` runtime support and `uuid` as a CLI dev-dependency

## User Setup Required
None - saved reports work with local filesystem state only.

## Next Phase Readiness
- `09-02` can now accumulate token usage into an existing saved-report structure instead of inventing a second reporting contract
- `ath report` already has a real load path, so later reporting work can focus on richer content instead of plumbing
- No blockers recorded for wave 2

---
*Phase: 09-reporting-and-error-quality*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-types/src/report.rs: FOUND
- crates/ath-cli/src/report.rs: FOUND
- Commit 9b606c6: FOUND
- Commit a08b0ed: FOUND
