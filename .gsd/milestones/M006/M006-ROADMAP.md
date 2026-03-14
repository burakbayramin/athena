# M006: Multi-Turn Conversations

**Vision:** Agents retain conversation context across retry attempts, producing better refinements through multi-turn dialogue instead of cold restarts.

## Success Criteria

- Retry attempts include prior conversation history (visible in verbose transcript)
- Token counts in reports include all conversation turns
- Conversation history is truncated when approaching token limits
- All existing tests pass unchanged

## Key Risks / Unknowns

- **Token budget explosion** — multi-turn accumulates tokens fast, could hit context limits
- **Structured output across turns** — JSON schema enforcement may behave differently with conversation history

## Proof Strategy

- Token budget → retire in S01 by proving conversation history is truncated and token counts are correct
- Structured output → retire in S02 by proving retry with history produces valid JSON schema output

## Verification Classes

- Contract verification: unit tests for message types, conversation builder, truncation
- Integration verification: retry loop preserves conversation across attempts (mock backends)
- Operational verification: none (no new services)
- UAT: verbose output shows multi-turn conversation

## Milestone Definition of Done

- All slice deliverables complete
- Retry loop preserves conversation history
- Token budget enforcement prevents context overflow
- Verbose transcript shows conversation turns
- All existing + new tests pass

## Requirement Coverage

- Covers: REQ-RETRY (improved retry quality)
- Partially covers: REQ-MEM-BUDGET (token budget applies to conversation)
- Leaves for later: none

## Slices

- [x] **S01: Conversation History Types and Builder** `risk:high` `depends:[]`
  > After this: AgentRequest carries conversation history, call_provider builds multi-message ChatRequest, truncation enforced — proven by unit tests
- [x] **S02: Retry Loop Conversation Threading** `risk:medium` `depends:[S01]`
  > After this: Phase runner retry loop preserves conversation across attempts, verbose transcript shows turns — proven by integration tests with mock backends

## Boundary Map

### S01 → S02

Produces:
- `ChatMessage` type in ath-types for user/assistant message pairs
- `AgentRequest.messages` field for conversation history
- `call_provider` handling of multi-message requests
- `ConversationBuilder` for accumulating and truncating conversation history
- Token estimation for conversation truncation

Consumes:
- nothing (first slice)
