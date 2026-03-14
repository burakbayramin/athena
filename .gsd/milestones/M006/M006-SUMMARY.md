---
id: M006
title: "Multi-Turn Conversations"
status: complete
slices_completed: 2
slices_total: 2
tests_before: 669
tests_after: 679
started: 2026-03-14
completed: 2026-03-14
---

# M006: Multi-Turn Conversations — Summary

Added conversation history support enabling agents to see their prior attempts during retry, improving refinement quality through multi-turn dialogue.

## Architecture

```
ConversationBuilder (token-budgeted accumulator)
  → AgentRequest.messages (Vec<ChatMessage>)
    → call_provider_streaming (builds multi-message ChatRequest)
      → genai ChatRequest::from_messages()
        → LLM sees full conversation context

Phase runner retry loop:
  attempt 1: fresh prompt → response → record in ConversationBuilder
  attempt 2: retry prompt + messages[user,assistant] → response → record
  attempt 3: retry prompt + messages[user,assistant,user,assistant] → response
```

## Slices Delivered

- **S01**: ChatMessage/ChatRole types, AgentRequest.messages field, call_provider multi-message support, ConversationBuilder with 32K token budget and middle-drop truncation. 9 new tests.
- **S02**: Phase runner retry loop threads conversation history per task. CapturingMockBackend for test inspection. Integration test proves history is threaded. 1 new test.

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| D042 | ~4 chars/token estimation | Conservative, avoids tiktoken dependency |
| D043 | Truncation keeps first 2 + most recent, drops middle | Preserves initial context and latest state |
| D044 | messages field serde default + skip_serializing_if | Backward compat with existing serialized data |
| D045 | Conversation per task (keyed by name) | Each task has independent multi-turn history |
| D046 | Records prompt+response after each execution | History accumulates naturally across retries |

## Files Changed

- `crates/ath-types/src/agent.rs` — ChatRole, ChatMessage, AgentRequest.messages
- `crates/ath-types/src/conversation.rs` — ConversationBuilder (new)
- `crates/ath-types/src/lib.rs` — exports
- `crates/ath-agents/src/actor/mod.rs` — multi-message call_provider_streaming
- `crates/ath-agents/src/mock.rs` — CapturingMockBackend (new)
- `crates/ath-orchestrator/src/phase_runner.rs` — conversation threading in retry loop
- 13 files updated for messages: vec![] field addition
