---
id: T02
result: passed
---

# T02: Wire agents config into ConfigStore and refactor build_agent_registry

Added `agents: AgentsConfig` field to `ConfigStore`. `load()` reads `.ath/agents.toml` when present, falls back to `default_agents_from_config()` from existing API keys. `load_from_layers()` also generates default agents for test compatibility.

Refactored `build_agent_registry()` and `build_planning_backend()` in run.rs: iterate `config.agents.agents` entries instead of hardcoded if-chains. Extracted `build_backend_for_agent()` that maps provider strings to handle constructors (anthropic→ClaudeHandle, google→GeminiHandle, openai→CodexHandle). Custom providers print warning and skip (deferred to S04).

All ConfigStore literals in test files updated with `agents: AgentsConfig::default()`.
