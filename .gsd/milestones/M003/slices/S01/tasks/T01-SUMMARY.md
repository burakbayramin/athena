---
id: T01
result: passed
---

# T01: Checkpoint type, fingerprint, and persistence

Built `checkpoint.rs` in ath-orchestrator with all checkpoint primitives. `Checkpoint` struct holds run_id, plan_fingerprint, plan, completed_records, completed_phase_ids, started_at, updated_at. `plan_fingerprint()` uses SHA-256 on canonical JSON. `CheckpointStore` provides atomic save/load/delete with temp-file + rename. `should_skip_group()` and `is_stale()` cover resume decision logic.

16 tests pass: fingerprint determinism/sensitivity (3), checkpoint logic (5), persistence (5), edge cases (3).

## Files
- `crates/ath-orchestrator/src/checkpoint.rs` — new: all types, logic, 16 tests
- `crates/ath-orchestrator/src/lib.rs` — added `pub mod checkpoint`
- `crates/ath-orchestrator/Cargo.toml` — added sha2 dependency
- `Cargo.toml` — added sha2 workspace dependency
