---
id: S02
parent: M002
milestone: M002
provides:
  - ObservationType enum with 7 internally-tagged variants (AgentRequest, AgentResponse, ReviewVerdict, RetryStarted, FileOperation, Error, RoutingDecision)
  - Observation wrapper struct with uuid, run_id, timestamp, phase_id, and typed event payload
  - ObservationWriter — append-only JSONL writer with BufWriter and explicit flush
  - ObservationReader — JSONL reader with empty/blank/missing file handling
  - ObservationBuffer — thread-safe (Send + Sync) in-memory buffer with configurable cap (default 500) and oldest-eviction
  - MemoryError observation variants (ObservationWriteError, ObservationReadError) with hint() messages
  - FileOpKind enum for file operation classification
requires:
  - slice: none
    provides: independent slice
affects:
  - S03
  - S04
  - S05
key_files:
  - crates/ath-memory/src/observe/types.rs
  - crates/ath-memory/src/observe/storage.rs
  - crates/ath-memory/src/observe/buffer.rs
  - crates/ath-memory/src/observe/mod.rs
  - crates/ath-memory/src/error.rs
  - crates/ath-memory/src/lib.rs
  - crates/ath-memory/Cargo.toml
key_decisions:
  - "D008: File-per-run JSONL layout (<run-uuid>.jsonl) — maps naturally to S03 extraction which reads per-run"
  - "D009: ath-memory depends on ath-types for AgentKind, TokenUsage, Severity — one-way dep, no cycle risk"
  - "Mutex poison recovery via unwrap_or_else(|e| e.into_inner()) — observation loss acceptable, run crash not"
  - "ObservationReader returns empty Vec for missing files — callers don't need existence checks"
  - "phase_id extracted from event variant onto Observation wrapper for convenience"
patterns_established:
  - "Internally tagged serde enum (#[serde(tag = \"type\")]) for JSONL grep-ability"
  - "File-per-run storage layout (observations/<run-uuid>.jsonl)"
  - "Buffer flush returns count for throughput diagnostics"
  - "MemoryError observation variants follow existing hint() pattern"
observability_surfaces:
  - ".ath/memory/observations/<run-uuid>.jsonl — grep-able JSONL with type tags"
  - "ObservationBuffer::len() — current buffer depth"
  - "ObservationBuffer::flush() return value — count of observations written"
  - "MemoryError::ObservationWriteError and ObservationReadError with hint() messages"
drill_down_paths:
  - .gsd/milestones/M002/slices/S02/tasks/T01-SUMMARY.md
duration: 25m
verification_result: passed
completed_at: 2026-03-14
---

# S02: Observation System

**Typed observation capture layer with append-only JSONL persistence, thread-safe buffering, and cap-enforced eviction.**

## What Happened

Built the complete observation subsystem in a single task. The `observe` submodule in `ath-memory` provides three layers:

**Types** (`types.rs`) — `ObservationType` enum with 7 internally-tagged variants covering the full agent interaction surface: AgentRequest, AgentResponse, ReviewVerdict, RetryStarted, FileOperation, Error, RoutingDecision. The `Observation` wrapper adds identity (uuid), run correlation (run_id), timestamp, and phase_id extracted from the event payload. Uses `AgentKind`, `TokenUsage`, and `Severity` from `ath-types` directly — no parallel type definitions.

**Storage** (`storage.rs`) — `ObservationWriter` wraps `BufWriter<File>` with append-only opens, auto-creates parent directories, and uses explicit `flush()`. `ObservationReader` deserializes JSONL line-by-line, skips blank lines, returns empty Vec for missing files.

**Buffer** (`buffer.rs`) — `ObservationBuffer` holds `Mutex<BufferInner>` with poison recovery. `record()` auto-generates id/timestamp and extracts phase_id. Cap enforcement evicts oldest entries on overflow (default 500). `flush()` drains buffer through writer and returns count. Compile-time `Send + Sync` assertion ensures thread safety.

Extended `MemoryError` with `ObservationWriteError` and `ObservationReadError` variants following the established `hint()` pattern.

## Verification

- `cargo test -p ath-memory -- observe` — **27 tests pass**
- `cargo check --workspace` — clean (3 dead_code warnings, expected for unused lib functions)
- Serde round-trip verified for all 7 ObservationType variants
- Buffer cap: 501 inserts → 500 entries, oldest evicted (verified by phase_id check)
- Writer + reader integration: write N, read back N identical observations
- Empty file: reader returns empty Vec
- Blank lines: reader skips whitespace-only lines
- Missing file: reader returns empty Vec (not error)
- Send + Sync: compile-time assertion + explicit test
- Full pipeline: buffer → writer → reader round-trip for all 7 event types
- `cargo test --workspace` — 497 tests pass, zero failures

## Deviations

None.

## Known Limitations

- No observation pruning/GC yet — files grow until S05 implements `ath memory gc`
- No streaming reader — entire JSONL file loaded into memory. Acceptable at current scale but may need iterator-based approach if observation files grow large.

## Follow-ups

None — downstream slices S03/S04/S05 consume this as designed.

## Files Created/Modified

- `crates/ath-memory/Cargo.toml` — Added `ath-types` dependency
- `crates/ath-memory/src/lib.rs` — Added `pub mod observe;` and re-exports
- `crates/ath-memory/src/error.rs` — Added ObservationWriteError and ObservationReadError variants with hints
- `crates/ath-memory/src/observe/mod.rs` — Submodule declarations and re-exports
- `crates/ath-memory/src/observe/types.rs` — ObservationType enum (7 variants), Observation wrapper, FileOpKind
- `crates/ath-memory/src/observe/storage.rs` — ObservationWriter + ObservationReader
- `crates/ath-memory/src/observe/buffer.rs` — ObservationBuffer with Mutex, cap enforcement, flush

## Forward Intelligence

### What the next slice should know
- `ObservationReader::read_run(root, run_id)` is the entry point for S03's extraction pipeline — it returns `Vec<Observation>` for a given run UUID.
- `ObservationType` variants are internally tagged with `#[serde(tag = "type")]` — the `"type"` field value matches the variant name exactly (e.g., `"AgentResponse"`, `"Error"`).
- `phase_id` on `Observation` is `Option<String>` — not all events have a phase context (e.g., top-level routing decisions).

### What's fragile
- `ObservationWriter` opens the file in append mode on construction — if the file is deleted mid-run, subsequent writes will fail with `ObservationWriteError`. The buffer's flush catches this, but observations are lost.

### Authoritative diagnostics
- `.ath/memory/observations/<run-uuid>.jsonl` — each line is self-describing JSON. `grep '"type":"Error"'` finds error events. This is the ground truth for what was captured.
- `ObservationBuffer::len()` — if this stays at cap (500) between flushes, the buffer is saturated and oldest events are being evicted.

### What assumptions changed
- No assumptions changed — implementation matched the plan exactly.
