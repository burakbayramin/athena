---
id: M003
provides:
  - Checkpoint struct with plan fingerprint, completed records, phase skip logic
  - plan_fingerprint() — deterministic SHA-256 hash of execution plan
  - CheckpointStore with atomic JSON persistence (save/load/delete)
  - Checkpoint-aware run_plan_with_progress and run_plan_with_memory
  - PhaseRestored progress event for skipped-on-resume phases
  - CLI --fresh and --status flags for checkpoint management
  - Automatic checkpoint resume via .ath/checkpoint.json
key_decisions:
  - "D021: SHA-256 on serde_json::to_string for plan fingerprint — deterministic because serde field order follows declaration order"
  - "D022: Checkpoint save is fire-and-forget (let _ =) — persistence failure shouldn't crash the run"
  - "D023: Stale checkpoint produces error, not silent discard — user must use --fresh explicitly"
  - "D024: Checkpoint granularity is per phase group, not per phase — simpler, avoids partial group state"
  - "D025: PhaseRestored mirrors PhaseCompleted fields for consistent progress rendering"
patterns_established:
  - "checkpoint_path: Option<&Path> as opt-in parameter — None means no checkpointing"
  - "Checkpoint::should_skip_group + is_stale as decision points in the coordinator loop"
  - "show_checkpoint_status pattern: load → display summary → suggest next action"
observability_surfaces:
  - ".ath/checkpoint.json — present = incomplete run, absent = clean state"
  - "ath run --status — inspect checkpoint without executing"
  - "PhaseRestored events in terminal — 'restored from checkpoint' messages"
requirement_outcomes:
  - id: REQ-RESUME
    from_status: active
    to_status: validated
    proof: "checkpoint_resume_skips_completed_phases integration test proves two-invocation resume. checkpoint_saved_after_each_group proves partial failure persistence. CLI --fresh/--status flags tested. 612 tests pass including 24 new checkpoint tests."
duration: ~1h
verification_result: passed
completed_at: 2026-03-14
---

# M003: Resumable Execution

**Checkpoint-based run resumption — failed runs resume from the last completed phase group, with stale detection, --fresh/--status CLI flags, and automatic cleanup on success — proven by 24 new tests, 612 total passing.**

## What Happened

Three slices built resumable execution from storage primitives to CLI integration:

**S01 (Checkpoint Types & Persistence)** created the `Checkpoint` struct in ath-orchestrator with run identity, plan fingerprint (SHA-256 of canonical JSON), completed records with HashSet-backed phase ID tracking, and atomic JSON persistence via temp-file + rename. `plan_fingerprint()` is deterministic — same plan always produces the same hash. `should_skip_group()` and `is_stale()` provide the decision logic for resume. 16 unit tests.

**S02 (Coordinator Integration)** wired checkpoint logic into both execution paths. `run_plan_with_progress` and `run_plan_with_memory` now accept `checkpoint_path: Option<&Path>`. On resume: loads checkpoint, validates fingerprint (stale → error), pre-populates results from completed records, skips completed groups with `PhaseRestored` progress events, saves after each group, and deletes checkpoint after all groups succeed. 3 integration tests prove resume, stale detection, and partial-failure persistence. All 148 existing orchestrator tests pass unchanged.

**S03 (CLI & UX)** connected the checkpoint machinery to `ath run`. Every run now passes `.ath/checkpoint.json` to the coordinator. `--fresh` deletes the checkpoint before running. `--status` loads and displays checkpoint summary without executing. Terminal output shows "restored from checkpoint" for skipped phases. 5 CLI tests.

## Cross-Slice Verification

| Success Criterion | Evidence |
|---|---|
| Resume from checkpoint — phases before N skipped | `checkpoint_resume_skips_completed_phases`: phases 1-2 restored, phase 3 executed fresh, 3 total records |
| `--fresh` ignores checkpoint | `fresh_flag_deletes_checkpoint` test + CLI flag deletes file before proceeding |
| `--status` shows checkpoint state | `show_status_existing_checkpoint` and `show_status_missing_checkpoint` tests |
| Checkpoint cleaned up after success | `checkpoint_resume_skips_completed_phases` asserts `!cp_path.exists()` |
| Stale checkpoints detected | `checkpoint_stale_plan_rejected` test: error contains "Stale checkpoint" with fingerprint diff |
| Existing test suite passes | `cargo test --workspace` — 612 passed, 0 failed (588 pre-existing + 24 new) |

## Requirement Changes

- REQ-RESUME: active → validated — checkpoint-based resumption proven by integration tests, CLI flags tested, 612 tests pass

## Forward Intelligence

### What the next milestone should know
- `ath run` now always checkpoints to `.ath/checkpoint.json`. On resume, completed phase groups are skipped and `PhaseRestored` events are emitted. On success, checkpoint is deleted.
- Checkpoint granularity is per phase group — mid-phase resume (within retry cycles) is not supported.
- `run_plan_with_progress` signature changed: 3rd parameter `checkpoint_path: Option<&Path>`. Pass `None` to disable.
- `run_plan_with_memory` also has checkpoint support via the same parameter (4th positional).

### What's fragile
- Plan fingerprint depends on exact `ExecutionPlan` serialization — struct field changes invalidate all existing checkpoints (intentional).
- Checkpoint save is fire-and-forget — filesystem full → silent loss of resumability.
- `memory.rs` still duplicates the coordinator loop. Adding checkpoint support doubled the maintenance surface for the group iteration logic.

### Authoritative diagnostics
- `cargo test -p ath-orchestrator -- checkpoint` — 19 tests covering all checkpoint paths
- `.ath/checkpoint.json` — presence means incomplete run; inspect JSON directly for run state
- `ath run --status` — human-readable checkpoint summary

### What assumptions changed
- No assumptions changed — all three slices went as planned.

## Files Created/Modified

- `crates/ath-orchestrator/src/checkpoint.rs` — new: Checkpoint, CheckpointStore, plan_fingerprint, 16 unit tests
- `crates/ath-orchestrator/src/coordinator.rs` — checkpoint-aware run_plan_with_progress, TestProgressObserver, 3 integration tests
- `crates/ath-orchestrator/src/memory.rs` — checkpoint-aware run_plan_with_memory
- `crates/ath-orchestrator/src/progress.rs` — PhaseRestored event variant
- `crates/ath-orchestrator/src/lib.rs` — pub mod checkpoint
- `crates/ath-orchestrator/Cargo.toml` — sha2 dependency
- `crates/ath-cli/src/run.rs` — --fresh/--status flags, checkpoint_path(), show_checkpoint_status(), 5 tests
- `crates/ath-cli/src/progress.rs` — PhaseRestored handling
- `crates/ath-cli/src/dry_run.rs` — RunArgs field updates
- `Cargo.toml` — sha2 workspace dependency
