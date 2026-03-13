---
phase: 09-reporting-and-error-quality
plan: 03
subsystem: reporting
tags: [pricing, cost-estimation, phase-summaries, report-totals, usage-reporting]

# Dependency graph
requires:
  - phase: 09-reporting-and-error-quality plan 01
    provides: "Saved run-report artifacts and latest-run loading"
  - phase: 09-reporting-and-error-quality plan 02
    provides: "Complete per-agent token accounting across execution and review"
provides:
  - "Static provider/model pricing helper for Athena's supported default models"
  - "Structured phase summaries and report totals with explicit `n/a` behavior for unsupported pricing"
  - "Human-readable `ath report` output that includes phase-by-phase token and estimated-cost totals"
affects: [09-reporting-and-error-quality]

# Tech tracking
tech-stack:
  added: []
  patterns: [pricing-table, backward-compatible-report-upgrade, report-summary-fallback]

key-files:
  created:
    - crates/ath-cli/src/cost.rs
  modified:
    - crates/ath-cli/src/main.rs
    - crates/ath-cli/src/report.rs
    - crates/ath-types/src/lib.rs
    - crates/ath-types/src/report.rs

key-decisions:
  - "Unsupported pricing is represented as `None` in report totals and rendered as `n/a`, never as a fake zero-dollar cost"
  - "Older `09-01` report artifacts remain readable through serde defaults plus on-the-fly phase summary reconstruction"
  - "Gemini 2.5 Pro uses the standard <=200K-input-token rate because Athena does not persist request-level context-size tiers"

patterns-established:
  - "Structured report summaries are the shared source of truth for both persisted totals and terminal rendering"
  - "Pricing logic lives in a dedicated CLI helper instead of being spread across report formatting code"

requirements-completed: [QUAL-04, OUTP-03]

# Metrics
duration: 4min
completed: 2026-03-13
---

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
