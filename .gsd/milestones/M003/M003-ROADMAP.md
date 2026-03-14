# M003: Resumable Execution

**Vision:** Failed runs resume from where they left off — no wasted LLM calls, no re-doing completed work, clear visibility into what was restored vs. executed fresh.

## Success Criteria

- A run that fails at phase N can be resumed by re-running `ath run` — phases before N are skipped with "Restored from checkpoint" output
- `ath run --fresh` ignores any existing checkpoint and starts clean
- `ath run --status` shows checkpoint state (completed phases, plan fingerprint) without executing
- Checkpoint is automatically cleaned up after successful completion
- Stale checkpoints (plan changed) are detected and the user is warned
- Existing test suite still passes (no regressions)

## Key Risks / Unknowns

- **Memory-aware execution path duplication** — `memory.rs` duplicates the coordinator loop, so checkpoint logic must work in both paths without further duplication
- **Plan fingerprint reliability** — need deterministic hashing that catches meaningful changes but doesn't false-positive on serialization order differences

## Proof Strategy

- Memory-aware duplication → retire in S02 by proving checkpoint works through the memory-aware path with an integration test
- Plan fingerprint → retire in S01 by proving same plan produces same fingerprint and different plans produce different fingerprints

## Verification Classes

- Contract verification: unit tests for checkpoint CRUD, plan fingerprinting, phase skip logic
- Integration verification: two-invocation test where first run fails, second resumes and completes
- Operational verification: `--fresh` clears checkpoint, `--status` shows state, successful run cleans up
- UAT / human verification: terminal output clearly distinguishes restored vs. fresh phases

## Milestone Definition of Done

This milestone is complete only when all are true:

- All slice deliverables are complete and tests pass
- Coordinator and memory-aware paths both support checkpoint save/restore
- CLI `--fresh` and `--status` flags work correctly
- A two-invocation integration test proves resume from checkpoint
- Stale checkpoint detection works (plan fingerprint mismatch)
- Existing test suite still passes (no regressions)

## Requirement Coverage

- Covers: REQ-RESUME (resumable execution from last completed phase)
- Partially covers: none
- Leaves for later: mid-phase resume (within retry cycles), checkpoint migration across versions
- Orphan risks: none

## Slices

- [x] **S01: Checkpoint Types & Persistence** `risk:high` `depends:[]`
  > After this: `Checkpoint` type with plan fingerprint, atomic JSON persistence, and phase-skip logic exist in `ath-orchestrator` — proven by unit tests for write/read/fingerprint/skip

- [x] **S02: Coordinator Integration** `risk:high` `depends:[S01]`
  > After this: `run_plan_with_progress` and `run_plan_with_memory` save checkpoints after each group and skip completed groups on resume — proven by integration tests with mock backends where a failed run resumes on second invocation

- [x] **S03: CLI & UX** `risk:low` `depends:[S01,S02]`
  > After this: `ath run` detects checkpoints and resumes, `--fresh` forces clean start, `--status` shows checkpoint state, terminal output distinguishes restored vs. fresh phases — proven by CLI integration tests

## Boundary Map

### S01 → S02

Produces:
- `Checkpoint` struct with `run_id`, `plan_fingerprint`, `plan`, `completed_records`, `completed_phase_ids`, `started_at`, `updated_at`
- `CheckpointStore` with `save(path, checkpoint)`, `load(path) -> Option<Checkpoint>`, `delete(path)` — all atomic writes
- `plan_fingerprint(plan: &ExecutionPlan) -> String` — deterministic hash
- `should_skip_group(checkpoint, group) -> bool` — checks if all phase IDs in a group have completed records

Consumes:
- nothing (first slice)

### S01 + S02 → S03

Produces:
- Modified `run_plan_with_progress` that accepts optional checkpoint path, saves after each group, restores on resume
- Modified `run_plan_with_memory` with same checkpoint support
- `ProgressEvent::PhaseRestored` variant for restored-from-checkpoint phases
- Checkpoint cleanup on successful plan completion

Consumes:
- `Checkpoint`, `CheckpointStore`, `plan_fingerprint`, `should_skip_group` from S01
