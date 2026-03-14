---
id: S01
parent: M006
milestone: M006
provides:
  - ChatRole enum (User/Assistant/System)
  - ChatMessage struct with estimated_tokens()
  - AgentRequest.messages field for conversation history
  - call_provider_streaming multi-message ChatRequest construction
  - ConversationBuilder with token budget truncation
requires: []
affects:
  - S02
key_files:
  - crates/ath-types/src/agent.rs
  - crates/ath-types/src/conversation.rs
  - crates/ath-agents/src/actor/mod.rs
key_decisions:
  - "D042: ChatMessage uses ~4 chars/token for estimation — conservative, avoids tiktoken dependency"
  - "D043: Truncation keeps first 2 messages (initial context pair) + most recent — oldest middle messages dropped first"
  - "D044: messages field has serde default + skip_serializing_if — backward compat with existing serialized data"
duration: 30m
verification_result: passed
completed_at: 2026-03-14
---

# S01: Conversation History Types and Builder

**Added conversation history support to the type system and agent layer. 678 tests pass, 0 failures.**

## What Happened

1. Added `ChatRole` enum and `ChatMessage` struct to `ath-types/src/agent.rs` with convenience constructors and token estimation.
2. Added `messages: Vec<ChatMessage>` field to `AgentRequest` — backward-compatible via `serde(default)`. Updated 20+ construction sites across 13 files.
3. Updated `call_provider_streaming` to accept `&[ChatMessage]` and build multi-message `ChatRequest` when history is present.
4. Created `ConversationBuilder` with configurable token budget (default 32K) and middle-drop truncation strategy.

## Verification

- `cargo test --workspace` — 678 passed, 0 failures (+9 new)

## Forward Intelligence

S02 needs to use `ConversationBuilder` in the phase runner retry loop — accumulate user prompts and assistant responses across retry attempts, pass `messages` in subsequent `AgentRequest`s.
