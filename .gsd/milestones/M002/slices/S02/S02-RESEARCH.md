# S02: Observation System — Research

**Date:** 2026-03-14

## Summary

S02 adds an observation capture layer to `ath-memory`: typed observation events, an in-memory buffer, an append-only JSONL writer, and a reader that can hydrate serialized observations for downstream consumers (S03's extraction pipeline). This slice is independent — no dependencies on S01's store or index.

The core model is straightforward: an `ObservationType` enum captures the events that matter for post-run extraction (agent requests/responses, review verdicts/retries, file operations, errors, routing decisions), each wrapped in an `Observation` struct with timestamp, run ID, and the event payload. An `ObservationBuffer` collects events during a run and flushes them to JSONL. An `ObservationStorage` reads them back.

The existing orchestrator already emits structured `ProgressEvent`s through a `SharedProgressObserver` trait — the observation system follows the same pattern but targets persistent storage rather than CLI rendering. In S04, the orchestrator will produce observations alongside progress events; S02 just builds the types, writer, and reader without touching the orchestrator.

Key design choices to resolve: where to put the types (in `ath-memory` per spec, since they're memory-subsystem internal), JSONL rotation strategy (file-per-run is simpler than time-based rotation and maps cleanly to extraction), and how much data to capture per event (lean toward structured fields rather than raw content blobs — the extractor can always read files).

## Recommendation

Build as three source files inside `ath-memory::observe`:

1. **`types.rs`** — `ObservationType` enum + `Observation` wrapper struct. Use existing `ath-types` types directly (`AgentKind`, `Severity`) rather than reinventing parallel identifiers. The spec's `AgentId`/`TaskId`/`PhaseId` aliases map to `AgentKind`, `String`, and `u32` respectively.

2. **`storage.rs`** — `ObservationWriter` (append-only JSONL writer with one file per run, BufWriter for batched I/O) and `ObservationReader` (read back JSONL for a given run or from a path). Storage path: `.ath/memory/observations/<run-uuid>.jsonl`.

3. **`buffer.rs`** — `ObservationBuffer` holding an in-memory `Vec<Observation>` behind a `Mutex` for thread safety (observations arrive from parallel phase execution). Provides `record()` to push and `flush()` to drain the buffer and write via `ObservationWriter`. Implements a configurable cap (`max_observations_per_run` from spec, default 500) — oldest events are evicted when the cap is hit.

Don't build observation hooks into the orchestrator (that's S04). Don't build extraction logic (that's S03). This slice proves the data model and I/O layer in isolation.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Timestamps | `chrono::DateTime<Utc>` | Already a workspace dep, used in `LayeredContent` |
| UUIDs for run IDs | `uuid::Uuid` | Already a workspace dep, used throughout |
| Agent identification | `ath_types::AgentKind` | Exact type already used in orchestrator — avoids parallel `AgentId` |
| Serialization | `serde` + `serde_json` | Already workspace deps, JSONL is newline-delimited JSON |
| Error types | `MemoryError` + `thiserror` | Extend existing error enum with observation variants |
| File I/O buffering | `std::io::BufWriter` | Standard library, no dep needed |

## Existing Code and Patterns

- `crates/ath-memory/src/error.rs` — `MemoryError` enum with `hint()` pattern. Add new variants for observation I/O (e.g., `ObservationWriteError`, `ObservationReadError`) following this pattern.
- `crates/ath-memory/src/store.rs` — Temp-file + rename for atomic writes. Observations are append-only, so we don't need atomic write — just `OpenOptions::append(true)` — but should still create parent dirs.
- `crates/ath-memory/src/types.rs` — `LayeredContent` with `Serialize/Deserialize` round-trip tests. Follow this test pattern for `Observation`.
- `crates/ath-orchestrator/src/progress.rs` — `ProgressEvent` enum mirrors the observation pattern. Many events overlap (TaskStarted, ReviewStarted, etc.). Observations are richer (include token counts, files, prompts) and persistent, while progress events are lightweight and transient.
- `crates/ath-types/src/agent.rs` — `AgentKind`, `AgentRequest`, `AgentResponse` types. Use `AgentKind` directly in observation types rather than creating a separate `AgentId` alias.
- `crates/ath-types/src/review.rs` — `ReviewVerdict`, `Severity`. Use `Severity` directly in observation types where needed.
- `crates/ath-types/src/phase.rs` — `TokenUsage` struct. Reuse for token tracking in agent response observations.

## Constraints

- **Thread safety required** — `ObservationBuffer` receives events from parallel phase execution (JoinSet fan-out in coordinator.rs). Must be `Send + Sync`. Use `std::sync::Mutex<Vec<Observation>>` — contention is low (observations are brief pushes, not held across awaits).
- **`ath-memory` currently doesn't depend on `ath-types`** — Adding this dependency is necessary for using `AgentKind`, `TokenUsage`, etc. This is a one-way dep (memory depends on types), no cycle risk since types has no deps on memory or orchestrator.
- **Append-only JSONL** — No random access, no updates, no deletes during a run. Only the writer appends; reader is post-run. This simplifies concurrency (no read-write races during a run).
- **File-per-run, not rolling files** — The spec shows `obs_<timestamp>.jsonl` + `current.jsonl`, but file-per-run (`<run-uuid>.jsonl`) maps more naturally to the extraction pipeline (S03 reads "all observations for this run"). Simpler, and the extraction pipeline won't need to correlate across file boundaries.
- **Windows path compatibility** — `PathBuf` in observation data must serialize as forward-slash paths for cross-platform JSONL portability, or use string paths. Follow the convention from `FileOutput` in phase_runner.rs which stores paths as `String`.
- **Max 500 observations per run** — Config default from spec. Enforced in buffer, not in writer (writer is dumb append).

## Common Pitfalls

- **Serializing `PathBuf` across platforms** — `PathBuf` serializes with backslashes on Windows. Store file paths as `String` (forward-slash normalized) in observation types, matching the `FileOutput.path: String` pattern in the orchestrator.
- **BufWriter not flushed on drop** — If the process crashes, buffered data is lost. Call `flush()` explicitly in `ObservationBuffer::flush()` and document that observations may be lost on crash (acceptable — they're supplementary data, not critical state).
- **Serde `#[serde(tag = "type")]` for enum** — Use internally tagged representation so JSONL lines include `"type":"AgentResponse"` for readability and grep-ability. The orchestrator's `PhaseStatus` uses this pattern.
- **Mutex poisoning** — If a thread panics while holding the observation buffer lock, subsequent `record()` calls will fail. Use `mutex.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned locks — observation loss is acceptable, but panicking the whole run is not.
- **JSONL line terminator** — Write `\n` after each JSON object. Don't use `\r\n` even on Windows — JSONL convention is `\n`.

## Open Risks

- **Observation data volume** — A complex run with many phases and retries could produce hundreds of observations. The 500-entry cap prevents memory bloat, but each observation could be large if it includes review feedback strings or error messages. Consider whether to truncate long string fields (e.g., cap `feedback` at 2000 chars).
- **Run ID availability** — The observation buffer needs a run UUID. Currently, runs don't have a persistent UUID — `PhaseRecord` has one but it's per-phase, not per-run. Need to decide whether the run UUID is generated by the caller when creating the buffer, or derived from the plan/coordinator. Likely: caller provides it when constructing `ObservationBuffer`.
- **ath-types dependency** — Adding `ath-types` as a dep of `ath-memory` is new. Need to verify no workspace cycle issues. Quick check: `ath-types` has zero internal deps, so this is safe.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust (general) | `majiayu000/claude-arsenal@rust-project` | available (17 installs) — generic Rust skill, not specific enough to warrant install |
| JSONL / serde | none found | no relevant skill |

No skills warrant installation — this is standard Rust serde + file I/O work.

## Sources

- Observation system specification: `docs/specs/memory-layer.md` §4.3
- ProgressEvent pattern: `crates/ath-orchestrator/src/progress.rs`
- Agent types: `crates/ath-types/src/agent.rs`
- S01 summary and forward intelligence: `.gsd/milestones/M002/slices/S01/S01-SUMMARY.md`
