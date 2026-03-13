# Phase 8: CLI and Progress - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver the full CLI surface around `ath run`, `ath init`, and `ath report`, plus live terminal progress during execution. This phase defines the default terminal experience, dry-run behavior, verbose transcript visibility, and command ergonomics. It does not expand reporting scope beyond the command surface, and it does not add parallel execution.

</domain>

<decisions>
## Implementation Decisions

### Default run output
- Default `ath run` uses a hybrid live board, not a pure spinner and not a scrolling event log.
- The live view shows both the current phase name and overall phase progress.
- Within the active phase, show a short task checklist with pending/running/done states.
- Keep milestone events visible as durable messages: review pass/fail, retry start, and phase completion.
- On review failure or retry, show a compact retry panel with reviewer identity, a short reason summary, and the attempt number.

### Dry-run and verbose visibility
- `ath run --dry-run` must be honest: if Athena does not have enough local planning data to show the real plan, fail clearly instead of inventing a preview.
- When dry-run can succeed, print the full execution plan: phases, task ordering, assigned agents, dependencies, and parallel groups.
- `--verbose` keeps the normal live board, then prints full transcripts as grouped blocks when each task finishes.
- Verbose transcripts are end-to-end: executor prompts/responses, reviewer prompts/verdicts, and retry feedback.
- Secrets are always redacted in verbose output.

### Command surface
- Plain `ath` with no subcommand shows concise help with example invocations.
- `ath report` should default to the latest run in the current project when no explicit target is given.
- Default `ath report` output should optimize for a human-readable terminal summary.
- `ath init` remains interactive and should offer the example spec during setup rather than always creating it or requiring a separate flag.

### Claude's Discretion
- Exact layout, symbols, and color treatment for the hybrid live board
- How many recent milestone events remain visible in the default terminal view
- Exact wording of dry-run failure guidance and help examples
- Future machine-readable `ath report` flag names and transcript truncation formatting, as long as verbose remains end-to-end

</decisions>

<specifics>
## Specific Ideas

- The CLI should feel like a modern Rust tool in the `cargo` / `just` / `rg` mold.
- Normal mode should feel calm and trustworthy even during long review or retry loops.
- Verbose mode is for diagnosis, not the default day-to-day experience.

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/ath-cli/src/main.rs`: Existing `clap` parser already defines `run`, `init`, and `report`, plus global `--verbose` and `--no-color`.
- `ath_planner::input::{resolve_input_mode, parse_input, display_project_spec_summary}`: Existing `ath run` path already resolves inputs and prints a human-readable project summary.
- `ath_planner::decompose::display_execution_plan`: Existing human-readable plan display can anchor `--dry-run` output when a local plan is available.
- `ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec}`: Plan data already carries task ordering, assigned agents, parallel groups, and critical path metadata.
- `ath_orchestrator::coordinator::AgentCoordinator` and `ath_orchestrator::phase_runner::PhaseStatus`: Execution state already contains the phase/task/retry/review information needed for progress reporting.

### Established Patterns
- `clap` for CLI parsing, with a single `ath` binary entry point
- Human-readable terminal output first, with deeper internals hidden behind `--verbose`
- Color by default when interactive, plain output when `--no-color` or `NO_COLOR` is set
- Sequential phase execution with fail-fast behavior and explicit review/retry states from Phase 7

### Integration Points
- Extend `crates/ath-cli/src/main.rs` to finalize Phase 8 argument surfaces and no-subcommand behavior.
- Wrap `AgentCoordinator::run_plan` / `run_phase` with a progress reporter that can render current phase, current task, assigned agent, and retry state.
- Keep `ath report` argument/help shape compatible with Phase 9, where reporting internals will be implemented.

</code_context>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope

</deferred>

---

*Phase: 08-cli-and-progress*
*Context gathered: 2026-03-13*
