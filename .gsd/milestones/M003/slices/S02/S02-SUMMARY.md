---
id: S02
parent: M003
milestone: M003
provides:
  - PhaseRestored progress event for skipped-on-resume phases
  - Checkpoint-aware run_plan_with_progress with save/restore/skip/cleanup logic
  - Checkpoint-aware run_plan_with_memory with same checkpoint support
  - Integration tests proving resume, stale detection, and partial failure checkpoint persistence
requires:
  - slice: S01
    provides: Checkpoint, CheckpointStore, plan_fingerprint, should_skip_group
affects:
  - S03
key_files:
  - crates/ath-orchestrator/src/coordinator.rs
  - crates/ath-orchestrator/src/memory.rs
  - crates/ath-orchestrator/src/progress.rs
  - crates/ath-cli/src/progress.rs
key_decisions:
  - "Checkpoint save uses let _ = (fire-and-forget) — checkpoint persistence failure shouldn't crash the run"
  - "Stale checkpoint produces PhaseRunnerError, not silent discard — user must explicitly use --fresh"
  - "Pre-populate results from checkpoint.completed_records so final Vec has all phases"
patterns_established:
  - "checkpoint_path: Option<&Path> as opt-in parameter — None means no checkpointing"
  - "PhaseRestored event mirrors PhaseCompleted fields for consistent progress rendering"
observability_surfaces:
  - "PhaseRestored progress events in terminal output — 'restored from checkpoint' in interactive, 'Phase N/M restored from checkpoint' in plain"
  - ".ath/checkpoint.json — present = incomplete run, absent = clean state"
drill_down_paths:
  - .gsd/milestones/M003/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003/slices/S02/tasks/T02-SUMMARY.md
duration: 25m
verification_result: passed
completed_at: 2026-03-14
---

# S02: Coordinator Integration

**Checkpoint save/restore wired into both coordinator and memory-aware execution paths — resume, stale detection, and partial failure proven by 3 integration tests.**

## What Happened

Two tasks wired checkpoint support into both execution paths:

**T01** added `PhaseRestored` to `ProgressEvent`, then modified `run_plan_with_progress` to accept `checkpoint_path: Option<&Path>`. On resume: loads checkpoint, validates plan fingerprint (stale → error with fingerprint diff), pre-populates results, skips completed groups with PhaseRestored events, saves after each group, deletes on success. Updated CLI progress handler for PhaseRestored in both interactive and plain modes. Three integration tests prove the critical flows.

**T02** applied identical checkpoint logic to `run_plan_with_memory`. Same load/validate/skip/save/cleanup pattern. Post-run extraction still runs on resumed runs (intentional — extraction should capture memory from new phases). Updated 7 existing test call sites.

## Verification

- `cargo test -p ath-orchestrator -- checkpoint` — 19/19 pass (16 unit + 3 integration)
- `cargo test -p ath-orchestrator -- memory` — 13/13 pass (all existing memory tests unchanged)
- `cargo test --workspace` — 607 passed, 0 failed
- `cargo check --workspace` — clean
- `checkpoint_resume_skips_completed_phases`: 2 PhaseRestored events emitted, 3 total records returned, checkpoint deleted
- `checkpoint_stale_plan_rejected`: error contains "Stale checkpoint" with fingerprints
- `checkpoint_saved_after_each_group`: phase 1 record saved after phase 2 fails

## Deviations

None.

## Files Created/Modified

- `crates/ath-orchestrator/src/progress.rs` — PhaseRestored event variant
- `crates/ath-orchestrator/src/coordinator.rs` — checkpoint-aware run_plan_with_progress, TestProgressObserver, 3 integration tests
- `crates/ath-orchestrator/src/memory.rs` — checkpoint-aware run_plan_with_memory, updated 7 test call sites
- `crates/ath-cli/src/progress.rs` — PhaseRestored in apply_event, milestone_line, plain_line
- `crates/ath-cli/src/run.rs` — updated call site with None

## Forward Intelligence

### What the next slice should know
- `run_plan_with_progress(&plan, observer, Some(&cp_path))` enables checkpointing. Pass `None` to disable.
- The CLI needs to compute `cp_path` (e.g. `.ath/checkpoint.json`) and pass it through.
- `--fresh` should just delete the checkpoint file before calling `run_plan_with_progress`.
- `--status` should load the checkpoint and display it without running anything.

### What's fragile
- Checkpoint save is fire-and-forget (`let _ =`). If the filesystem is full, checkpoint save silently fails and the user loses resumability. Acceptable tradeoff — crashing the run over a checkpoint save would be worse.
