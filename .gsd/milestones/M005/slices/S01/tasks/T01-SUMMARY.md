---
id: T01
result: passed
---

# T01: Switch call_provider to streaming and add chunk callback infrastructure

Replaced `exec_chat` with `exec_chat_stream` in `call_provider_streaming()`. Uses `ChatOptions` with `capture_content(true)` and `capture_usage(true)`. Stream iterated to completion, content/usage extracted from `StreamEnd`. JSON schema requests fall back to non-streaming (provider compat). Added `ChunkCallback` type, `on_chunk` field to `ActorMessage`, `send_streaming()` to `AgentBackend` trait with default impl. All 4 actor handles (Claude/Gemini/Codex/Generic) updated.
