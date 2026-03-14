# M005: Streaming Output — Roadmap

## Slices

- [x] **S01: Internal Streaming Switch** `risk:medium` `depends:[]`
  Switch `call_provider` from `exec_chat` to `exec_chat_stream` with `capture_content`/`capture_usage`. Same return type, same behavior — just uses the streaming API internally. Proves streaming works without breaking existing tests.

- [x] **S02: Streaming Chunk Observer** `risk:medium` `depends:[S01]`
  Add optional chunk callback to `call_provider_streaming`, wire through actors. Add `StreamChunk` progress event. Verbose mode shows live tokens. Non-verbose shows activity dot.

## Dependency Graph

### S01 → S02

## Success Criteria

- All 665 existing tests pass after switching to streaming API
- Verbose mode shows real-time agent output tokens
- Token counts in reports match pre-streaming values
- Mid-stream errors handled by retry logic
