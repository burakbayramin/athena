# S02: Observation System

**Goal:** `ath-memory` has a typed observation capture layer — event types, an in-memory buffer, an append-only JSONL writer, and a reader for serialized observations.
**Demo:** Unit tests prove observation types serialize/deserialize, buffer collects and flushes events to JSONL files, reader hydrates them back, and the 500-entry cap evicts oldest entries.

## Must-Haves

- `ObservationType` enum covering agent requests/responses, review verdicts/retries, file operations, errors, and routing decisions
- `Observation` wrapper struct with timestamp, run ID, phase ID, and event payload
- `ObservationWriter` — append-only JSONL writer (one file per run, BufWriter, explicit flush)
- `ObservationReader` — reads JSONL back into `Vec<Observation>` for a given run or path
- `ObservationBuffer` — thread-safe (`Send + Sync`) in-memory buffer with `record()` and `flush()`, configurable cap (default 500), oldest-eviction on overflow
- `MemoryError` extended with observation-specific variants (`ObservationWriteError`, `ObservationReadError`) following `hint()` pattern
- `ath-types` added as dependency of `ath-memory` (for `AgentKind`, `TokenUsage`, `Severity`)
- All file paths stored as forward-slash-normalized `String` (not `PathBuf`)
- Internally tagged serde enum (`#[serde(tag = "type")]`) for JSONL grep-ability
- Mutex poisoning recovery in buffer (`unwrap_or_else(|e| e.into_inner())`)

## Proof Level

- This slice proves: contract (data model + I/O round-trip)
- Real runtime required: no
- Human/UAT required: no

## Verification

- `cargo test -p ath-memory -- observe` — all observation module tests pass
- `cargo check --workspace` — no regressions, no cycles from new `ath-types` dep
- Round-trip: serialize → JSONL → deserialize produces identical observations
- Buffer cap: 501st observation evicts the oldest
- Thread safety: buffer is `Send + Sync` (compile-time check)
- Writer creates parent dirs, flushes explicitly, uses `\n` line terminator
- Reader handles empty file and single-line file
- `MemoryError` observation variants produce actionable `hint()` strings (verified in tests)

## Observability / Diagnostics

- **JSONL file per run** — Each run produces `.ath/memory/observations/<run-uuid>.jsonl`. An agent can list files in this dir to see all observed runs, and `cat` any file to inspect raw events. The internally-tagged `"type"` field makes events `grep`-able (e.g., `grep '"type":"Error"' <file>`).
- **Buffer length** — `ObservationBuffer::len()` exposes current buffer depth. Useful for health checks and progress diagnostics.
- **Flush count** — `ObservationBuffer::flush()` returns the number of observations written, providing a concrete signal of capture throughput.
- **Error hints** — `MemoryError::ObservationWriteError` and `ObservationReadError` carry `hint()` messages directing the agent to inspect the observations directory or the JSONL file.
- **No secrets in observations** — Observation types capture prompt summaries (not full prompts), agent kinds, and structural metadata. No API keys, tokens, or credentials flow through this layer.
- **Failure visibility** — Serialization failures on individual observations surface as `MemoryError` with the failing observation's context. Reader skips blank/malformed lines rather than failing the entire read, logging the skip via the error path.

## Tasks

- [x] **T01: Implement observation types, storage, and buffer** `est:1h30m`
  - Why: The entire observation capture layer is tightly coupled (buffer needs types + writer, writer needs types, reader needs types). Building in one pass avoids redundant context loading and produces a coherent, testable module.
  - Files: `crates/ath-memory/Cargo.toml`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/src/observe/mod.rs`, `crates/ath-memory/src/observe/types.rs`, `crates/ath-memory/src/observe/storage.rs`, `crates/ath-memory/src/observe/buffer.rs`, `crates/ath-memory/src/error.rs`
  - Do: Add `ath-types` dep to ath-memory. Create `observe` submodule with types (ObservationType enum, Observation struct), storage (ObservationWriter with BufWriter + append, ObservationReader for JSONL deserialization), and buffer (ObservationBuffer with Mutex, cap enforcement, flush-to-writer). Extend MemoryError with observation variants. Use `AgentKind`/`TokenUsage`/`Severity` from ath-types directly. Use `#[serde(tag = "type")]` on the enum. Store paths as `String`. Handle mutex poisoning. Write comprehensive tests covering serde round-trip, buffer cap/eviction, writer flush + parent dir creation, reader edge cases (empty, malformed lines), and `Send + Sync` compile-time assertion.
  - Verify: `cargo test -p ath-memory -- observe` passes, `cargo check --workspace` clean
  - Done when: All observation module tests pass, workspace compiles clean, buffer is `Send + Sync`, JSONL round-trip is proven

## Files Likely Touched

- `crates/ath-memory/Cargo.toml`
- `crates/ath-memory/src/lib.rs`
- `crates/ath-memory/src/error.rs`
- `crates/ath-memory/src/observe/mod.rs`
- `crates/ath-memory/src/observe/types.rs`
- `crates/ath-memory/src/observe/storage.rs`
- `crates/ath-memory/src/observe/buffer.rs`
