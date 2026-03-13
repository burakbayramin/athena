---
id: S09
parent: M001
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
# S09: Reporting And Error Quality

**# Phase 9 Plan 1: Run Report Artifact Summary**

## What Happened

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

# Phase 9 Plan 4: Actionable Error Quality Summary

**Athena's terminal errors now distinguish provider auth/config failures, review halts, and validation issues with concrete next-step guidance, all through one centralized CLI renderer**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-13T11:44:16Z
- **Completed:** 2026-03-13T11:49:15Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added provider-specific `AgentError::hint()` guidance that names the correct env var and config key for Anthropic, Google, and OpenAI auth failures
- Extended `PhaseRunnerError::MaxRetriesExceeded` with reviewer identity and populated it from the phase runner's terminal review path
- Upgraded `ValidationError::InvalidValue` to carry optional received content and render it directly when available
- Centralized CLI error formatting behind `render_error_output()`, with tests covering provider auth failures, review halts, validation errors, and unchanged `ConfigError` behavior

## Task Commits

Both tasks landed through one shared red/green TDD pair:

1. **Task 1: Enrich typed errors with the context users actually need** - `711a6ad` (test), `0d2b5bc` (feat)
2. **Task 2: Centralize actionable provider/review/schema error rendering in the CLI** - `711a6ad` (test), `0d2b5bc` (feat)

## Files Created/Modified
- `crates/ath-agents/src/error.rs` - Adds provider-specific actionable hints
- `crates/ath-cli/src/main.rs` - Adds centralized error rendering and CLI-facing error-output tests
- `crates/ath-orchestrator/src/error.rs` - Carries reviewer identity through max-retry failure errors
- `crates/ath-orchestrator/src/phase_runner.rs` - Populates the richer max-retry error with reviewer context
- `crates/ath-types/src/error.rs` - Supports optional raw received values on invalid-value validation errors

## Decisions Made
- The typed error layer owns actionable data, while the CLI owns formatting and presentation
- Review failure context is propagated explicitly instead of expecting users to infer reviewer identity from transcript logs
- Error rendering still prints source chains after the top-level actionable hint, preserving debug value without sacrificing readability

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `rustfmt` on module roots attempted to reformat unrelated files, so the incidental churn was trimmed back before commit
- `git` needed an index refresh on two unchanged files after the formatting pass; the file contents themselves never diverged from `HEAD`

## User Setup Required
None - the new error output is automatic for existing commands.

## Next Phase Readiness
- Phase 9 is complete: reporting, usage/cost accounting, and error quality are all delivered
- Phase 10 can now build parallel execution on top of a fully instrumented CLI with durable reports and actionable failures
- Next workflow step is Phase 10 discussion/planning, not more Phase 9 execution

---
*Phase: 09-reporting-and-error-quality*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-cli/src/main.rs: FOUND
- crates/ath-orchestrator/src/error.rs: FOUND
- Commit 0d2b5bc: FOUND
