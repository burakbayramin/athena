---
id: M004
provides:
  - Extensible AgentId struct replacing hardcoded AgentKind enum
  - Config-driven agent definitions via .ath/agents.toml
  - Configurable skill routing via .ath/skills.toml
  - Generic OpenAI-compatible provider for Ollama/Groq/Together/etc.
  - CLI commands for agent inspection (ath agents list/test)
key_decisions:
  - "D026: AgentId stores provider as lowercase string — case-insensitive matching"
  - "D027: AgentRegistry keyed by provider string — extensible to any provider"
  - "D028: Custom serde visit_map handles both legacy and new format"
  - "D030: API keys referenced by env var name in config, never as literals"
  - "D033: Skills config routes override defaults, don't replace them"
  - "D035: Custom base_url uses OpenAI adapter kind via ServiceTargetResolver"
  - "D036: GenericHandle accepts optional API key for local models"
patterns_established:
  - "AgentId::new(provider, model) for extensible agent creation"
  - "agent.is_claude()/is_gemini()/is_codex() for provider branching"
  - "build_backend_for_agent() maps provider to handle — extend here for new providers"
  - "AgentConfig.resolve_api_key() checks env at call time"
observability_surfaces:
  - "ath agents list — shows all configured agents and their status"
  - "ath agents test — verifies backend construction"
requirement_outcomes:
  - id: REQ-PLUGINS
    from_status: out_of_scope
    to_status: validated
    proof: "Custom agents defined in .ath/agents.toml, loaded dynamically, used in runs"
  - id: REQ-LOCAL-MODELS
    from_status: out_of_scope
    to_status: validated
    proof: "GenericHandle with base_url supports Ollama and any OpenAI-compatible endpoint"
duration: 2h
verification_result: passed
completed_at: 2026-03-14
---

# M004: Agent & Skill Plugin System

**Replaced hardcoded 3-agent system with extensible config-driven agent definitions, configurable skill routing, and generic provider support for any OpenAI-compatible endpoint. 665 tests pass across 8 crates.**

## What Happened

**S01 (AgentId Type Refactor)** was the foundation — replacing the `AgentKind` enum with an `AgentId` struct across ~370 occurrences in 37 files. This was the highest-risk slice: one wrong pattern match and any of the 612 existing tests could break. Custom serde handles backward compatibility with old serialized data. The `AgentRegistry` switched from `mem::discriminant` keys to provider strings.

**S02 (Agent Config)** introduced `.ath/agents.toml` for config-driven agent definitions. Each entry specifies provider, model, api_key_env, and optional base_url. When no config file exists, defaults are synthesized from existing env vars — zero behavior change for current users. The hardcoded `build_agent_registry()` if-chains were replaced with a config-driven loop.

**S03 (Skill Routing)** made the taxonomy configurable via `.ath/skills.toml`. Config routes merge over (not replace) the hardcoded defaults, so users only need to specify overrides. The default agent for unrecognized tags is also configurable.

**S04 (Generic Provider)** added `GenericHandle` using genai's `ServiceTargetResolver` for custom endpoints. Any OpenAI-compatible API (Ollama, Groq, Together, Mistral) works by setting `base_url` in the agent config. Ollama support is zero-config (no API key needed).

**S05 (CLI)** added `ath agents list` and `ath agents test` subcommands for inspecting configured agents and verifying backend construction.

## Cross-Slice Verification

- `cargo test --workspace` — 665 passed, 0 failures (53 new tests across slices)
- `cargo check --workspace` — clean (only pre-existing dead_code warnings)
- `ath agents list` shows configured agents with correct status
- Backward compat: `{"Claude":"opus-4"}` deserializes into `AgentId { provider: "anthropic" }`
- No `.ath/agents.toml` + env vars = identical behavior to pre-M004

## Requirement Changes

- REQ-PLUGINS: out_of_scope → validated — custom agents defined in `.ath/agents.toml`, loaded dynamically
- REQ-LOCAL-MODELS: out_of_scope → validated — GenericHandle with base_url supports Ollama/local models

## Forward Intelligence

### What the next milestone should know
- `AgentId::new("provider", "model")` is the universal constructor. Any string pair works.
- `build_backend_for_agent()` in run.rs is the single point for adding new provider-specific backends.
- The `AgentKind` type alias is deprecated but still exists — safe to remove when ready.
- `AgentRegistry` maps one backend per provider (not per model). Two Claude models share one backend.

### What's fragile
- ConfigStore struct literals need `agents` and `skills` fields in tests — adding new fields requires updating 4 test files.
- The genai crate's `AdapterKind::from_model` does model-name-based routing (prefix matching). Custom models with names like "gpt-custom" would be misrouted to OpenAI unless `ServiceTargetResolver` overrides it.

### Authoritative diagnostics
- `ath agents list` — definitive view of configured agents and their availability
- `cargo test -p ath-config -- agents` — config parsing/validation
- `cargo test -p ath-orchestrator -- taxonomy` — skill routing (including config-driven)

### What assumptions changed
- Original assumption: genai needs custom adapter code for new providers. Actual: `ServiceTargetResolver` handles any OpenAI-compatible endpoint with zero adapter code.
- Original assumption: Ollama needs special handling. Actual: genai already routes unknown model names to Ollama by default. The GenericHandle just adds explicit base_url support.

## Files Created/Modified

- `crates/ath-types/src/agent.rs` — AgentId struct, custom serde, tests (replaces AgentKind enum)
- `crates/ath-config/src/agents.rs` — AgentConfig/AgentsConfig types, TOML parsing, defaults
- `crates/ath-config/src/skills.rs` — SkillsConfig, TOML parsing, validation
- `crates/ath-config/src/store.rs` — agents/skills fields, loading from .ath/*.toml
- `crates/ath-agents/src/actor/generic.rs` — GenericHandle for OpenAI-compatible endpoints
- `crates/ath-orchestrator/src/taxonomy.rs` — config-driven routing table
- `crates/ath-orchestrator/src/router.rs` — provider-string grouping (was discriminant)
- `crates/ath-cli/src/agents.rs` — ath agents list/test subcommands
- `crates/ath-cli/src/run.rs` — config-driven registry/backend construction
- 37+ files across 8 crates — AgentKind→AgentId mechanical migration
