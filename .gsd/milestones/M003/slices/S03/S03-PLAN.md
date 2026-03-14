# S03: CLI & UX

**Goal:** `ath run` detects checkpoints and resumes automatically, `--fresh` forces clean start, `--status` shows checkpoint state, terminal output distinguishes restored vs. fresh phases
**Demo:** CLI integration tests prove all three flags work correctly on real checkpoint data

## Must-Haves

- `ath run` passes checkpoint path to coordinator (automatic resume)
- `--fresh` flag deletes checkpoint before running
- `--status` flag shows checkpoint state without executing
- Terminal output shows "restored from checkpoint" for skipped phases
- Checkpoint path is `.ath/checkpoint.json` (project-relative)

## Proof Level

- This slice proves: integration (CLI → coordinator → checkpoint)
- Real runtime required: no (test coverage)
- Human/UAT required: no

## Verification

- `cargo test -p ath-cli` — all tests pass
- `cargo check --workspace` — clean
- `cargo test --workspace` — all tests pass, no regressions

## Tasks

- [x] **T01: Wire checkpoint into ath run and add --fresh/--status flags** `est:25m`
  - Why: Final assembly — connects checkpoint machinery to the CLI entry point
  - Files: `crates/ath-cli/src/run.rs`, `crates/ath-cli/src/main.rs`
  - Do:
    1. Add `--fresh` and `--status` flags to `RunArgs`
    2. Compute checkpoint path: `project_dir.join(".ath").join("checkpoint.json")`
    3. If `--fresh`: delete checkpoint file before proceeding
    4. If `--status`: load checkpoint and display summary (completed phases, fingerprint, timestamps), return without executing
    5. Pass `Some(&checkpoint_path)` to `run_plan_with_progress` (and `run_plan_with_memory` if memory is enabled)
    6. Add tests:
       - `--status` on existing checkpoint shows summary
       - `--status` on missing checkpoint says "no checkpoint found"
       - `--fresh` deletes existing checkpoint
       - Checkpoint path computed correctly
  - Verify: `cargo test -p ath-cli` + `cargo test --workspace`
  - Done when: all CLI tests pass, no regressions

## Files Likely Touched

- `crates/ath-cli/src/run.rs` — --fresh, --status flags, checkpoint path wiring
- `crates/ath-cli/src/main.rs` — possible adjustments
