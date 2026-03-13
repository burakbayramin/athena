# Phase 9: Reporting and Error Quality - Research

**Researched:** 2026-03-13
**Domain:** Final run reporting, token/cost accounting, and actionable CLI error surfaces in Athena's Rust workspace
**Confidence:** HIGH

## Summary

Phase 9 has to turn Athena's existing execution data into something durable and user-facing. The codebase already has the core raw material: `AgentCoordinator::run_plan_with_progress()` returns a `Vec<PhaseRecord>`, `PhaseRecord` already captures phase completion and review attempts, `AgentResponse` and `TaskOutput` already carry token counts, and the CLI already exposes `ath report`. What is missing is persistence, aggregation, and presentation.

Three implementation gaps dominate planning:

1. `ath run` currently discards the returned `Vec<PhaseRecord>` after printing a completion line, so there is no saved run artifact for `ath report` to load.
2. `TokenUsage.estimated_cost_usd` exists, but `run_phase_with_progress()` always writes `0.0`, and reviewer token usage is not recorded in `PhaseRecord.contributions` at all.
3. Athena has multiple typed error families (`AgentError`, `PhaseRunnerError`, `ValidationError`, `ConfigError`), but the CLI only renders extra fix guidance for `ConfigError`.

**Primary recommendation:** Plan Phase 9 in four slices:
- persist a structured run report artifact from the existing plan + records
- make `ath report` load the latest run by default and render a human-readable summary
- fill in token/cost accounting, including reviewer usage and provider pricing
- expand CLI error formatting so provider, review, and schema failures include actionable context

This keeps `QUAL-04`, `OUTP-03`, and `OUTP-04` traceable instead of mixing persistence, pricing, and UX changes into one large plan.

<user_constraints>
## User Constraints

No Phase 9 `CONTEXT.md` exists yet. Planning is therefore based on the roadmap, requirements, and current codebase only.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| QUAL-04 | Athena tracks and reports token usage and estimated cost per phase per agent | Existing token counts exist on `AgentResponse`, `TaskOutput`, and `TokenUsage`, but cost calculation and reviewer accounting are incomplete |
| OUTP-03 | Athena produces structured final report: phase table, agent assignments, review outcomes | `ExecutionPlan` plus `Vec<PhaseRecord>` already contain most of the raw data, but nothing persists it to disk |
| OUTP-04 | Error messages include actionable context distinguishing API errors, review failures, and schema violations | Typed errors already exist in multiple crates; CLI rendering only needs to surface them consistently |
</phase_requirements>

## Current Codebase State

### What already exists
- `crates/ath-cli/src/run.rs`
  - Runs parse -> decompose -> route -> isolate -> execute
  - Receives `records` from `AgentCoordinator::run_plan_with_progress()`
  - Persists `.ath/last-plan.json` for dry-run, proving project-local artifact storage is already an accepted Phase 8 pattern
- `crates/ath-cli/src/report.rs`
  - `ath report` exists with latest-run default semantics, but is still a placeholder
- `crates/ath-cli/src/main.rs`
  - Centralized CLI error rendering already unwraps `anyhow::Error` and shows fix hints for `ConfigError`
- `crates/ath-types/src/phase.rs`
  - `PhaseRecord`, `AgentContribution`, `ReviewAttempt`, and `TokenUsage`
  - `TokenUsage` already has `estimated_cost_usd`
- `crates/ath-types/src/agent.rs`
  - `AgentKind` exposes `provider_name()` and `model()`, which is enough to key pricing tables
  - `AgentResponse` records `input_tokens` and `output_tokens`
- `crates/ath-orchestrator/src/coordinator.rs`
  - `run_plan()` and `run_plan_with_progress()` return `Vec<PhaseRecord>`
- `crates/ath-orchestrator/src/phase_runner.rs`
  - Aggregates executor token counts into `PhaseRecord.contributions`
  - Records review attempts and final verdicts
- `crates/ath-agents/src/error.rs`
  - `AgentError` already distinguishes auth, rate limit, timeout, server, and invalid-response cases
- `crates/ath-orchestrator/src/error.rs`
  - `PhaseRunnerError` already distinguishes reviewer, retry, task, review-dispatch, and atomic-write failures
- `crates/ath-types/src/error.rs`
  - `ValidationError` already models empty field, invalid value, circular dependency, missing dependency, orphaned goal, empty phase, and unsatisfied contract cases

### What is missing
- No `RunReport` or equivalent persisted run artifact combines plan metadata with execution outcomes
- No latest-run pointer or per-run directory exists for `ath report`
- `PhaseRecord` captures only `phase_name`; it does not itself persist plan order, dependencies, assigned agents, or parallel grouping
- Reviewer token usage is not added to `contributions`
- `estimated_cost_usd` is always zero in executor contributions
- No provider pricing table or cost-estimation helper exists
- `ath report` cannot yet load JSON or render markdown/human-readable summaries
- CLI error display does not recognize `AgentError`, `PhaseRunnerError`, or `ValidationError` for richer output

## Key Findings From Code Inspection

### 1. Reporting needs both `ExecutionPlan` and `PhaseRecord`
`PhaseRecord` alone is not enough for `OUTP-03`. The report requirement explicitly wants phase/task/agent/dependency/parallelism tables. The execution-time `PhaseRecord` holds completion timestamps, contributions, and review attempts, but not the dependency graph or assigned-agent plan snapshot. That data still lives in `ExecutionPlan`.

**Planning implication:** The persisted report artifact should snapshot both:
- routed `ExecutionPlan`
- completed `Vec<PhaseRecord>`

The most natural place to build that artifact is immediately after `run_command()` gets both values.

### 2. Cost tracking is only half-wired
`run_phase_with_progress()` adds executor token counts into `AgentContribution.tokens`, but always sets `estimated_cost_usd: 0.0`. It also never adds the reviewer response's token usage into contributions.

**Planning implication:** Phase 9 should not just "format existing cost data". It has to finish token accounting first:
- add reviewer token tracking
- calculate cost from provider/model/token counts
- aggregate totals for final reports

### 3. Project-local artifact storage is already established
Phase 8 introduced `.ath/last-plan.json`. That means Phase 9 does not need to invent a new storage philosophy.

**Planning implication:** A run artifact layout under `.ath/` is consistent with current behavior, for example:
```text
.ath/
  last-plan.json
  runs/
    latest.json
    2026-03-13T10-30-00Z/
      report.json
      report.md
```

This gives `ath report` a stable default lookup path and an explicit target path story.

### 4. Error quality is mostly a CLI formatting problem, not a missing type problem
The workspace already has typed errors:
- `AgentError` classifies provider failures
- `PhaseRunnerError` has execution/review context plus `hint()`
- `ValidationError` has schema/validation context plus `hint()`
- `ConfigError` already has `hint()`

But `crates/ath-cli/src/main.rs` only checks `ConfigError` specially.

**Planning implication:** The error-quality work should focus on:
- preserving typed errors through `anyhow` wrapping
- rendering provider/reviewer/schema context in `display_error()`
- adding tests for user-facing output

### 5. No new crate is obviously required
The existing crate boundaries are already usable:
- `ath-types` for serializable report types
- `ath-orchestrator` for execution-time record completeness
- `ath-cli` for persistence, loading, rendering, and user-facing errors

**Planning implication:** Phase 9 can stay incremental by extending existing crates rather than introducing a new reporting crate.

## Architecture Patterns

### Pattern 1: Persist a project-local run report artifact
**What:** Save a structured run report under `.ath/runs/...` after every successful `ath run`.
**Why:** `ath report` needs a durable source of truth, and the latest-run default must resolve without re-executing or re-planning.

**Recommended shape:**
```rust
#[derive(Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: String,
    pub generated_at: DateTime<Utc>,
    pub plan: ExecutionPlan,
    pub phase_records: Vec<PhaseRecord>,
    pub totals: ReportTotals,
}
```

### Pattern 2: Keep the report writer in the CLI layer
**What:** Build and write report artifacts from `run.rs`, where both the routed plan and finished records already meet.
**Why:** The orchestrator should stay execution-focused. It should not know about report directories, latest-run pointers, or markdown rendering.

### Pattern 3: Separate pricing from aggregation
**What:** Use a small provider/model pricing helper to calculate per-contribution cost, then aggregate totals in a report builder.
**Why:** Pricing tables change for different providers/models; report assembly should not hard-code token math.

**Recommended split:**
- `cost.rs` or similar: provider/model -> per-token pricing
- report builder: contribution/review totals -> phase totals -> run totals

### Pattern 4: Render human-readable reports from structured data
**What:** `ath report` should load structured data first, then render terminal-friendly summaries.
**Why:** The roadmap wants structured final output and a human-readable default report. Persisting JSON first keeps the report command deterministic and testable.

### Pattern 5: Centralize CLI error rendering
**What:** Extend the existing `display_error()` path in `main.rs` to recognize and format provider, review, and schema failures.
**Why:** Error quality is a UX concern. Keeping it centralized avoids each command inventing its own ad hoc formatting rules.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Run execution history | Ad hoc text dumps from `run.rs` | Structured serializable report type plus JSON persistence | Keeps `ath report` deterministic |
| Phase summary tables | Re-derive dependencies from `PhaseRecord` only | Persist routed `ExecutionPlan` inside the report artifact | That metadata already exists before execution |
| Provider naming in reports | String parsing on debug output | `AgentKind::provider_name()` and `AgentKind::model()` | Already normalized in `ath-types` |
| Review status output | Infer from raw transcript text | `ReviewAttempt.verdict` and timestamps | Already typed and serialized |
| Error categorization | Regex over error strings | Existing `AgentError`, `PhaseRunnerError`, `ValidationError`, `ConfigError` variants | More reliable and testable |

## Common Pitfalls

### Pitfall 1: Saving only `PhaseRecord`
**What goes wrong:** The final report cannot show dependencies, parallel groups, or planned agent assignments.
**How to avoid:** Persist the routed `ExecutionPlan` alongside completed phase records.

### Pitfall 2: Forgetting reviewer token usage
**What goes wrong:** Phase totals and overall cost are understated, breaking `QUAL-04`.
**How to avoid:** Record review response usage as its own contribution or explicit review-usage field before building totals.

### Pitfall 3: Hard-coding pricing into presentation code
**What goes wrong:** Cost math becomes duplicated across report writing and human-readable rendering.
**How to avoid:** Isolate pricing lookup and cost calculation into a dedicated helper/module.

### Pitfall 4: Losing typed errors behind `anyhow`
**What goes wrong:** CLI messages degrade back into generic strings, even though the lower layers know the provider/phase/schema context.
**How to avoid:** Preserve concrete errors as sources and downcast them in one place in `display_error()`.

### Pitfall 5: Making `ath report` depend on live execution state
**What goes wrong:** The command cannot report on prior runs or explicit targets.
**How to avoid:** Build `ath report` around persisted artifacts plus a latest-run pointer.

## Planning Recommendation

The roadmap's four plan slots map cleanly to these implementation slices:

1. **09-01: Run report artifact and persistence**
   - add serializable run-report types
   - save JSON/markdown plus latest-run pointer under `.ath/runs`
   - invoke writer at the end of `ath run`

2. **09-02: Report command loading and terminal rendering**
   - load latest or explicit target
   - render human-readable phase table, review outcomes, totals, and locations
   - add file-loading and formatting tests

3. **09-03: Token/cost completion**
   - include reviewer usage
   - add provider/model pricing helpers
   - roll up per-agent, per-phase, and run totals

4. **09-04: Actionable error surfaces**
   - extend CLI error rendering for provider, review, and schema failures
   - ensure messages identify the provider/phase/field and give next-step guidance

## Validation Architecture

### Test Infrastructure
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace test settings |
| Quick run command | `cargo test -p ath-cli` |
| Full suite command | `cargo test --workspace` |
| Estimated runtime | ~25 seconds |

### Sampling Rate
- **After every task commit:** Run `cargo test -p ath-cli`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

### Per-Task Verification Map
| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::writes_run_report_artifacts` | ❌ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::latest_run_default_resolves` | ❌ W0 | ⬜ pending |
| 09-02-01 | 02 | 2 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::renders_human_readable_summary` | ❌ W0 | ⬜ pending |
| 09-03-01 | 03 | 3 | QUAL-04 | unit | `cargo test -p ath-orchestrator phase_runner::tests::run_phase_record_tracks_token_usage` | ✅ | ⬜ pending |
| 09-03-02 | 03 | 3 | QUAL-04 | unit | `cargo test -p ath-cli report::tests::computes_per_phase_and_run_costs` | ❌ W0 | ⬜ pending |
| 09-04-01 | 04 | 4 | OUTP-04 | unit | `cargo test -p ath-cli error_display::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠ flaky*

### Wave 0 Requirements
- [ ] Add serializable run-report types and fixtures for report persistence tests
- [ ] Add report loading/rendering test modules in `ath-cli`
- [ ] Add pricing-fixture coverage for provider/model cost calculations
- [ ] Add CLI error-output tests that assert provider/review/schema context and hints

*Existing cargo-based test infrastructure covers the rest once these modules exist.*

### Manual-Only Verifications
| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Human-readable report is legible in a real terminal | OUTP-03 | Table readability and line wrapping are presentation-sensitive | Run `ath report` on a saved run and verify the phase/review/cost summary is readable without raw JSON inspection |
| Actionable error messages are understandable to users | OUTP-04 | Best judged by the final composed CLI output, not just variant-level unit tests | Trigger one provider auth error, one review failure, and one schema/validation failure; confirm the terminal message identifies the source and next step |

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `crates/ath-cli/src/run.rs`
- Codebase inspection: `crates/ath-cli/src/report.rs`
- Codebase inspection: `crates/ath-cli/src/main.rs`
- Codebase inspection: `crates/ath-types/src/phase.rs`
- Codebase inspection: `crates/ath-types/src/agent.rs`
- Codebase inspection: `crates/ath-types/src/error.rs`
- Codebase inspection: `crates/ath-types/src/review.rs`
- Codebase inspection: `crates/ath-orchestrator/src/coordinator.rs`
- Codebase inspection: `crates/ath-orchestrator/src/phase_runner.rs`
- Codebase inspection: `crates/ath-orchestrator/src/error.rs`
- Codebase inspection: `crates/ath-agents/src/error.rs`
- Codebase inspection: `crates/ath-config/src/error.rs`

### Secondary (HIGH confidence)
- None needed; the implementation questions are driven directly by the current codebase

## Metadata

**Confidence breakdown:**
- Current reporting seams: HIGH - inspected directly in CLI/orchestrator/types code
- Token/cost gaps: HIGH - inspected directly where `PhaseRecord` is built
- Error-quality scope: HIGH - typed error families already exist and were inspected directly

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 unless the report storage layout or crate boundaries change