# S02: Coordinator Integration

**Goal:** Both `run_plan_with_progress` and `run_plan_with_memory` support checkpoint save/restore — skip completed groups on resume, save after each group, clean up on success
**Demo:** Integration tests prove: a run that fails at group N can resume from group N on second invocation; checkpoint is cleaned up after success; stale checkpoint is detected

## Must-Haves

- `PhaseRestored` progress event variant for skipped phases
- `run_plan_with_progress` saves checkpoint after each group and skips groups that are already complete
- `run_plan_with_memory` same checkpoint support
- Checkpoint cleanup on successful completion
- Stale checkpoint detection with clear error
- Integration test: two-invocation resume scenario with mock backends

## Proof Level

- This slice proves: integration
- Real runtime required: no (mock backends)
- Human/UAT required: no

## Verification

- `cargo test -p ath-orchestrator -- checkpoint` — all tests pass (unit + integration)
- `cargo test -p ath-orchestrator` — all tests pass (no regressions)
- `cargo check --workspace` — clean

## Tasks

- [x] **T01: Add PhaseRestored event and checkpoint-aware coordinator** `est:30m`
  - Why: Wire checkpoint save/restore into the coordinator's group loop
  - Files: `crates/ath-orchestrator/src/progress.rs`, `crates/ath-orchestrator/src/coordinator.rs`
  - Do:
    1. Add `PhaseRestored { phase_id, phase_name, phase_index, total_phases }` to `ProgressEvent`
    2. Add `checkpoint_path: Option<PathBuf>` parameter to `run_plan_with_progress`
    3. At start: if checkpoint_path is Some, load checkpoint. If stale, return error. If valid, pre-populate results from completed_records
    4. In the group loop: if checkpoint says skip this group, emit PhaseRestored events and continue
    5. After each group completes: save checkpoint with accumulated records
    6. After all groups complete: delete checkpoint file
    7. Update `run_plan` to pass `None` for checkpoint_path (backward compat)
    8. Add tests: resume skips completed group, checkpoint cleaned on success, stale checkpoint rejected
  - Verify: `cargo test -p ath-orchestrator -- checkpoint`
  - Done when: resume integration test passes

- [x] **T02: Checkpoint support in memory-aware path** `est:20m`
  - Why: `run_plan_with_memory` duplicates the coordinator loop — needs same checkpoint hooks
  - Files: `crates/ath-orchestrator/src/memory.rs`
  - Do:
    1. Add `checkpoint_path: Option<PathBuf>` parameter to `run_plan_with_memory`
    2. Same logic: load at start, skip completed groups, save after each, delete on success
    3. Emit PhaseRestored for skipped phases
    4. Post-run extraction still runs even on resumed runs
    5. Add test: memory-aware resume works
  - Verify: `cargo test -p ath-orchestrator -- memory`
  - Done when: memory resume test passes, all existing memory tests still pass

## Files Likely Touched

- `crates/ath-orchestrator/src/progress.rs` — PhaseRestored event
- `crates/ath-orchestrator/src/coordinator.rs` — checkpoint-aware run_plan_with_progress
- `crates/ath-orchestrator/src/memory.rs` — checkpoint-aware run_plan_with_memory
