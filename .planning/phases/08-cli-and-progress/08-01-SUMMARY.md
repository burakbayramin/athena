---
phase: 08-cli-and-progress
plan: 01
subsystem: cli
tags: [clap, help, dry-run, latest-run, command-dispatch]

# Dependency graph
requires:
  - phase: 04-input-parsing
    provides: "resolve_input_mode, parse_input, and project spec summary display used by ath run"
  - phase: 05-phase-decomposition
    provides: "Execution plan display reused by the run command"
provides:
  - "Module-based ath-cli command surfaces for run, init, and report"
  - "Help-first root CLI output with example invocations"
  - "Parsed --dry-run flag and latest-run report target contract"
  - "Focused cli_surface tests covering clap shape and placeholder semantics"
affects: [08-cli-and-progress, 09-reporting]

# Tech tracking
tech-stack:
  added: []
  patterns: [command-dispatch, module-based-cli]

key-files:
  created:
    - crates/ath-cli/src/init.rs
    - crates/ath-cli/src/report.rs
    - crates/ath-cli/src/run.rs
  modified:
    - crates/ath-cli/src/main.rs

key-decisions:
  - "ConfigStore loading moved into run_command so plain `ath` can show clap help without requiring configured providers"
  - "`ath run --dry-run` fails clearly until the local no-cost plan cache exists, avoiding a misleading or API-backed preview"
  - "`ath report` accepts an optional explicit target but defaults its contract to the latest run in the current project"

patterns-established:
  - "Subcommand modules own both clap argument structs and command entry points"
  - "No-subcommand help comes from Cli::command() rendering instead of a custom fallback string"

requirements-completed: []

# Metrics
duration: 16min
completed: 2026-03-13
---

# Phase 8 Plan 1: CLI Surface Summary

**Modular ath-cli command dispatch with real clap help, parsed dry-run surface, and latest-run report targeting contracts**

## Performance

- **Duration:** 16 min
- **Started:** 2026-03-13T12:27:58Z
- **Completed:** 2026-03-13T12:43:58Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Split the monolithic `main.rs` into dispatcher-only logic plus dedicated `run`, `init`, and `report` command modules
- Replaced the `ath` no-subcommand fallback string with real clap help output and example invocations
- Added the `--dry-run` run flag, latest-run report target defaults, and placeholder command handlers aligned with Phase 8 context
- Added 8 `cli_surface` tests covering clap validity, help text, dry-run parsing, and placeholder semantics

## Task Commits

Implementation landed in one scoped commit because the dispatcher split and placeholder handlers changed the same CLI files and were verified together:

1. **Plan 08-01 implementation** - `0beb58c` (feat)

## Files Created/Modified
- `crates/ath-cli/src/main.rs` - Root clap parser, global flags, help rendering, dispatch, and `cli_surface` tests
- `crates/ath-cli/src/run.rs` - `RunArgs`, existing parse/decompose flow, provider status output, and honest dry-run placeholder failure
- `crates/ath-cli/src/init.rs` - Interactive init command surface and setup placeholder text
- `crates/ath-cli/src/report.rs` - Report command surface, latest-run target resolution, and placeholder text for explicit targets

## Decisions Made
- Moved config loading out of the top-level dispatch path so plain `ath` can print help even on an unconfigured machine
- Chose an explicit dry-run failure message now instead of silently routing `--dry-run` through the normal API-backed planning path
- Kept `ath report` target parsing as an optional free-form value so Phase 9 can decide the final report lookup implementation without re-breaking the CLI

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `cargo fmt` reformatted unrelated workspace files because of line-ending normalization; those incidental diffs were restored before commit so the plan stayed scoped to `ath-cli`

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `ath-cli` now has stable command boundaries for the live progress reporter wiring in `08-02`
- Plain help and command parsing are locked, so later plans can add execution, verbose transcripts, and dry-run cache behavior without reshaping the CLI surface
- No blockers recorded for the next plan

---
*Phase: 08-cli-and-progress*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-cli/src/main.rs: FOUND
- crates/ath-cli/src/run.rs: FOUND
- crates/ath-cli/src/init.rs: FOUND
- crates/ath-cli/src/report.rs: FOUND
- Commit 0beb58c: FOUND
