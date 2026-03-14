---
id: T01
result: passed
---

# T01: StreamChunk event and CLI wiring

Added `ProgressEvent::StreamChunk` variant. `make_chunk_callback()` bridges observer → `ChunkCallback`. Phase runner calls `send_streaming()` with chunk callback for both task execution and review dispatch. `VerboseTranscriptSink` prints chunks inline via `print_inline_chunk()` which suspends the progress bar spinner for clean output. 4 new tests covering callback creation, event emission, and CLI handling.
