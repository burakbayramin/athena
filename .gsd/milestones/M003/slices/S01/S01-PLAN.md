# S01: Checkpoint Types & Persistence

**Goal:** `Checkpoint` type with plan fingerprint, atomic JSON persistence, and phase-skip logic exist in `ath-orchestrator`
**Demo:** Unit tests prove checkpoint write/read round-trip, plan fingerprinting is deterministic and collision-free, and phase-group skip logic correctly identifies completed vs. pending groups

## Must-Haves

- `Checkpoint` struct with run_id, plan_fingerprint, plan, completed_records, completed_phase_ids, started_at, updated_at
- `plan_fingerprint(plan: &ExecutionPlan) -> String` — deterministic SHA-256 hash of canonical JSON
- `CheckpointStore` with `save`, `load`, `delete` — atomic temp-file + rename writes
- `should_skip_group(checkpoint, group: &[u32]) -> bool` — all phase IDs in group have records
- Stale checkpoint detection via fingerprint comparison

## Proof Level

- This slice proves: contract
- Real runtime required: no
- Human/UAT required: no

## Verification

- `cargo test -p ath-orchestrator -- checkpoint` — all checkpoint unit tests pass
- `cargo check --workspace` — clean, no regressions
- `cargo test --workspace` — all tests pass

## Observability / Diagnostics

- Runtime signals: none (types-only slice)
- Inspection surfaces: `.ath/checkpoint.json` is human-readable JSON
- Failure visibility: `CheckpointError` variants with actionable messages
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `ExecutionPlan` and `PhaseRecord` from `ath-types`
- New wiring introduced in this slice: none — types only, wired in S02
- What remains before the milestone is truly usable end-to-end: S02 (coordinator integration), S03 (CLI)

## Tasks

- [x] **T01: Checkpoint type, fingerprint, and persistence** `est:30m`
  - Why: All checkpoint primitives in one task — the type, fingerprint function, store, and skip logic are tightly coupled and small enough for a single unit
  - Files: `crates/ath-orchestrator/src/checkpoint.rs`, `crates/ath-orchestrator/src/lib.rs`, `crates/ath-orchestrator/Cargo.toml`
  - Do:
    1. Add `sha2` workspace dependency to root Cargo.toml and ath-orchestrator
    2. Create `checkpoint.rs` in ath-orchestrator with:
       - `Checkpoint` struct: `run_id: String`, `plan_fingerprint: String`, `plan: ExecutionPlan`, `completed_records: Vec<PhaseRecord>`, `completed_phase_ids: HashSet<u32>`, `started_at: DateTime<Utc>`, `updated_at: DateTime<Utc>` — all Serialize/Deserialize
       - `plan_fingerprint(plan: &ExecutionPlan) -> String`: serialize plan to canonical JSON via `serde_json::to_string`, SHA-256 hash, hex-encode
       - `CheckpointStore::save(path, checkpoint)`: atomic write via temp file + rename (follow VikingStore pattern from ath-memory)
       - `CheckpointStore::load(path) -> Result<Option<Checkpoint>>`: return None for missing file, Err for corrupt
       - `CheckpointStore::delete(path)`: remove file, ignore if missing
       - `Checkpoint::add_records(&mut self, records: Vec<PhaseRecord>)`: adds records and updates completed_phase_ids + updated_at
       - `Checkpoint::should_skip_group(&self, group: &[u32]) -> bool`: true iff all phase IDs in group are in completed_phase_ids
       - `Checkpoint::is_stale(&self, plan: &ExecutionPlan) -> bool`: fingerprint mismatch
    3. Add `pub mod checkpoint;` to lib.rs
    4. Write tests:
       - Checkpoint round-trip (save + load)
       - Fingerprint determinism (same plan → same hash)
       - Fingerprint sensitivity (different plan → different hash)
       - should_skip_group: fully completed group → true
       - should_skip_group: partially completed group → false
       - should_skip_group: empty group → true (vacuous truth)
       - is_stale: same plan → false, different plan → true
       - add_records updates completed_phase_ids and updated_at
       - Load missing file → None
       - Delete missing file → Ok
       - Corrupt file → Err
  - Verify: `cargo test -p ath-orchestrator -- checkpoint` + `cargo check --workspace`
  - Done when: all checkpoint tests pass, workspace compiles clean

## Files Likely Touched

- `Cargo.toml` — add sha2 workspace dependency
- `crates/ath-orchestrator/Cargo.toml` — add sha2 dependency
- `crates/ath-orchestrator/src/checkpoint.rs` — new: all checkpoint types and logic
- `crates/ath-orchestrator/src/lib.rs` — add pub mod checkpoint
