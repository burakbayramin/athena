---
id: S01
parent: M003
milestone: M003
provides:
  - Checkpoint struct with run_id, plan_fingerprint, plan, completed_records, completed_phase_ids, started_at, updated_at
  - plan_fingerprint() — deterministic SHA-256 hash of canonical JSON ExecutionPlan
  - CheckpointStore with atomic save/load/delete (temp file + rename)
  - Checkpoint::should_skip_group() — checks if all phase IDs in a group are completed
  - Checkpoint::is_stale() — detects plan fingerprint mismatch
  - Checkpoint::add_records() — adds records and updates phase_ids + timestamp
requires: []
affects:
  - S02
  - S03
key_files:
  - crates/ath-orchestrator/src/checkpoint.rs
  - crates/ath-orchestrator/Cargo.toml
  - Cargo.toml
key_decisions:
  - "SHA-256 on serde_json::to_string for plan fingerprint — deterministic because serde field order follows declaration order"
  - "Reused PhaseRunnerError::AtomicWriteFailed for checkpoint errors — no new error variant needed"
  - "completed_phase_ids is HashSet<u32> persisted alongside records — O(1) lookup on resume"
patterns_established:
  - "Atomic temp-file + rename for checkpoint writes (matches VikingStore pattern from ath-memory)"
  - "should_skip_group uses vacuous truth for empty groups"
observability_surfaces:
  - ".ath/checkpoint.json is human-readable pretty-printed JSON"
drill_down_paths:
  - .gsd/milestones/M003/slices/S01/tasks/T01-SUMMARY.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S01: Checkpoint Types & Persistence

**Checkpoint primitives with plan fingerprint, atomic persistence, and phase-skip logic — 16 tests pass.**

## What Happened

Single task built the complete checkpoint module in `ath-orchestrator/src/checkpoint.rs`. The `Checkpoint` struct captures full run state: identity (run_id), plan integrity (SHA-256 fingerprint), progress (completed_records + completed_phase_ids HashSet), and timing (started_at, updated_at). `CheckpointStore` provides atomic JSON persistence with temp-file + rename (matching the existing VikingStore pattern). Three decision methods: `should_skip_group` for resume logic, `is_stale` for plan-change detection, `add_records` for progress accumulation.

## Verification

- `cargo test -p ath-orchestrator -- checkpoint` — 16/16 pass
- `cargo check --workspace` — clean (only pre-existing dead_code warnings)
- Fingerprint: deterministic (same plan → same hash), sensitive (different name/order → different hash)
- Persistence: round-trip, overwrite, missing-returns-None, corrupt-returns-Err, creates parent dirs
- Logic: skip fully-completed group, don't skip partial, skip empty (vacuous), stale detection

## Deviations

None.

## Files Created/Modified

- `crates/ath-orchestrator/src/checkpoint.rs` — new: Checkpoint, CheckpointStore, plan_fingerprint, 16 tests
- `crates/ath-orchestrator/src/lib.rs` — added pub mod checkpoint
- `crates/ath-orchestrator/Cargo.toml` — added sha2 dependency
- `Cargo.toml` — added sha2 workspace dependency

## Forward Intelligence

### What the next slice should know
- `Checkpoint::new(run_id, plan)` creates a fresh checkpoint. `add_records` accumulates. `should_skip_group` and `is_stale` are the decision points for resume logic.
- `CheckpointStore` is stateless — all methods take a `&Path`. The coordinator decides where the checkpoint lives (`.ath/checkpoint.json`).
- `plan_fingerprint` uses `serde_json::to_string` which is deterministic for the same struct. No need for canonical JSON sorting — Rust serde follows field declaration order.

### What's fragile
- Plan fingerprint depends on the exact serialization of `ExecutionPlan`. If any field is added/removed/reordered in the struct, existing checkpoints become stale. This is intentional — schema changes should invalidate checkpoints.
