---
id: S03
parent: M003
milestone: M003
provides:
  - "--fresh flag to force clean start (deletes existing checkpoint)"
  - "--status flag to inspect checkpoint state without executing"
  - "Automatic checkpoint resume via .ath/checkpoint.json path wired into run_plan_with_progress"
  - "show_checkpoint_status() with summary display (run ID, fingerprint, completed phases, timestamps)"
requires:
  - slice: S01
    provides: Checkpoint, CheckpointStore
  - slice: S02
    provides: checkpoint-aware run_plan_with_progress
affects: []
key_files:
  - crates/ath-cli/src/run.rs
key_decisions:
  - "Checkpoint path is .ath/checkpoint.json — same directory as existing plan cache and run reports"
  - "--status uses CheckpointStore::load directly — no coordinator needed"
  - "--fresh deletes file before computing plan — clean separation from plan derivation"
patterns_established:
  - "show_checkpoint_status pattern: load + display + suggest next action"
observability_surfaces:
  - "ath run --status — primary checkpoint inspection command"
  - ".ath/checkpoint.json presence — indicates incomplete run"
drill_down_paths:
  - .gsd/milestones/M003/slices/S03/tasks/T01-SUMMARY.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S03: CLI & UX

**`ath run` resumes automatically from checkpoint, `--fresh` forces clean start, `--status` shows checkpoint state — 5 new tests, 612 total passing.**

## What Happened

Single task wired the checkpoint machinery into the CLI. Added `--fresh` and `--status` flags to `RunArgs`. `show_checkpoint_status()` loads the checkpoint and displays a structured summary: run ID, plan fingerprint (truncated to 16 chars), completed phase count and names, and timestamps. Missing checkpoint → "No checkpoint found" message. Corrupt checkpoint → error. Normal `ath run` now passes `.ath/checkpoint.json` to `run_plan_with_progress`, enabling automatic resume from the last completed phase group.

## Verification

- `cargo test -p ath-cli -- run::tests` — 7/7 pass (2 existing + 5 new)
- `cargo test --workspace` — 612 passed, 0 failed
- `cargo check --workspace` — clean
- checkpoint_path computed correctly (project-relative)
- --status on missing checkpoint → Ok (no checkpoint found)
- --status on existing checkpoint → Ok (summary displayed)
- --status on corrupt checkpoint → Err with "Failed to load"
- --fresh deletes checkpoint file

## Deviations

None.

## Files Created/Modified

- `crates/ath-cli/src/run.rs` — checkpoint_path(), show_checkpoint_status(), --fresh/--status flags, 5 tests
- `crates/ath-cli/src/dry_run.rs` — updated RunArgs constructions with fresh/status fields

## Forward Intelligence

### What the next milestone should know
- M003 is complete. `ath run` automatically resumes from `.ath/checkpoint.json`. `--fresh` forces clean start. `--status` shows checkpoint state.
- Checkpoint granularity is per phase group — mid-phase resume is not supported.
