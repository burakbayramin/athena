---
id: T01
parent: S02
milestone: M002
provides:
  - ObservationType enum with 7 internally-tagged variants
  - Observation wrapper struct with identity, run correlation, and timestamp
  - ObservationWriter — append-only JSONL writer with BufWriter
  - ObservationReader — JSONL reader with empty/blank line handling
  - ObservationBuffer — thread-safe in-memory buffer with cap enforcement
  - MemoryError observation variants with hint() messages
key_files:
  - crates/ath-memory/src/observe/types.rs
  - crates/ath-memory/src/observe/storage.rs
  - crates/ath-memory/src/observe/buffer.rs
  - crates/ath-memory/src/observe/mod.rs
  - crates/ath-memory/src/error.rs
  - crates/ath-memory/src/lib.rs
  - crates/ath-memory/Cargo.toml
key_decisions:
  - Used ath-types AgentKind, Severity, TokenUsage directly rather than parallel observation-specific types
  - phase_id on Observation wrapper is extracted from the event variant for convenience
  - ObservationReader returns empty Vec (not error) for missing files — callers don't need to check existence
  - Mutex poison recovery via unwrap_or_else(|e| e.into_inner()) — observation loss acceptable, run crash not
patterns_established:
  - Internally tagged serde enum (#[serde(tag = "type")]) for JSONL grep-ability
  - File-per-run storage layout (<root>/<run-uuid>.jsonl)
  - Buffer flush returns count for throughput diagnostics
  - MemoryError observation variants follow existing hint() pattern
observability_surfaces:
  - .ath/memory/observations/<run-uuid>.jsonl — grep-able JSONL with type tags
  - ObservationBuffer::len() — current buffer depth
  - ObservationBuffer::flush() return value — count of observations written
  - MemoryError::ObservationWriteError and ObservationReadError with hint() messages
duration: 25m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Implement observation types, storage, and buffer

**Built the full observation capture layer: 7-variant typed enum, append-only JSONL writer/reader, and thread-safe in-memory buffer with cap enforcement.**

## What Happened

Added `ath-types` as a dependency of `ath-memory` (no workspace cycles). Created the `observe` submodule with three files:

- `types.rs` — `ObservationType` enum with 7 internally-tagged variants (AgentRequest, AgentResponse, ReviewVerdict, RetryStarted, FileOperation, Error, RoutingDecision) plus `Observation` wrapper with uuid, run_id, timestamp, phase_id, and event. Also defines `FileOpKind` enum for file operation types. All paths stored as `String`, not `PathBuf`.

- `storage.rs` — `ObservationWriter` wraps `BufWriter<File>` with append-only opens, parent dir creation, explicit flush. `ObservationReader` reads JSONL line-by-line, skips empty/whitespace lines, returns empty Vec for missing files.

- `buffer.rs` — `ObservationBuffer` holds `Mutex<BufferInner>` with poison recovery. `record()` auto-generates id and timestamp, extracts phase_id from event, evicts oldest at cap. `flush()` drains and writes to writer, returns count. Compile-time `Send + Sync` assertion via const block.

Extended `MemoryError` with `ObservationWriteError` and `ObservationReadError` variants, both carrying path + message and following the `hint()` pattern.

## Verification

- `cargo test -p ath-memory -- observe` — **27 tests pass** (all observation module tests)
- `cargo check --workspace` — clean compilation, no regressions, no cycles
- Serde round-trip tested for all 7 ObservationType variants
- Buffer cap: 501 inserts → 500 entries, oldest evicted (verified by checking phase_ids after flush)
- Writer + reader integration: write N observations, read back N identical observations (verified with `assert_eq!`)
- Empty file: reader returns empty Vec
- Blank lines: reader skips whitespace-only lines, returns only valid observations
- Missing file: reader returns empty Vec (not error)
- Send + Sync: compile-time assertion passes, plus explicit test
- All 7 event types written and read back through buffer→writer→reader pipeline
- `cargo test --workspace` — 497 tests pass, zero failures

### Slice-level verification status

| Check | Status |
|-------|--------|
| `cargo test -p ath-memory -- observe` — all tests pass | ✅ |
| `cargo check --workspace` — no regressions, no cycles | ✅ |
| Round-trip: serialize → JSONL → deserialize identical | ✅ |
| Buffer cap: 501st evicts oldest | ✅ |
| Thread safety: Send + Sync compile-time check | ✅ |
| Writer creates parent dirs, flushes explicitly, uses `\n` | ✅ |
| Reader handles empty file and single-line file | ✅ |
| MemoryError observation variants produce actionable hints | ✅ |

All slice-level verification checks pass.

## Diagnostics

- **Inspect observations:** List `.ath/memory/observations/` dir to see per-run JSONL files. Each line is self-describing JSON with `"type":"VariantName"` — `grep '"type":"Error"' <file>` to find errors.
- **Error hints:** `MemoryError::ObservationWriteError` hints to check directory permissions. `ObservationReadError` hints to inspect the JSONL file for corruption.
- **Buffer state:** Call `len()` for current depth, `flush()` return value for write count.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/Cargo.toml` — Added `ath-types` dependency
- `crates/ath-memory/src/lib.rs` — Added `pub mod observe;` and re-exports
- `crates/ath-memory/src/error.rs` — Added ObservationWriteError and ObservationReadError variants with hints
- `crates/ath-memory/src/observe/mod.rs` — Submodule declarations and re-exports
- `crates/ath-memory/src/observe/types.rs` — ObservationType enum (7 variants) + Observation wrapper + FileOpKind
- `crates/ath-memory/src/observe/storage.rs` — ObservationWriter + ObservationReader
- `crates/ath-memory/src/observe/buffer.rs` — ObservationBuffer with Mutex, cap enforcement, flush
