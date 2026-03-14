---
id: M005
title: "Streaming Output"
status: complete
slices_completed: 2
slices_total: 2
tests_before: 665
tests_after: 669
started: 2026-03-14
completed: 2026-03-14
---

# M005: Streaming Output — Summary

Added streaming output infrastructure end-to-end, from genai's `exec_chat_stream` API through the actor layer, orchestrator progress system, and CLI verbose rendering.

## Architecture

```
genai exec_chat_stream
  → call_provider_streaming (on_chunk callback)
    → ActorMessage.on_chunk
      → AgentBackend::send_streaming()
        → make_chunk_callback (observer → ChunkCallback bridge)
          → ProgressEvent::StreamChunk
            → VerboseTranscriptSink.print_inline_chunk()
```

## Slices Delivered

- **S01**: Internal streaming switch. `exec_chat` → `exec_chat_stream` with `capture_content`/`capture_usage`. `ChunkCallback` type, `send_streaming()` trait method with default impl. JSON schema fallback to non-streaming.
- **S02**: Streaming chunk observer. `StreamChunk` progress event, `make_chunk_callback()` bridge, phase runner wiring, CLI inline chunk output with progress bar suspension.

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| D037 | JSON schema → non-streaming fallback | Provider compat — not all support structured output + streaming |
| D038 | ChunkCallback as Arc<dyn Fn(&str)> | Clonable, Send + Sync across actor boundaries |
| D039 | send_streaming() default → send() | MockBackend unchanged, streaming opt-in |
| D040 | Chunks only when captures_transcripts | Verbose-mode feature, no overhead in normal mode |
| D041 | Progress bar suspended during chunk writes | Prevents spinner/text interleaving |

## Files Changed

- `crates/ath-agents/src/actor/mod.rs` — streaming internals, ChunkCallback
- `crates/ath-agents/src/backend.rs` — send_streaming() trait method
- `crates/ath-agents/src/actor/{claude,gemini,codex,generic}.rs` — handle impls
- `crates/ath-orchestrator/src/progress.rs` — StreamChunk event, make_chunk_callback
- `crates/ath-orchestrator/src/phase_runner.rs` — send_streaming() calls
- `crates/ath-cli/src/progress.rs` — print_inline_chunk
- `crates/ath-cli/src/verbose.rs` — chunk rendering
- `Cargo.toml` — futures dependency

## Notes

Current task execution and review dispatch use JSON schemas, causing fallback to non-streaming `exec_chat`. The streaming path activates automatically when non-schema calls are added (multi-turn conversations, freeform prompts). Infrastructure is complete and tested.
