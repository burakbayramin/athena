---
phase: 09-reporting-and-error-quality
plan: 04
subsystem: cli
tags: [error-rendering, provider-hints, review-failures, validation-errors, downcasting]

# Dependency graph
requires:
  - phase: 09-reporting-and-error-quality plan 03
    provides: "Completed reporting/cost output so the final gap is pure error quality"
  - phase: 07-phase-runner-and-review plan 03
    provides: "Phase runner retry lifecycle and review halt conditions"
provides:
  - "Provider-specific auth/config hints on `AgentError`"
  - "Reviewer-aware max-retry failures carrying reviewer identity and failed attempt number"
  - "Centralized CLI error rendering for provider, review, validation, and config failures"
affects: [09-reporting-and-error-quality, 10-parallel-execution]

# Tech tracking
tech-stack:
  added: []
  patterns: [centralized-error-rendering, typed-error-downcast, actionable-hint-layer]

key-files:
  created: []
  modified:
    - crates/ath-agents/src/error.rs
    - crates/ath-cli/src/main.rs
    - crates/ath-orchestrator/src/error.rs
    - crates/ath-orchestrator/src/phase_runner.rs
    - crates/ath-types/src/error.rs

key-decisions:
  - "CLI error output is assembled in one `render_error_output()` path rather than scattered across commands"
  - "Provider auth hints name both the environment variable and the config-file key users should inspect"
  - "Review halt errors carry reviewer identity in the typed error itself so the CLI does not need to reconstruct it from logs"

patterns-established:
  - "Actionable hints live close to typed errors, while the CLI owns the final terminal presentation"
  - "Validation errors can now surface raw received values without dumping unreadable debug structures"

requirements-completed: [OUTP-04]

# Metrics
duration: 5min
completed: 2026-03-13
---

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
