---
id: T03
result: passed
---

# T03: ConversationBuilder with truncation

`ConversationBuilder` in `crates/ath-types/src/conversation.rs`. Default 32K token budget. `push_user`/`push_assistant`/`push` accumulate messages. Token estimation at ~4 chars/token. Truncation strategy: keep first 2 messages (initial context pair) + most recent, drop from middle. 9 tests covering empty state, accumulation, truncation, budget preservation.
