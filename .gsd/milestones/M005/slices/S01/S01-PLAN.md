# S01: Internal Streaming Switch

**Goal:** `call_provider` uses `exec_chat_stream` internally — same return type, same behavior, but the streaming API under the hood.
**Demo:** All 665 existing tests pass. Token counts identical. No visible change to user.

## Must-Haves

- `call_provider` uses `exec_chat_stream` with `capture_content(true)` and `capture_usage(true)`
- Iterates stream to completion, collects content from `StreamEnd.captured_content`
- Token counts from `StreamEnd.captured_usage`
- Fallback: if streaming fails, fall back to `exec_chat` (safety net)

## Verification

- `cargo test --workspace` — 665+ tests pass, 0 failures
- Token counts in existing tests unchanged

## Tasks

- [ ] **T01: Switch call_provider to streaming** `est:30m`
  - Why: Foundation for streaming output — must prove streaming API works identically before adding UI
  - Files: `crates/ath-agents/src/actor/mod.rs`
  - Do: Replace `exec_chat` with `exec_chat_stream` in `call_provider`. Set `ChatOptions` with `capture_usage(true)` and `capture_content(true)`. Iterate stream events, extract content/usage from `StreamEnd`. Keep JSON schema support (may need to fall back to non-streaming for structured output).
  - Verify: `cargo test --workspace`
  - Done when: All 665 tests pass with streaming under the hood

## Files Likely Touched

- `crates/ath-agents/src/actor/mod.rs`
