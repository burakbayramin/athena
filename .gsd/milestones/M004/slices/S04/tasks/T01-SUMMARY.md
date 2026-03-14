---
id: T01
result: passed
---

# T01: Implement GenericHandle with custom base_url support

Created `GenericHandle` in `crates/ath-agents/src/actor/generic.rs`. Uses genai's `ServiceTargetResolver` to override endpoint and adapter kind for custom base URLs. `AuthResolver` injects the API key for all adapter kinds. Works without API key (Ollama). Works without base_url (Groq-style providers). 5 new tests.
