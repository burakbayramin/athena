---
id: T02
result: passed
---

# T02: Checkpoint support in memory-aware path

Added `checkpoint_path: Option<&Path>` to `run_plan_with_memory`. Same logic as coordinator: load/validate checkpoint, skip completed groups with PhaseRestored events, save after each group, delete on success. Post-run extraction still runs on resumed runs. Updated all 7 existing test call sites with `None`. All 13 existing memory tests pass unchanged.

## Files
- `crates/ath-orchestrator/src/memory.rs` — checkpoint-aware run_plan_with_memory, updated test call sites
