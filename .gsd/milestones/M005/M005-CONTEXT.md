# M005: Streaming Output — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Add streaming output so users see agent responses in real-time as tokens arrive, instead of waiting for the complete response. The actor layer switches from `exec_chat` to `exec_chat_stream` and emits chunk events through the progress observer system.

## Why This Milestone

Currently, long agent calls (30-120 seconds) show no output until complete. Users have no idea if the agent is producing good output, hallucinating, or stuck. Streaming gives immediate feedback and makes the tool feel responsive.

## User-Visible Outcome

### When this milestone is complete, the user can:

- See agent output tokens appear in real-time during `ath run --verbose`
- See a streaming indicator (characters/second) during normal (non-verbose) runs
- Get the same final result — streaming is transparent to the orchestration layer

### Entry point / environment

- Entry point: `ath run` (with or without `--verbose`)
- Environment: local dev — CLI binary
- Live dependencies involved: LLM APIs (streaming endpoints)

## Completion Class

- Contract complete means: `call_provider` uses `exec_chat_stream`, chunks emitted via callback, final content + tokens identical to non-streaming
- Integration complete means: verbose mode shows live tokens, non-verbose shows activity indicator
- Operational complete means: retry/circuit-breaker logic works with streaming, errors mid-stream handled

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A run with `--verbose` shows streaming token output from agents
- Token counts in the final report are correct (from stream end event)
- A mid-stream error (simulated) triggers retry without corruption
- All 665 existing tests pass

## Risks and Unknowns

- **Token usage from streaming**: genai may not report token counts identically in streaming vs non-streaming. Need to verify the `End` event provides usage stats.
- **JSON schema mode + streaming**: Some providers may not support structured output with streaming. Need graceful fallback.
- **Ollama streaming**: Token usage reporting may be limited (known genai issue #4448).

## Existing Codebase / Prior Art

- `crates/ath-agents/src/actor/mod.rs` — `call_provider()` uses `exec_chat`, `run_with_retry_and_breaker()` wraps it
- `crates/ath-agents/src/backend.rs` — `AgentBackend::send()` returns complete response
- `crates/ath-orchestrator/src/progress.rs` — progress observer trait and events
- `crates/ath-cli/src/progress.rs` — terminal progress reporter
- `crates/ath-cli/src/verbose.rs` — verbose transcript sink

## Relevant Requirements

- No explicit streaming requirement — this is a UX improvement

## Scope

### In Scope

- Switch `call_provider` to use `exec_chat_stream` internally
- Emit chunk events through a new callback mechanism
- Verbose mode: print tokens as they arrive
- Non-verbose mode: show activity indicator (spinner/dots)
- Correct token counts from streaming response
- Graceful fallback when streaming is unavailable

### Out of Scope / Non-Goals

- WebSocket/SSE streaming to external clients
- Streaming the review/decomposition prompts (planning phase)
- Changing the `AgentBackend::send()` return type (still returns complete response)

## Technical Constraints

- `AgentBackend::send()` signature must not change — streaming is internal to the actor
- Token counts must match or closely approximate non-streaming counts
- All 665 existing tests must pass unchanged

## Integration Points

- `ath-agents` actor layer — streaming implementation
- `ath-orchestrator` progress system — new chunk event type
- `ath-cli` terminal output — live token display

## Open Questions

- **Chunk callback mechanism**: closure in `call_provider` vs channel vs progress observer? Leaning toward optional closure parameter — simplest, no trait changes needed.
