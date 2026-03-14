---
id: S01
parent: M005
milestone: M005
provides:
  - Streaming API internally used by all agent backends
  - call_provider_streaming with optional on_chunk callback
  - AgentBackend::send_streaming() trait method
  - ChunkCallback type for streaming chunk delivery
  - JSON schema fallback to non-streaming for provider compat
requires: []
affects:
  - S02
key_files:
  - crates/ath-agents/src/actor/mod.rs
  - crates/ath-agents/src/backend.rs
  - crates/ath-agents/src/actor/claude.rs
  - crates/ath-agents/src/actor/gemini.rs
  - crates/ath-agents/src/actor/codex.rs
  - crates/ath-agents/src/actor/generic.rs
key_decisions:
  - "D037: JSON schema requests fall back to non-streaming exec_chat — not all providers support structured output + streaming"
  - "D038: ChunkCallback is Arc<dyn Fn(&str) + Send + Sync> — can be cloned and sent across actor boundaries"
  - "D039: send_streaming() has default impl delegating to send() — MockBackend and existing tests unchanged"
duration: 20m
verification_result: passed
completed_at: 2026-03-14
---

# S01: Internal Streaming Switch + Chunk Callback Infrastructure

**Switched all agent backends from `exec_chat` to `exec_chat_stream` with chunk callback support. 665 tests pass, 0 failures.**

## What Happened

Replaced `call_provider` internals with `exec_chat_stream`, capturing content and usage via `ChatOptions`. Added `call_provider_streaming` with optional `on_chunk: Option<&(dyn Fn(&str) + Send + Sync)>` callback. JSON schema requests fall back to non-streaming. Added `ChunkCallback` type, `ActorMessage.on_chunk` field, and `AgentBackend::send_streaming()` trait method. All 4 handles updated. Added `futures` workspace dependency.

## Verification

- `cargo test --workspace` — 665 passed, 0 failed
- Streaming API used internally, no behavioral change for users yet

## Forward Intelligence

S02 needs to wire the chunk callback from CLI verbose mode through the coordinator/phase runner to the backend calls. The `send_streaming()` method is ready — the phase runner just needs to call it with a callback that emits progress events.
