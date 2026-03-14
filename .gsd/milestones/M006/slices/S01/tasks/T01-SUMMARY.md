---
id: T01
result: passed
---

# T01: ChatMessage type and AgentRequest.messages field

Added `ChatRole` enum (User/Assistant/System), `ChatMessage` struct with convenience constructors and `estimated_tokens()`. Added `messages: Vec<ChatMessage>` to `AgentRequest` with `#[serde(default, skip_serializing_if)]`. Updated all 20+ construction sites across 13 files to include `messages: vec![]`.
