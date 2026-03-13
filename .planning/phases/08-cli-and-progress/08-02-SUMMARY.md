---
phase: 08-cli-and-progress
plan: 02
subsystem: orchestration
tags: [progress, execution, indicatif, routing, isolation, cli]

# Dependency graph
requires:
  - phase: 07-phase-runner-and-review
    provides: "AgentCoordinator, run_phase review loop, AgentRegistry, and execution audit records"
  - phase: 08-cli-and-progress plan 01
    provides: "Module-based ath-cli command surfaces for run/init/report"
provides:
  - "ProgressEvent observer seam in ath-orchestrator for phase, task, review, retry, and completion updates"
  - "TerminalProgressReporter with interactive board updates and plain-text fallback"
  - "Real ath run pipeline through routing, isolation checks, provider registry build, and AgentCoordinator execution"
  - "Focused progress tests in both ath-orchestrator and ath-cli"
affects: [08-cli-and-progress, 09-reporting-and-error-quality]

# Tech tracking
tech-stack:
  added: [indicatif, tracing, tracing-subscriber, tracing-indicatif]
  patterns: [observer-seam, terminal-progress-reporter, provider-registry-builder]

key-files:
  created:
    - crates/ath-orchestrator/src/progress.rs
    - crates/ath-cli/src/progress.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/ath-cli/Cargo.toml
    - crates/ath-cli/src/main.rs
    - crates/ath-cli/src/run.rs
    - crates/ath-orchestrator/src/coordinator.rs
    - crates/ath-orchestrator/src/lib.rs
    - crates/ath-orchestrator/src/phase_runner.rs

key-decisions:
  - "Progress events are emitted through an Arc-based observer seam so ath-orchestrator stays terminal-agnostic"
  - "Planning now selects the first configured provider instead of hard-requiring Claude for parse/decompose"
  - "TerminalProgressReporter falls back to plain text when stdout is non-interactive or NO_COLOR/no_color is active"

patterns-established:
  - "Execution lifecycle emits semantic ProgressEvent values before and after task/review transitions"
  - "CLI run preparation assigns agents and checks isolation before AgentCoordinator begins execution"

requirements-completed: [OUTP-02]

# Metrics
duration: 17min
completed: 2026-03-13
---

# Phase 8 Plan 2: Progress Reporter Summary

**Real ath run execution with orchestrator progress events, a terminal live board, and pre-execution routing and isolation checks**

## Performance

- **Duration:** 17 min
- **Started:** 2026-03-13T09:45:50Z
- **Completed:** 2026-03-13T10:02:58Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Added a project-owned `ProgressEvent` seam and observer contract so coordinator and phase runner emit progress while work is happening
- Wired `ath run` through real execution: planning backend selection, agent assignment, isolation validation, provider registry creation, and `AgentCoordinator::run_plan_with_progress`
- Built `TerminalProgressReporter` with interactive board updates plus plain-text fallback for non-interactive or `NO_COLOR` execution
- Added 5 orchestrator progress tests and 3 CLI progress tests, then verified `cargo test -p ath-cli` and `cargo test --workspace`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add orchestrator-side progress events and lifecycle emission** - `848b62c` (feat)
2. **Task 2: Build terminal reporter and wire ath run into real execution** - `502bc3f` (feat)

## Files Created/Modified
- `crates/ath-orchestrator/src/progress.rs` - ProgressEvent definitions, observer seam, and focused lifecycle tests
- `crates/ath-orchestrator/src/coordinator.rs` - Phase start/completion emission and `run_plan_with_progress`
- `crates/ath-orchestrator/src/phase_runner.rs` - Task, review, retry, and review-pass/fail progress emission during execution
- `crates/ath-cli/src/progress.rs` - TerminalProgressReporter with interactive snapshot rendering, durable milestone lines, and plain-text fallback
- `crates/ath-cli/src/run.rs` - Real run pipeline through planning, routing, isolation, registry build, coordinator execution, and finish messaging
- `Cargo.toml` / `crates/ath-cli/Cargo.toml` / `Cargo.lock` - Progress stack dependencies and CLI access to orchestration crates

## Decisions Made
- Used an `Arc<dyn ProgressObserver>` seam instead of terminal libraries in `ath-orchestrator`, keeping the core execution layer presentation-free
- Chose to emit both `ReviewPassed` and `ReviewFailed` so the default UI can preserve durable milestone messages for either outcome
- Selected the first configured provider for planning to avoid blocking `ath run` on Anthropic-only availability

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- A CLI isolation test initially asserted the wrong error text; updated the assertion to match the real `IsolationError::FileConflict` contract after verifying the behavior
- Workspace verification still reports a pre-existing unused import warning in `crates/ath-orchestrator/src/isolation.rs`; tests remain green

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The default run path now has a stable event stream that `08-03` can reuse for verbose transcript blocks instead of forking execution
- `ath run` reaches the real coordinator, so transcript capture can attach to actual task/review lifecycle events in the next plan
- No blockers recorded for `08-03`

---
*Phase: 08-cli-and-progress*
*Completed: 2026-03-13*

## Self-Check: PASSED
- crates/ath-orchestrator/src/progress.rs: FOUND
- crates/ath-cli/src/progress.rs: FOUND
- crates/ath-cli/src/run.rs: FOUND
- Commit 848b62c: FOUND
- Commit 502bc3f: FOUND
