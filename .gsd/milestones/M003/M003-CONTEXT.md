# M003: Resumable Execution — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Add checkpoint-based run resumption so that when an `ath run` fails mid-execution (API error, review exhaustion, crash, Ctrl+C), the user can re-run `ath run` and pick up from the last successfully completed phase group — without re-calling LLMs for work already done.

## Why This Milestone

Every failed run currently wastes all completed work. A 5-phase project where phase 4 fails means phases 1–3's LLM calls, review cycles, and file writes are discarded. The user must pay for and wait through everything again. This is the most commonly requested v2 feature and directly reduces cost and frustration.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Run `ath run "build a todo API"`, have it fail at phase 3 of 5, then re-run `ath run` and see it resume from phase 3 — phases 1–2 are skipped with a "restored from checkpoint" message
- Run `ath run --fresh` to force a clean start, ignoring any existing checkpoint
- See clear terminal output indicating which phases were restored vs. executed fresh
- Run `ath run --status` to see the state of an incomplete run without executing anything

### Entry point / environment

- Entry point: `ath run` (automatic checkpoint detection), `ath run --fresh` (force clean start), `ath run --status` (inspect checkpoint)
- Environment: local dev — CLI binary
- Live dependencies involved: same LLM APIs as normal execution

## Completion Class

- Contract complete means: checkpoint write/read/skip logic passes unit tests with fixture plans and records
- Integration complete means: a coordinator run that fails at phase N can resume from phase N on next invocation, proven by integration test with mock backends
- Operational complete means: checkpoint files are human-inspectable JSON, `--fresh` cleans up stale checkpoints, progress output clearly distinguishes restored vs. fresh phases

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A two-invocation scenario: first run fails at phase N, second run resumes from phase N and completes — proven by integration test with sequenced mock backends
- `ath run --fresh` ignores an existing checkpoint and starts clean
- `ath run --status` shows the checkpoint state without executing
- Checkpoint is cleaned up after successful completion

## Risks and Unknowns

- **Plan identity** — resuming requires verifying the checkpoint's plan matches the current project state. If the user changes their description between runs, resuming from a stale checkpoint would produce incoherent results. Need a plan fingerprint.
- **File state drift** — phases 1–2 wrote files to disk. If the user manually edited those files between runs, resuming from phase 3 may produce conflicts. Need to decide: warn, ignore, or block.
- **Memory integration** — `run_plan_with_memory` in memory.rs duplicates the coordinator loop. Resumability must work with both paths.

## Existing Codebase / Prior Art

- `crates/ath-orchestrator/src/coordinator.rs` — `run_plan_with_progress` iterates `parallel_groups` and returns `Vec<PhaseRecord>`. This is where checkpoint save/restore hooks go.
- `crates/ath-orchestrator/src/memory.rs` — `run_plan_with_memory` duplicates the group iteration loop. Must also support checkpointing.
- `crates/ath-cli/src/run.rs` — `run_command()` calls coordinator and builds report. Checkpoint detection and `--fresh`/`--status` flags go here.
- `crates/ath-cli/src/dry_run.rs` — `write_plan_cache` / `load_plan_cache` already cache the execution plan at `.ath/last-plan.json`. Checkpoint will extend this pattern.
- `crates/ath-cli/src/report.rs` — `write_run_report` persists completed runs at `.ath/runs/<run-id>/report.json`. Checkpoint is the pre-completion equivalent.
- `crates/ath-types/src/phase.rs` — `PhaseRecord` is the unit of completed work. Checkpoint stores accumulated `Vec<PhaseRecord>`.
- `crates/ath-types/src/plan.rs` — `ExecutionPlan` is serde-serializable. Plan fingerprinting will hash this.

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions — it is an append-only register; read it during planning, append to it during execution.

## Relevant Requirements

- REQ-RESUME: Resumable execution from last completed phase — currently out of scope (v2), this milestone validates it

## Scope

### In Scope

- Checkpoint file persistence at `.ath/checkpoint.json` (plan + completed records + run metadata)
- Plan fingerprint for stale-checkpoint detection
- Resume logic in coordinator that skips completed phase groups
- `--fresh` flag to force clean start
- `--status` flag to inspect checkpoint state
- Progress output for restored phases ("Restored from checkpoint")
- Checkpoint cleanup after successful completion
- Works with both regular and memory-aware execution paths

### Out of Scope / Non-Goals

- Resuming mid-phase (within a review retry cycle) — checkpoint granularity is per phase group
- Automatic plan re-derivation on resume — the user must provide the same project description
- Checkpoint migration across Athena versions — if the plan or record schema changes, checkpoint is invalidated
- Distributed or multi-machine checkpointing

## Technical Constraints

- Checkpoint must be a single JSON file for simplicity and atomic write safety
- Plan fingerprint must be deterministic (same plan → same fingerprint, regardless of serialization order)
- Checkpoint write must be atomic (temp file + rename) to prevent corruption on crash
- Must not break any existing tests (coordinator, phase_runner, CLI)

## Integration Points

- `ath-orchestrator` — coordinator gets checkpoint save/load/skip logic
- `ath-cli` — run.rs gets `--fresh`, `--status` flags and checkpoint detection
- `ath-types` — new `Checkpoint` type in a new module or in report.rs
- `ath-orchestrator/memory.rs` — memory-aware execution path also needs checkpoint support

## Open Questions

- **Plan fingerprint scope**: hash the full serialized plan, or just phases + execution_order? Leaning toward full plan hash — simplest, catches any change.
- **Partial group completion**: if a parallel group of 3 phases completes 2 before the 3rd fails, do we checkpoint the 2 completed phases or discard the whole group? Leaning toward discarding the group — simpler, avoids partial state.
