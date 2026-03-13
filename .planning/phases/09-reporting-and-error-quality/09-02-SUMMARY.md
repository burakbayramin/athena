---
phase: 09-reporting-and-error-quality
plan: 02
subsystem: reporting
tags: [token-usage, reviewer-usage, retries, agent-identity, phase-record]

# Dependency graph
requires:
  - phase: 09-reporting-and-error-quality plan 01
    provides: "Saved run-report artifact and latest-run loading path"
  - phase: 07-phase-runner-and-review plan 03
    provides: "Phase runner retry loop and review dispatch orchestration"
provides:
  - "Reviewer token usage recorded per review attempt and rolled into per-phase totals"
  - "Exact per-agent contribution accumulation keyed by full `AgentKind` identity"
  - "Retry-safe contribution merging with deduplicated produced-file paths"
affects: [09-reporting-and-error-quality]

# Tech tracking
tech-stack:
  added: []
  patterns: [review-attempt-usage, exact-agent-aggregation, retry-safe-rollup]

key-files:
  created: []
  modified:
    - crates/ath-types/src/phase.rs
    - crates/ath-orchestrator/src/phase_runner.rs
    - crates/ath-cli/src/report.rs

key-decisions:
  - "Reviewer token usage is stored on each `ReviewAttempt` with `#[serde(default)]` to preserve backward-compatible deserialization"
  - "Per-agent contribution merging matches full `AgentKind` equality so model identity is not collapsed to provider discriminants"
  - "Reviewer usage is duplicated intentionally: once on the review attempt for per-attempt reporting and once in contributions for aggregate per-agent totals"

patterns-established:
  - "Phase records are now self-sufficient for report-grade usage accounting across executor runs, reviews, and retries"
  - "Contribution merging deduplicates file paths while still accumulating token totals across repeated attempts"

requirements-completed: []

# Metrics
duration: 3min
completed: 2026-03-13
---

# Phase 9 Plan 2: Token Accounting Summary

**Phase records now carry full token accounting across executors, reviewers, and retries, with exact per-agent rollups that preserve model identity for later reporting and cost estimation**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-13T11:29:07Z
- **Completed:** 2026-03-13T11:31:57Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added reviewer token usage to `ReviewAttempt` and kept older serialized data backward-compatible with a default token payload
- Rolled reviewer usage into `PhaseRecord.contributions` so aggregate per-agent totals include both execution and review work
- Replaced provider-discriminant contribution merging with full `AgentKind` matching and added retry/file-dedup tests
- Verified the changes with focused phase-runner tests, `ath-types` phase tests, and a full `cargo test --workspace`

## Task Commits

Both tasks landed through one shared red/green TDD pair:

1. **Task 1: Record reviewer token usage alongside executor contributions** - `6e5dadd` (test), `d1675cc` (feat)
2. **Task 2: Make per-agent accumulation exact and report-safe across retries** - `6e5dadd` (test), `d1675cc` (feat)

## Files Created/Modified
- `crates/ath-types/src/phase.rs` - Adds review-attempt token usage, default/backward-compatible serde handling, and richer phase-schema tests
- `crates/ath-orchestrator/src/phase_runner.rs` - Captures reviewer tokens, merges contributions by full `AgentKind`, and deduplicates file paths across retries
- `crates/ath-cli/src/report.rs` - Updates saved-report test fixtures to the richer phase schema

## Decisions Made
- `ReviewAttempt.tokens` is the smallest schema addition that preserves per-attempt review cost data without forcing report code to inspect transcripts
- Contribution totals remain the source of truth for aggregate per-agent rollups, so reviewer tokens are merged there in addition to the review history
- Backward compatibility matters because saved report artifacts already exist on disk from `09-01`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Reviewer token data needed to be represented twice to satisfy both aggregate totals and per-attempt report detail without extra reconstruction logic
- The direct `rustfmt` invocation in this environment required `--edition 2021`

## User Setup Required
None - token accounting remains fully local to Athena's persisted execution data.

## Next Phase Readiness
- `09-03` can price usage directly from persisted per-agent/per-attempt token totals instead of inferring missing reviewer work
- `ath report` now has complete usage inputs for richer totals once pricing is added
- No blockers recorded for wave 3

---
*Phase: 09-reporting-and-error-quality*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-types/src/phase.rs: FOUND
- crates/ath-orchestrator/src/phase_runner.rs: FOUND
- Commit d1675cc: FOUND
