# S02: Observation System — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is a pure data model + I/O layer with no runtime integration. All contracts are fully testable via unit tests and file inspection — no running server or orchestrator needed.

## Preconditions

- Rust toolchain installed (`cargo` available)
- Working directory is the athena project root
- `ath-types` crate exists in workspace (dependency)

## Smoke Test

Run `cargo test -p ath-memory -- observe` — all 27 tests should pass with zero failures.

## Test Cases

### 1. Serde round-trip for all observation types

1. Run `cargo test -p ath-memory -- observe::types`
2. **Expected:** All 7 round-trip tests pass — each ObservationType variant serializes to JSON and deserializes back to an identical value. The `"type"` field appears in the JSON output with the variant name (e.g., `"type":"AgentRequest"`).

### 2. Buffer cap enforcement and oldest-eviction

1. Run `cargo test -p ath-memory -- cap`
2. **Expected:** `cap_501_yields_500` passes — after inserting 501 observations into a buffer with cap=500, `len()` returns 500 and the oldest entry (first inserted) is gone. `cap_enforcement_evicts_oldest` passes — flushed output contains the 500 most recent entries in insertion order.

### 3. Writer creates parent directories and appends

1. Run `cargo test -p ath-memory -- writer_creates`
2. **Expected:** `writer_creates_parent_dirs` passes — writing to a path with non-existent parent directories succeeds; directories are created automatically. `writer_creates_file_and_appends` passes — file is created on first write, subsequent writes append (not overwrite).

### 4. Reader handles edge cases

1. Run `cargo test -p ath-memory -- reader`
2. **Expected:** `reader_returns_empty_for_missing_file` — returns `Ok(vec![])` for a non-existent file. `reader_handles_empty_file` — returns `Ok(vec![])` for a zero-byte file. `reader_skips_blank_lines` — whitespace-only lines between valid JSON lines are ignored, only valid observations returned.

### 5. Buffer thread safety

1. Run `cargo test -p ath-memory -- send_sync`
2. **Expected:** Compile-time assertion passes — `ObservationBuffer` implements both `Send` and `Sync`. This is enforced by a const block that would fail compilation if the traits weren't satisfied.

### 6. Full pipeline round-trip (buffer → writer → reader)

1. Run `cargo test -p ath-memory -- flush_all_event_types`
2. **Expected:** All 7 observation types are recorded into a buffer, flushed to a JSONL file via the writer, read back via the reader, and the deserialized observations match the originals exactly. Flush returns count=7.

### 7. Workspace compilation check

1. Run `cargo check --workspace`
2. **Expected:** Clean compilation with no errors. Dead-code warnings on observation module functions are expected (not yet called from integration code). No dependency cycles introduced by the `ath-types` dependency.

## Edge Cases

### Buffer flush on empty buffer

1. Create an `ObservationBuffer` with default cap, don't record anything.
2. Call `flush()`.
3. **Expected:** Returns `Ok(0)` — no observations written, no error, no file created.

### Reader with single-line JSONL file

1. Write exactly one observation to a JSONL file.
2. Read it back with `ObservationReader`.
3. **Expected:** Returns `Ok(vec![obs])` with exactly one observation matching the written data.

### Observation without phase_id

1. Record a `RoutingDecision` event (which has no phase context).
2. Flush and read back.
3. **Expected:** `phase_id` on the `Observation` wrapper is `None`. Serialized JSON omits the `phase_id` field or sets it to `null`.

### Mutex poison recovery

1. The buffer uses `unwrap_or_else(|e| e.into_inner())` on mutex lock.
2. **Expected:** If a thread panics while holding the lock, subsequent `record()` and `flush()` calls recover the inner data and continue operating rather than propagating the panic.

## Failure Signals

- Any test in `cargo test -p ath-memory -- observe` fails → regression in observation module
- `cargo check --workspace` shows errors (not warnings) → dependency cycle or type mismatch introduced
- JSONL files missing the `"type"` field → serde tag configuration broken, downstream grep-ability lost
- `flush()` returns a count that doesn't match the number of recorded observations → buffer drain logic broken
- Reader returns error instead of empty Vec for missing file → contract violation that would break S03 extraction

## Requirements Proved By This UAT

- None — no REQUIREMENTS.md tracked for this project's M002 slice-level requirements

## Not Proven By This UAT

- Runtime integration with orchestrator (S04 scope)
- Observation-to-memory extraction quality (S03 scope)
- CLI inspection of observation files (S05 scope)
- Performance under high observation throughput (not required at current scale)

## Notes for Tester

- The 3 dead-code warnings on `cargo check --workspace` are expected — these functions are public API for downstream slices (S03/S04/S05) but not yet called from any integration code.
- All tests use `tempdir` for file I/O — no cleanup needed, no persistent state left behind.
- The `Send + Sync` assertion is a compile-time check — if the test binary compiles, the assertion has already passed.
