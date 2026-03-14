# M006: Multi-Turn Conversations — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Add conversation history support so agents can have multi-turn exchanges within a task execution. Instead of a single prompt→response cycle, the system can send follow-up messages that include the full conversation context, enabling iterative refinement without starting fresh.

## Why This Milestone

Currently, when a review fails, the retry loop builds a new prompt from scratch with feedback injected as text. The agent loses all context from its prior attempt — it's starting cold. Multi-turn preserves conversation context, leading to better-quality refinements. This also unlocks future patterns like self-correction, clarification requests, and chain-of-thought prompting.

## User-Visible Outcome

### When this milestone is complete, the user can:

- See agents refine output iteratively within a task (visible in verbose mode transcript)
- Get better retry quality because agents see their prior attempt in conversation history
- Same CLI interface — `ath run` — no new commands needed

### Entry point / environment

- Entry point: `ath run` (with or without `--verbose`)
- Environment: local dev — CLI binary
- Live dependencies involved: LLM APIs (multi-turn chat endpoints)

## Completion Class

- Contract complete means: `AgentRequest` supports conversation history, `call_provider` builds multi-message `ChatRequest`, retry loop preserves conversation
- Integration complete means: retry with conversation history produces better outcomes than fresh retry
- Operational complete means: token budget enforcement on conversation history prevents context overflow

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A retry scenario preserves conversation history and the agent references prior context
- Token counts in the final report include all turns
- Conversation history is trimmed when approaching token limits
- All existing tests pass

## Risks and Unknowns

- **Token budget explosion**: Multi-turn accumulates tokens fast. Need conversation truncation/summarization strategy.
- **Structured output across turns**: JSON schema enforcement on follow-up messages may behave differently per provider.
- **Test complexity**: Hard to unit-test multi-turn quality. Focus on structural tests (history is passed, truncated correctly).

## Existing Codebase / Prior Art

- `crates/ath-types/src/agent.rs` — `AgentRequest` currently has single `prompt` field
- `crates/ath-agents/src/actor/mod.rs` — `call_provider` builds single-message `ChatRequest`
- `crates/ath-orchestrator/src/phase_runner.rs` — retry loop builds fresh prompts
- `crates/ath-orchestrator/src/phase_runner.rs` — `review::build_retry_prompt` injects feedback as text

## Relevant Requirements

- REQ-RETRY — Multi-turn improves retry quality by preserving conversation context

## Scope

### In Scope

- `AgentRequest` conversation history field (Vec of messages)
- `call_provider` multi-message `ChatRequest` construction
- Retry loop preserving conversation across attempts
- Token budget enforcement on conversation history
- Verbose mode showing conversation turns

### Out of Scope / Non-Goals

- Agent-initiated follow-up questions (agents asking for clarification)
- Conversation branching/forking
- Persistent conversation storage across runs
- Changing the `AgentBackend` trait signature (history goes in `AgentRequest`)

## Technical Constraints

- genai `ChatRequest` already supports multi-message via `ChatMessage` list
- Token counting must include all messages in conversation
- Existing single-turn behavior must remain default — multi-turn is opt-in per retry

## Integration Points

- `ath-types` — message types for conversation history
- `ath-agents` — multi-message request construction
- `ath-orchestrator` — retry loop conversation management
- `ath-cli` — verbose transcript shows conversation turns

## Open Questions

- **Truncation strategy**: Drop oldest messages? Summarize? Keep system + last N? Leaning toward keep system + first + last N user/assistant pairs.
