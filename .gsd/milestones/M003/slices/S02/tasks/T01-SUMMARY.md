---
id: T01
result: passed
---

# T01: Add PhaseRestored event and checkpoint-aware coordinator

Added `PhaseRestored` variant to `ProgressEvent`. Modified `run_plan_with_progress` to accept `checkpoint_path: Option<&Path>`. On resume: loads checkpoint, validates fingerprint (stale → error), pre-populates results from completed_records, skips completed groups with PhaseRestored events, saves checkpoint after each group, deletes checkpoint on success. `run_plan` backward compat with `None`. CLI progress handler renders "restored from checkpoint" for both interactive and plain modes.

3 integration tests: resume skips completed phases (2 restored + 1 fresh = 3 total), stale plan rejected, checkpoint saved after partial failure (phase 1 passes, phase 2 fails → checkpoint has phase 1).

## Files
- `crates/ath-orchestrator/src/progress.rs` — PhaseRestored event variant
- `crates/ath-orchestrator/src/coordinator.rs` — checkpoint-aware run_plan_with_progress, 3 integration tests
- `crates/ath-cli/src/progress.rs` — PhaseRestored handling in apply_event, milestone_line, plain_line
- `crates/ath-cli/src/run.rs` — updated call site with None checkpoint_path
