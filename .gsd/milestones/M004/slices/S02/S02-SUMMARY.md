---
id: S02
parent: M004
milestone: M004
provides:
  - AgentConfig and AgentsConfig types for config-driven agent definitions
  - TOML parsing and validation for .ath/agents.toml
  - Backward-compatible defaults from env vars when no config file exists
  - Config-driven build_agent_registry() and build_planning_backend()
requires:
  - S01
affects:
  - S03
  - S04
  - S05
key_files:
  - crates/ath-config/src/agents.rs
  - crates/ath-config/src/store.rs
  - crates/ath-config/src/error.rs
  - crates/ath-cli/src/run.rs
key_decisions:
  - "D030: API keys referenced by env var name in config (api_key_env), never stored as literals"
  - "D031: AgentsConfig defaults synthesized from ConfigStore fields for backward compat"
  - "D032: Custom providers skip with warning in build_backend_for_agent — deferred to S04"
patterns_established:
  - "build_backend_for_agent() maps provider string to handle constructor — extend here for new providers"
  - "AgentConfig.resolve_api_key() checks env at call time, not load time"
drill_down_paths:
  - .gsd/milestones/M004/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S02/tasks/T02-SUMMARY.md
duration: 25m
verification_result: passed
completed_at: 2026-03-14
---

# S02: Agent Config & Dynamic Registry

**Config-driven agent definitions with TOML parsing, validation, and dynamic registry construction. 642 tests pass (18 new).**

## What Happened

**T01** created `agents.rs` in ath-config with `AgentConfig` (provider, model, api_key_env, base_url, display_name) and `AgentsConfig`. TOML parsing validates required fields and rejects duplicates. `default_agents_from_config()` bridges the old ConfigStore fields to the new format. 18 tests cover parsing, validation, defaults, and file I/O.

**T02** wired `AgentsConfig` into `ConfigStore` — loaded from `.ath/agents.toml` when present, falling back to env-var-based defaults. Refactored `build_agent_registry()` and `build_planning_backend()` to iterate config entries instead of hardcoded if-chains. Extracted `build_backend_for_agent()` for provider→handle mapping.

## Verification

- `cargo test --workspace` — 642 passed, 0 failed
- `cargo check --workspace` — clean
- Backward compat: no `.ath/agents.toml` + env vars set = identical behavior to pre-refactor

## Forward Intelligence

### What the next slice should know
- `AgentsConfig` is loaded by `ConfigStore::load()` and available as `config.agents`
- `build_backend_for_agent()` in run.rs is where S04 will add the generic OpenAI provider case
- `AgentConfig.base_url` is parsed but not yet consumed — S04 will use it
- `AgentConfig.resolve_api_key()` checks env at call time — safe for runtime changes

### What's fragile
- `ConfigStore` struct literals in tests now need `agents: AgentsConfig::default()` — any new ConfigStore field requires updating 4 test files
