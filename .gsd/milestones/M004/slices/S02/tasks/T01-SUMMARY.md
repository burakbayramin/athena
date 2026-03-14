---
id: T01
result: passed
---

# T01: Define AgentConfig types and TOML parsing in ath-config

Created `crates/ath-config/src/agents.rs` with `AgentConfig` (provider, model, api_key_env, base_url, display_name) and `AgentsConfig` (vec of agents). TOML parsing via `AgentsConfig::parse()` / `::load()`. Validation: empty fields rejected, duplicate provider+model rejected. `default_agents_from_env()` for env-var fallback, `default_agents_from_config()` to bridge old ConfigStore fields. Added `InvalidAgentConfig` error variant with actionable hint. 18 new tests.
