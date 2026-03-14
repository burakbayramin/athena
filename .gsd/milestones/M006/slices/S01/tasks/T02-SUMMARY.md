---
id: T02
result: passed
---

# T02: call_provider multi-message support

`call_provider_streaming` now accepts `&[ChatMessage]`. When non-empty, builds multi-message `ChatRequest` via `ChatRequest::from_messages()` — system context first, then history, then current prompt as final user message. Empty messages preserves existing single-message behavior.
