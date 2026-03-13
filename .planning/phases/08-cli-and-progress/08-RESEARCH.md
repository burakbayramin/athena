# Phase 8: CLI and Progress - Research

**Researched:** 2026-03-13
**Domain:** Rust CLI command surface, terminal progress rendering, transcript visibility, dry-run execution boundaries
**Confidence:** HIGH

## Summary

Phase 8 is the first phase where Athena has to feel like a real CLI product instead of a collection of internal crates. The current codebase already has strong building blocks: `clap`-derived subcommands in `ath-cli`, human-readable project summary and plan display in `ath-planner`, routed tasks in `ath-orchestrator`, and full phase execution data in `PhaseRecord`, `ReviewAttempt`, `TaskOutput`, and `PhaseStatus`. What is missing is the glue: a proper run-time command surface, a progress event stream that exists *during* execution rather than only after `run_phase()` returns, a verbose transcript path that preserves normal progress UX, and an honest dry-run mode that never falls back to LLM-backed planning.

Two constraints dominate the planning:

1. **Real-time progress needs new orchestration hooks.** `AgentCoordinator::run_plan()` and `run_phase()` currently return completed records; they do not emit events as tasks/reviews/retries happen. Phase 8 therefore needs an observer/reporter seam before any terminal UI can be accurate.
2. **`--dry-run` cannot reuse the normal parse/decompose pipeline.** `parse_input()` and `decompose_project_spec()` both spend LLM calls. Because the locked decision is "fail clearly if no local plan exists", the dry-run success path must consume a serialized local plan artifact (or equivalent local planning data), not generate a new one.

**Primary recommendation:** Plan the phase in four sequential slices that match the roadmap:
- CLI surface cleanup and command structuring
- Progress event plumbing plus default terminal reporter
- Verbose transcript rendering on top of the same event stream
- Dry-run wiring around a local serialized `ExecutionPlan` path

This keeps the risky design choices isolated and makes `PLAN-05` / `OUTP-02` traceable across plans.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Default `ath run` uses a hybrid live board, not a pure spinner and not a scrolling event log.
- The live view shows both the current phase name and overall phase progress.
- Within the active phase, show a short task checklist with pending/running/done states.
- Keep milestone events visible as durable messages: review pass/fail, retry start, and phase completion.
- On review failure or retry, show a compact retry panel with reviewer identity, a short reason summary, and the attempt number.
- `ath run --dry-run` must be honest: if Athena does not have enough local planning data to show the real plan, fail clearly instead of inventing a preview.
- When dry-run can succeed, print the full execution plan: phases, task ordering, assigned agents, dependencies, and parallel groups.
- `--verbose` keeps the normal live board, then prints full transcripts as grouped blocks when each task finishes.
- Verbose transcripts are end-to-end: executor prompts/responses, reviewer prompts/verdicts, and retry feedback.
- Secrets are always redacted in verbose output.
- Plain `ath` with no subcommand shows concise help with example invocations.
- `ath report` should default to the latest run in the current project when no explicit target is given.
- Default `ath report` output should optimize for a human-readable terminal summary.
- `ath init` remains interactive and should offer the example spec during setup rather than always creating it or requiring a separate flag.

### Claude's Discretion
- Exact layout, symbols, and color treatment for the hybrid live board
- How many recent milestone events remain visible in the default terminal view
- Exact wording of dry-run failure guidance and help examples
- Future machine-readable `ath report` flag names and transcript truncation formatting, as long as verbose remains end-to-end

### Deferred Ideas (OUT OF SCOPE)
None - discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PLAN-05 | User can dry-run to see the full plan without executing (no API cost) | Dry-run must branch onto local serialized planning data only; no parse/decompose LLM calls allowed in this mode |
| OUTP-02 | Terminal shows real-time progress: current phase, active agent, task status | Add progress event hooks around coordinator/runner and render them through a hybrid terminal reporter |
</phase_requirements>

## Current Codebase State

### What already exists
- `crates/ath-cli/src/main.rs`
  - `clap` parser already defines `run`, `init`, and `report`
  - global `--verbose` and `--no-color` flags already exist
  - current `run` path does: input resolution -> ProjectSpec summary -> LLM decomposition -> execution plan display
- `crates/ath-planner/src/input/mod.rs`
  - `resolve_input_mode()` already enforces input-mode rules from Phase 4
  - `display_project_spec_summary()` already prints a human-readable pre-execution summary
- `crates/ath-planner/src/decompose/display.rs`
  - `display_execution_plan()` and `format_execution_plan()` already render human-readable plan output
- `crates/ath-types/src/plan.rs`
  - `ExecutionPlan` already serializes/deserializes and carries `execution_order`, `parallel_groups`, and `critical_path_length`
  - `TaskSpec` already has `assigned_agent`
- `crates/ath-types/src/phase.rs`
  - `PhaseRecord`, `AgentContribution`, and `ReviewAttempt` already provide the audit data a reporter needs after execution
- `crates/ath-orchestrator/src/router.rs`
  - `assign_all_tasks()` already mutates `TaskSpec.assigned_agent`
- `crates/ath-orchestrator/src/isolation.rs`
  - `check_isolation()` already validates parallel-group file conflicts
- `crates/ath-orchestrator/src/coordinator.rs`
  - `AgentCoordinator::run_plan()` already sequences phases through `run_phase()`
- `crates/ath-orchestrator/src/phase_runner.rs`
  - `PhaseStatus` captures pending/running/awaiting_review/complete/retrying/review_failed
  - `TaskOutput` / `FileOutput` already capture executor-side structured output

### What is missing
- `ath-cli` does not call router/isolation/coordinator at all yet
- There is no progress observer trait, callback, or event enum emitted during execution
- There is no persistent local execution-plan artifact for dry-run success
- There is no transcript renderer or redaction path
- `ath report` and `ath init` exist only as placeholders
- Workspace dependencies do not currently include `indicatif`, `tracing-subscriber`, or `tracing-indicatif`

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 (workspace) | Subcommands, flags, help output | Already in workspace and already used by `ath-cli` |
| colored | 3 (workspace) | Plain colored terminal output and help/error emphasis | Already used in `ath-cli` and planner display |
| tokio | 1 (workspace) | Async CLI entry point and orchestrator execution | Already used throughout workspace |
| ath-planner | workspace | Input resolution, ProjectSpec summary, plan display | Existing path for run preparation |
| ath-orchestrator | workspace | Routing, isolation, phase execution, review/retry pipeline | Existing execution engine from Phases 6-7 |
| ath-types | workspace | `ExecutionPlan`, `PhaseRecord`, `PhaseStatus`, `TaskOutput` | Existing serialized plan and audit contracts |
| serde / serde_json | 1.0 | Serialize local plan cache and transcript/event payloads | Already in workspace |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| indicatif | 0.18.0 | Progress bars, spinners, `MultiProgress`, durable `println` integration | Default hybrid live board and phase/task status rendering |
| tracing | 0.1.x | Structured event/span instrumentation | Emit progress and transcript events without hard-wiring terminal output into orchestrator |
| tracing-subscriber | 0.3.x | Layered tracing collectors | Required if using a terminal/reporting layer around spans/events |
| tracing-indicatif | 0.3.14 | Bind tracing spans to indicatif progress bars via `IndicatifLayer` | Useful if the planner wants span-backed progress updates instead of a fully custom reporter |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `indicatif` + reporter seam | Plain `println!` logging | Easy, but cannot deliver the locked hybrid live board without noisy output |
| `tracing-indicatif` integration | Pure `indicatif` callbacks only | Simpler, but gives up structured span/event plumbing and makes verbose transcript layering harder |
| Serialized local `ExecutionPlan` artifact | Re-run LLM planning in `--dry-run` | Violates `PLAN-05` because dry-run would incur API cost |
| Orchestrator callback trait | Printing directly inside `run_phase()` | Couples terminal UI to core execution logic and makes tests brittle |

## Architecture Patterns

### Recommended Project Structure
```
crates/ath-cli/src/
  main.rs                 # CLI parsing + command dispatch
  progress.rs             # TerminalProgressReporter / ProgressEvent rendering
  verbose.rs              # Transcript block rendering + redaction helpers
  dry_run.rs              # Local plan loading / dry-run display path
  report.rs               # Human-readable report placeholder wiring
  init.rs                 # Interactive init placeholder wiring

crates/ath-orchestrator/src/
  progress.rs             # ProgressEvent enum + observer trait (or callback adapter)
  coordinator.rs          # emit phase-level progress events
  phase_runner.rs         # emit task/review/retry events
```

### Pattern 1: Command dispatch split
**What:** Keep `Cli` / `Commands` for parsing, then hand off to focused command functions (`run_command`, `init_command`, `report_command`) with explicit argument structs.
**When to use:** Immediately in Plan 01.
**Why:** `main.rs` already mixes parsing and execution. Phase 8 adds enough behavior that the run path must become testable without spawning a process.

**Example:**
```rust
#[derive(Debug, clap::Args)]
struct RunArgs {
    description: Option<String>,
    #[arg(long, value_name = "FILE")]
    spec: Option<PathBuf>,
    #[arg(long, value_name = "DIR")]
    codebase: Option<PathBuf>,
    #[arg(long)]
    dry_run: bool,
}

async fn run_command(args: RunArgs, global: GlobalArgs) -> anyhow::Result<()> {
    // parse / load local plan / execute
    Ok(())
}
```

### Pattern 2: Observer seam for real-time progress
**What:** Introduce a small progress event API between orchestrator and CLI instead of printing from execution code.
**When to use:** Before any `indicatif` rendering.
**Why:** Current `run_phase()` only returns once a phase is done. `OUTP-02` requires updates *during* execution.

**Recommended shape:**
```rust
pub enum ProgressEvent {
    PhaseStarted { phase_id: u32, phase_name: String, total_phases: usize, index: usize },
    TaskStarted { phase_id: u32, task_name: String, agent: AgentKind, total_tasks: usize, index: usize },
    TaskCompleted { phase_id: u32, task_name: String, agent: AgentKind },
    ReviewStarted { phase_id: u32, reviewer: AgentKind, attempt: u32 },
    ReviewFailed { phase_id: u32, reviewer: AgentKind, attempt: u32, reason: String },
    RetryStarted { phase_id: u32, attempt: u32 },
    PhaseCompleted { phase_id: u32, phase_name: String },
}

pub trait ProgressObserver {
    fn on_event(&mut self, event: ProgressEvent);
}
```

This keeps `ath-orchestrator` testable and lets `ath-cli` own the terminal UX.

### Pattern 3: Hybrid board with durable side-channel
**What:** Use `indicatif::MultiProgress` (or a single stable progress area) for transient status, plus durable event lines printed outside the spinner/progress area.
**When to use:** Default terminal mode.
**Why:** User explicitly rejected both a pure spinner and a full event log.

`indicatif` is a good fit because:
- `MultiProgress` supports multiple live bars/spinners in one terminal area
- `println` integration lets durable event lines coexist with progress bars
- it already handles TTY-aware drawing and hidden/no-op rendering more cleanly than manual ANSI work

**Key implementation insight:** The live board should render current phase + overall position + a short task checklist, while retry/review milestones are emitted as durable lines. Do not try to make every event a progress bar row.

### Pattern 4: Verbose mode as a second sink, not a second code path
**What:** Reuse the same observer/event stream, but attach a transcript sink that flushes grouped blocks after task completion.
**When to use:** Plan 03.
**Why:** If verbose mode forks the execution logic, normal and verbose runs will drift.

**Recommended behavior:**
- normal mode: live board + durable milestone lines
- verbose mode: same normal mode output, then grouped transcript blocks per task/review/retry
- redaction happens before transcript rendering, not inside the reporter UI

### Pattern 5: Dry-run boundary after local plan load, before orchestration
**What:** `--dry-run` must short-circuit before any agent request or git call, and only succeed when an `ExecutionPlan` is already available locally.
**When to use:** Plan 04.
**Why:** Current parse/decompose flow is LLM-backed, so it cannot be part of dry-run.

**Recommended shape:**
```rust
fn load_local_execution_plan(path: &Path) -> anyhow::Result<ExecutionPlan> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

if args.dry_run {
    let plan = load_local_execution_plan(plan_path)
        .context("dry-run requires a previously generated local plan")?;
    display_execution_plan(&plan, &[]);
    return Ok(());
}
```

The planner should treat the exact local artifact path as an implementation detail, but the phase must produce a real local-plan success path or `PLAN-05` will remain only partially satisfied.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Subcommand parsing | Manual `std::env::args()` parsing | Existing `clap` derive structs in `ath-cli` | Already established in codebase |
| Plan display formatting | New dry-run formatter from scratch | `display_execution_plan()` / `format_execution_plan()` | Already prints phases, dependencies, and parallel groups |
| Routing logic | Custom task-to-agent mapping in CLI | `assign_all_tasks()` from `router.rs` | Existing Phase 6 routing rules already encode the project contract |
| Parallel conflict checks | Ad hoc file-overlap logic | `check_isolation()` from `isolation.rs` | Existing validation path from Phase 6 |
| Transcript data model | New verbose-only structs | `TaskOutput`, `ReviewAttempt`, `PhaseRecord`, `PhaseStatus` | Existing Phase 7 types already carry execution and review details |
| Terminal ANSI animations | Raw escape-code renderer | `indicatif` progress types + existing `colored` output | More portable, easier to test via formatter seams |

## Common Pitfalls

### Pitfall 1: No event stream means fake "real-time" progress
**What goes wrong:** The CLI only prints updates after `run_phase()` finishes, so the terminal appears frozen during long tasks or reviews.
**Why it happens:** The current orchestrator returns final records, not incremental events.
**How to avoid:** Add a progress observer seam first, then build the reporter on top of it.
**Warning signs:** Phase bars only move once per phase, or retry information appears only after completion.

### Pitfall 2: `--dry-run` accidentally spends API cost
**What goes wrong:** The code reuses `parse_input()` and `decompose_project_spec()` inside dry-run mode.
**Why it happens:** The current run path is LLM-backed all the way through planning.
**How to avoid:** Gate dry-run before any agent backend is constructed for parsing/decomposition, and require a local serialized plan.
**Warning signs:** tests need mocked agent calls for dry-run, or the code path still instantiates planning backends.

### Pitfall 3: Progress bars hide the important retry/review events
**What goes wrong:** Review failures and retries scroll away or disappear when bars redraw.
**Why it happens:** Progress libraries are optimized for transient state, not durable milestones.
**How to avoid:** Keep a durable event channel (`println` / plain lines) for retries, review failures, and phase completion.
**Warning signs:** A user cannot tell why the run paused, retried, or failed from default output.

### Pitfall 4: Verbose mode floods the terminal and becomes unreadable
**What goes wrong:** Prompt/response text streams live while bars update, causing interleaved noise.
**Why it happens:** Transcripts are printed inline with execution events.
**How to avoid:** Buffer transcripts and print grouped blocks after task completion; always redact secrets before rendering.
**Warning signs:** The same terminal region shows both live bars and long multi-line prompts simultaneously.

### Pitfall 5: Reporter logic leaks into orchestrator internals
**What goes wrong:** `run_phase()` starts owning terminal-specific behavior, colors, or progress styles.
**Why it happens:** It is tempting to print where the events occur.
**How to avoid:** Keep orchestrator output abstract (`ProgressEvent` / observer trait). CLI owns rendering.
**Warning signs:** `ath-orchestrator` gains `indicatif`, `colored`, or output-formatting concerns.

### Pitfall 6: Non-TTY and `NO_COLOR` behavior breaks automation
**What goes wrong:** Progress bars write control characters into redirected output or CI logs.
**Why it happens:** The reporter assumes an interactive terminal.
**How to avoid:** Fall back to plain durable lines when `NO_COLOR` is set or stdout/stderr is not interactive.
**Warning signs:** test logs or redirected output contain half-drawn spinner frames or escape sequences.

### Pitfall 7: Tests assert animation frames instead of semantic events
**What goes wrong:** CLI progress tests become flaky because they depend on timing or bar redraw output.
**Why it happens:** Progress rendering is tested at the terminal-frame level.
**How to avoid:** Unit test event-to-view formatting and observer sequencing; reserve only a small manual smoke check for actual terminal animation.
**Warning signs:** tests use sleeps, redraw timing, or fragile substring expectations from spinner frames.

## Code Examples

### Testing the CLI surface without spawning a subprocess
```rust
use clap::{CommandFactory, Parser};

#[test]
fn cli_shape_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn parse_run_dry_run() {
    let cli = Cli::parse_from(["ath", "run", "--dry-run"]);
    assert!(matches!(cli.command, Some(Commands::Run { dry_run: true, .. })));
}
```

### Observer-driven progress reporting
```rust
pub struct TerminalProgressReporter {
    current_phase: Option<String>,
    total_phases: usize,
}

impl ProgressObserver for TerminalProgressReporter {
    fn on_event(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::PhaseStarted { phase_name, total_phases, .. } => {
                self.current_phase = Some(phase_name);
                self.total_phases = total_phases;
            }
            ProgressEvent::ReviewFailed { reason, attempt, .. } => {
                eprintln!("retrying (attempt {}): {}", attempt + 1, reason);
            }
            _ => {}
        }
    }
}
```

### Local plan cache for honest dry-run
```rust
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredPlan {
    plan: ExecutionPlan,
}

fn write_plan_cache(path: &Path, plan: &ExecutionPlan) -> anyhow::Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(&StoredPlan { plan: plan.clone() })?)?;
    Ok(())
}
```

This is viable because `ExecutionPlan` already derives `Serialize` / `Deserialize`.

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| CLI as a thin parser with placeholder commands | Command-specific execution functions plus parse tests | Makes `ath-cli` testable and keeps command growth manageable |
| Plain log spam for progress | `indicatif`-backed hybrid live board with durable milestone lines | Matches the locked UX without a silent terminal |
| Recompute planning inside dry-run | Local serialized plan artifact for dry-run success path | Keeps `PLAN-05` honest and cost-free |
| Separate verbose execution flow | Same observer/event stream with an extra transcript sink | Prevents drift between normal and verbose runs |

## Open Questions

1. **Local plan artifact location**
   - What we know: dry-run success needs a local serialized `ExecutionPlan`
   - What is unclear: whether that should live under a project-local `.ath/` path, an explicit user-provided file, or a generated workspace artifact
   - Recommendation: decide once in planning and keep the storage detail contained to the dry-run module

2. **How much of `ath report` is Phase 8 versus Phase 9**
   - What we know: Phase 8 owns the command/help surface and default target behavior
   - What is unclear: how much placeholder output is needed now versus deferred to Phase 9
   - Recommendation: Phase 8 should establish parsing/help and a minimal human-readable stub that does not preempt Phase 9 reporting internals

3. **Whether to use `tracing-indicatif` directly or behind an adapter**
   - What we know: roadmap explicitly mentions `indicatif + tracing-indicatif`
   - What is unclear: whether the codebase benefits from direct span-based progress immediately, since no tracing subscriber stack exists yet
   - Recommendation: keep a project-owned progress observer interface even if the concrete implementation uses `tracing-indicatif`

## Validation Architecture

### Test Infrastructure
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace test settings |
| Quick run command | `cargo test -p ath-cli` |
| Full suite command | `cargo test --workspace` |
| Estimated runtime | ~20 seconds |

### Sampling Rate
- **After every task commit:** Run `cargo test -p ath-cli`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

### Per-Task Verification Map
| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | SC-01 | unit | `cargo test -p ath-cli cli_surface::tests` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 2 | OUTP-02 | unit | `cargo test -p ath-cli progress::tests` | ❌ W0 | ⬜ pending |
| 08-02-02 | 02 | 2 | OUTP-02 | unit | `cargo test -p ath-orchestrator progress::tests` | ❌ W0 | ⬜ pending |
| 08-03-01 | 03 | 3 | SC-04 | unit | `cargo test -p ath-cli verbose::tests` | ❌ W0 | ⬜ pending |
| 08-04-01 | 04 | 4 | PLAN-05 | unit | `cargo test -p ath-cli dry_run::tests::dry_run_requires_local_plan` | ❌ W0 | ⬜ pending |
| 08-04-02 | 04 | 4 | PLAN-05 | integration | `cargo test -p ath-cli dry_run::tests::dry_run_prints_full_plan_without_execution` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠ flaky*

### Wave 0 Requirements
- [ ] Add `indicatif` to workspace dependencies for progress rendering
- [ ] Add `tracing` and `tracing-subscriber` to workspace dependencies if planner chooses a span-backed reporter
- [ ] Add `tracing-indicatif` to workspace dependencies if planner chooses the roadmap's tracing-integrated path
- [ ] Create progress observer / event infrastructure in `ath-orchestrator`
- [ ] Add CLI test modules for parsing/help/dry-run/progress formatting

*Existing workspace test infrastructure is sufficient once the new modules exist.*

### Manual-Only Verifications
| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Hybrid live board remains readable in a real terminal | OUTP-02 | Animated progress output is difficult to assert reliably in unit tests | Run `ath run` in an interactive terminal and verify current phase, current agent, task checklist, and retry messages stay legible |
| `NO_COLOR` / redirected output remains plain-text friendly | OUTP-02 | Interactivity detection is environment-sensitive | Run `NO_COLOR=1 ath run ...` and `ath run ... > out.txt` and confirm there are no escape codes or broken spinner frames |

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `crates/ath-cli/src/main.rs`
- Codebase inspection: `crates/ath-planner/src/input/mod.rs`
- Codebase inspection: `crates/ath-planner/src/decompose/display.rs`
- Codebase inspection: `crates/ath-types/src/plan.rs`
- Codebase inspection: `crates/ath-types/src/phase.rs`
- Codebase inspection: `crates/ath-orchestrator/src/router.rs`
- Codebase inspection: `crates/ath-orchestrator/src/isolation.rs`
- Codebase inspection: `crates/ath-orchestrator/src/coordinator.rs`
- Codebase inspection: `crates/ath-orchestrator/src/phase_runner.rs`
- Codebase inspection: `Cargo.toml` workspace dependencies

### Secondary (HIGH confidence)
- [`indicatif` docs on docs.rs](https://docs.rs/indicatif/latest/indicatif/)
- [`tracing-indicatif` docs on docs.rs](https://docs.rs/tracing-indicatif/latest/tracing_indicatif/)
- [`clap` docs on docs.rs](https://docs.rs/clap/latest/clap/)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Existing codebase hooks: HIGH - core execution/planning types already exist and were inspected directly
- Progress rendering stack: HIGH - `indicatif` / `tracing-indicatif` docs are current and align with the roadmap direction
- Dry-run constraints: HIGH - derived directly from current code paths plus the locked "no LLM" decision

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable unless the progress library choices change)
