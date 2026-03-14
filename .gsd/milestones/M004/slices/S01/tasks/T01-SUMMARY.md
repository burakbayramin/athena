---
id: T01
result: passed
---

# T01: Define AgentId in ath-types with backward-compatible serde

Created `AgentId` struct with `provider: String`, `model: String`. Custom serde handles both new format `{"provider":"anthropic","model":"opus-4"}` and legacy `{"Claude":"opus-4"}`. Helper constructors `claude()`, `gemini()`, `codex()`. Provider detection via `is_claude()`, `is_gemini()`, `is_codex()`. Provider normalized to lowercase. Implements Display, Hash, Eq, Clone, Debug. `AgentKind` kept as deprecated type alias. 12 new tests.
