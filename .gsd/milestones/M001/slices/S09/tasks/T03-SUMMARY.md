---
id: T03
parent: S09
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
# T03: Plan 03

**# Phase 9 Plan 3: Cost Estimation Summary**

## What Happened

# Phase 9 Plan 3: Cost Estimation Summary

**Saved reports now carry structured phase/run usage summaries with real model pricing for supported defaults, and `ath report` renders token plus estimated-cost totals while showing `n/a` whenever Athena cannot price a model honestly**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-13T11:37:35Z
- **Completed:** 2026-03-13T11:41:37Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added a dedicated pricing helper for `opus-4`, `sonnet-4`, `2.5-pro`, and `o3`, with explicit unsupported-pricing results for unknown models
- Extended the run-report schema with persisted phase summaries and optional cost totals so saved artifacts can express `n/a` honestly
- Upgraded `ath report` rendering to show per-phase usage/cost sections, per-agent totals, and overall run totals from the same structured summary data
- Kept older Phase 9 report artifacts readable by defaulting missing summaries and rebuilding them from raw phase records when needed

## Task Commits

Both tasks landed through one shared red/green TDD pair:

1. **Task 1: Add provider/model pricing helpers and estimate contribution costs honestly** - `281370b` (test), `cf6c534` (feat)
2. **Task 2: Surface token and cost totals in the human-readable report output** - `281370b` (test), `cf6c534` (feat)

## Files Created/Modified
- `crates/ath-cli/src/cost.rs` - Pricing table and explicit unsupported-pricing helper
- `crates/ath-cli/src/report.rs` - Structured phase-summary aggregation, fallback upgrade path for older reports, and richer terminal rendering
- `crates/ath-cli/src/main.rs` - Registers the new cost module in the CLI binary
- `crates/ath-types/src/report.rs` - Adds `ReportTotals`, `PhaseSummary`, `AgentTotals`, and backward-compatible serde defaults
- `crates/ath-types/src/lib.rs` - Re-exports the richer report types

## Decisions Made
- Cost estimates are optional values because unsupported pricing must surface as unknown, not zero
- Athena keeps raw phase records and derived phase summaries together so report output can stay stable even as rendering evolves
- Report rendering rebuilds summaries on the fly when opening pre-upgrade artifacts to avoid breaking `ath report` for existing saved runs

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Request-level context tiers are not persisted, so Gemini 2.5 Pro pricing must be documented as a base-tier estimate rather than pretending Athena knows the long-context rate
- The richer report schema needed explicit serde defaults to keep earlier saved artifacts loadable

## User Setup Required
None - pricing and totals are computed from saved report artifacts already written by Athena.

## Next Phase Readiness
- `09-04` can focus entirely on actionable CLI errors because reporting and cost totals are complete
- Phase 9's reporting goals are now finished; the final plan only needs to close the error-quality requirement
- No blockers recorded for the last wave

---
*Phase: 09-reporting-and-error-quality*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-cli/src/cost.rs: FOUND
- crates/ath-cli/src/report.rs: FOUND
- Commit cf6c534: FOUND
