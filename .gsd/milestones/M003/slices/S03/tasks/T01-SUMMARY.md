---
id: T01
result: passed
---

# T01: Wire checkpoint into ath run and add --fresh/--status flags

Added `--fresh` and `--status` flags to `RunArgs`. `--status` loads and displays checkpoint summary (run ID, fingerprint, completed phases, timestamps) or reports "no checkpoint found". `--fresh` deletes checkpoint before proceeding. Normal `ath run` passes `.ath/checkpoint.json` to `run_plan_with_progress` for automatic resume.

5 tests: checkpoint path computation, status with missing/existing/corrupt checkpoint, fresh flag deletes checkpoint.

## Files
- `crates/ath-cli/src/run.rs` — checkpoint_path(), show_checkpoint_status(), --fresh/--status flags, 5 tests
- `crates/ath-cli/src/dry_run.rs` — updated RunArgs constructions with new fields
