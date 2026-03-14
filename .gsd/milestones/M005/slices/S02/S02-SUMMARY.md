---
id: S02
parent: M005
milestone: M005
provides:
  - StreamChunk progress event for real-time chunk delivery
  - make_chunk_callback() bridges ProgressObserver to ChunkCallback
  - Phase runner uses send_streaming() for task and review calls
  - VerboseTranscriptSink prints chunks inline (progress bar suspended)
  - print_inline_chunk on TerminalProgressReporter
requires:
  - S01
affects: []
key_files:
  - crates/ath-orchestrator/src/progress.rs
  - crates/ath-orchestrator/src/phase_runner.rs
  - crates/ath-cli/src/progress.rs
  - crates/ath-cli/src/verbose.rs
key_decisions:
  - "D040: StreamChunk events only emitted when observer captures_transcripts (verbose mode)"
  - "D041: Progress bar suspended during chunk writes to prevent interleaving"
duration: 25m
verification_result: passed
completed_at: 2026-03-14
---

# S02: Streaming Chunk Observer

**Wired streaming chunks from backend through orchestrator to CLI verbose output. 669 tests pass, 0 failures.**

## What Happened

Added `ProgressEvent::StreamChunk` with phase_id, phase_name, agent, chunk fields. `make_chunk_callback()` in progress.rs creates a `ChunkCallback` that emits `StreamChunk` events — returns `None` when observer is absent or non-verbose. Phase runner now calls `backend.send_streaming(request, chunk_cb)` for task execution and review dispatch. CLI handles chunks via `VerboseTranscriptSink::on_event` → `print_inline_chunk()`, which suspends the indicatif progress bar spinner before writing to stdout for clean output.

## Verification

- `cargo test --workspace` — 669 passed, 0 failures (+4 new tests)
- Tests cover: callback creation with/without observer, chunk event emission, CLI chunk handling

## Notes

Current task execution and review dispatch use JSON schema, which falls back to non-streaming `exec_chat`. The streaming path will activate automatically when non-schema calls are added (e.g., multi-turn conversations, freeform prompts).
