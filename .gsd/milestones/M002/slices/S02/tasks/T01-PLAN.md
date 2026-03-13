---
estimated_steps: 5
estimated_files: 7
---

# T01: Implement observation types, storage, and buffer

**Slice:** S02 — Observation System
**Milestone:** M002

## Description

Build the full observation capture layer inside `ath-memory::observe` — typed events, append-only JSONL writer/reader, and a thread-safe in-memory buffer. This is the data model and I/O foundation that S03's extraction pipeline and S04's orchestrator hooks will consume.

## Steps

1. **Add `ath-types` dependency** to `crates/ath-memory/Cargo.toml`. Verify `cargo check --workspace` to confirm no cycles.

2. **Create `observe/types.rs`** — Define `ObservationType` enum (internally tagged with `#[serde(tag = "type")]`) covering: `AgentRequest` (agent, prompt summary, phase_id, task_name), `AgentResponse` (agent, token usage, phase_id, task_name), `ReviewVerdict` (reviewer agent, passed, severity, reason summary, attempt_number, phase_id), `RetryStarted` (phase_id, attempt_number, feedback summary), `FileOperation` (op kind, path as String, phase_id), `Error` (message, phase_id, severity), `RoutingDecision` (agent, task_name, reason, phase_id). Define `Observation` wrapper struct with `id: Uuid`, `run_id: Uuid`, `timestamp: DateTime<Utc>`, `phase_id: Option<u32>`, and `event: ObservationType`. All paths as `String`, not `PathBuf`.

3. **Create `observe/storage.rs`** — `ObservationWriter`: takes a root path (`.ath/memory/observations/`), creates `<run_id>.jsonl` file with `OpenOptions::create(true).append(true)`, wraps in `BufWriter`. Method `write(&mut self, obs: &Observation) -> Result<(), MemoryError>` serializes to JSON + `\n`. Method `flush(&mut self)` calls inner `BufWriter::flush()`. Creates parent dirs on first write. `ObservationReader`: static method `read_run(root: &Path, run_id: &Uuid) -> Result<Vec<Observation>, MemoryError>` reads JSONL line-by-line, skips empty lines, collects into Vec. Method `read_file(path: &Path)` for arbitrary JSONL path.

4. **Create `observe/buffer.rs`** — `ObservationBuffer` with `inner: Mutex<BufferInner>` where `BufferInner` holds `observations: Vec<Observation>`, `run_id: Uuid`, and `max_cap: usize` (default 500). Method `record(&self, event: ObservationType)` acquires lock (with poison recovery), constructs `Observation` with auto-generated id and current timestamp, evicts oldest if at cap, then pushes. Method `flush(&self, writer: &mut ObservationWriter) -> Result<usize, MemoryError>` drains buffer and writes all to writer, returns count. Method `len(&self) -> usize`. Assert `Send + Sync` at compile time.

5. **Wire module and write tests** — Add `pub mod observe;` to `lib.rs` with re-exports. Add `ObservationWriteError` and `ObservationReadError` variants to `MemoryError` with `hint()` messages. Write tests: serde round-trip for each `ObservationType` variant, `Observation` round-trip, writer creates file and appends multiple observations, reader reads back identically, buffer cap enforcement (insert 501, get 500 with oldest evicted), buffer flush drains to writer, reader handles empty file, reader skips blank lines, `Send + Sync` compile assertion for `ObservationBuffer`.

## Must-Haves

- [ ] `ObservationType` enum with all 7 variants, internally tagged serde
- [ ] `Observation` wrapper with uuid, run_id, timestamp, optional phase_id, event
- [ ] `ObservationWriter` — append-only JSONL, BufWriter, explicit flush, parent dir creation
- [ ] `ObservationReader` — read JSONL by run_id or by path, skip empty lines
- [ ] `ObservationBuffer` — Mutex-based, Send + Sync, record/flush/len, cap with oldest-eviction
- [ ] `MemoryError` extended with observation variants + hints
- [ ] `ath-types` dependency added (no workspace cycle)
- [ ] Comprehensive tests covering round-trip, cap, threading, edge cases

## Verification

- `cargo test -p ath-memory -- observe` — all tests pass
- `cargo check --workspace` — clean compilation, no regressions
- Serde round-trip test for each ObservationType variant
- Buffer cap test: 501 inserts → 500 entries, oldest evicted
- Writer + reader integration: write N observations, read back N identical observations
- Empty file / blank line reader edge cases pass

## Observability Impact

- **New diagnostic surface:** `.ath/memory/observations/<run-uuid>.jsonl` files become inspectable artifacts. Each line is self-describing JSON with `"type"` tag — agents can grep for specific event types.
- **Buffer introspection:** `ObservationBuffer::len()` returns current depth; `flush()` returns count of observations written.
- **Error diagnostics:** Two new `MemoryError` variants (`ObservationWriteError`, `ObservationReadError`) carry `hint()` messages pointing agents to the observations directory.
- **Failure visibility:** Write errors include the file path and underlying I/O error. Read errors include the file path and line-level context. Malformed JSONL lines are skipped (not fatal), preserving partial reads.
- **No secrets:** Observations capture summaries and metadata, never raw prompts or credentials.

## Inputs

- `crates/ath-memory/src/error.rs` — existing `MemoryError` enum to extend
- `crates/ath-memory/src/lib.rs` — module declarations to update
- `crates/ath-types/src/agent.rs` — `AgentKind`, `AgentRequest`, `AgentResponse` types
- `crates/ath-types/src/phase.rs` — `TokenUsage` struct
- `crates/ath-types/src/review.rs` — `Severity` enum
- `crates/ath-orchestrator/src/progress.rs` — `ProgressEvent` pattern reference (don't import, just follow the pattern)
- S02-RESEARCH.md — design decisions, constraints, pitfall avoidance

## Expected Output

- `crates/ath-memory/Cargo.toml` — `ath-types` added to dependencies
- `crates/ath-memory/src/lib.rs` — `pub mod observe;` added with re-exports
- `crates/ath-memory/src/error.rs` — two new variants with hint() messages
- `crates/ath-memory/src/observe/mod.rs` — submodule declarations and re-exports
- `crates/ath-memory/src/observe/types.rs` — `ObservationType` + `Observation`
- `crates/ath-memory/src/observe/storage.rs` — `ObservationWriter` + `ObservationReader`
- `crates/ath-memory/src/observe/buffer.rs` — `ObservationBuffer`
