# M004: Agent & Skill Plugin System

**Vision:** Agents and skills are defined via config, not code — any OpenAI-compatible provider works out of the box, skill routing is user-configurable, and the type system supports unlimited providers.

## Success Criteria

- `AgentKind` enum is replaced by `AgentId` struct across all crates
- Agents can be defined in `.ath/agents.toml` with provider, model, base_url, api_key_env
- Skill→agent routing can be configured via `.ath/skills.toml`
- A generic OpenAI-compatible provider handles Ollama and any base_url endpoint
- `ath agents list` shows all available agents (built-in + custom)
- `ath agents test` verifies agent connectivity
- All existing tests pass with the refactored type system
- Old serialized data (reports, checkpoints) deserializes correctly

## Key Risks / Unknowns

- **AgentKind pervasiveness** — used in pattern matches across 8 crates. Must change ~100 call sites without breaking 612 tests.
- **genai adapter mapping** — genai uses `AdapterKind` enum internally. Need to map provider strings to adapter kinds or bypass genai for custom providers.
- **Serde backward compatibility** — `AgentKind::Claude("opus-4")` serialized as `{"Claude":"opus-4"}`. The new `AgentId` must be able to read this format.

## Proof Strategy

- AgentKind pervasiveness → retire in S01 by proving all 612 tests pass after replacing the enum with AgentId
- genai adapter mapping → retire in S04 by proving a custom OpenAI-compatible provider can send requests
- Serde backward compat → retire in S01 by proving old serialized AgentKind format deserializes into AgentId

## Verification Classes

- Contract verification: unit tests for AgentId, config parsing, routing, provider trait
- Integration verification: run with custom agent executes and produces output
- Operational verification: `ath agents list/test` work, config errors actionable
- UAT / human verification: none

## Milestone Definition of Done

This milestone is complete only when all are true:

- AgentId replaces AgentKind everywhere — no enum pattern matches remain
- Config-defined agents load and register alongside built-in providers
- Skill routing uses config when present, defaults when absent
- Generic OpenAI provider works for Ollama/custom endpoints
- `ath agents list|test` CLI commands work
- All existing tests pass (no regressions)
- Old reports/checkpoints deserialize correctly

## Requirement Coverage

- Covers: REQ-PLUGINS (custom agent definitions), REQ-LOCAL-MODELS (local/self-hosted models via Ollama)
- Partially covers: none
- Leaves for later: agent marketplace, streaming, multi-turn
- Orphan risks: none

## Slices

- [x] **S01: AgentId Type Refactor** `risk:high` `depends:[]`
  > After this: `AgentKind` enum replaced by `AgentId` struct across all 8 crates — all 612 tests pass, old serialized format deserializes correctly

- [x] **S02: Agent Config & Dynamic Registry** `risk:medium` `depends:[S01]`
  > After this: agents defined in `.ath/agents.toml` load into the registry alongside built-in providers — proven by unit tests for config parsing and registry construction

- [x] **S03: Configurable Skill Routing** `risk:medium` `depends:[S01]`
  > After this: skill→agent mapping loaded from `.ath/skills.toml`, falls back to defaults when absent — proven by unit tests for config parsing and routing with custom skills

- [ ] **S04: Generic OpenAI Provider** `risk:medium` `depends:[S01,S02]`
  > After this: a generic OpenAI-compatible provider sends requests to any base_url (Ollama, Groq, etc.) — proven by unit test with mock HTTP server

- [ ] **S05: CLI & End-to-End Integration** `risk:low` `depends:[S01,S02,S03,S04]`
  > After this: `ath agents list|test` work, a full run with mixed built-in and custom agents executes — proven by CLI tests and integration test

## Boundary Map

### S01 → all

Produces:
- `AgentId` struct with `provider: String`, `model: String`, `Display`, `Hash`, `Eq`, custom serde (backward compat)
- `AgentId::new(provider, model)` constructor
- `AgentId::provider()`, `AgentId::model()`, `AgentId::provider_name()` methods
- Built-in constants: `AgentId::claude(model)`, `AgentId::gemini(model)`, `AgentId::codex(model)`
- All crates updated to use `AgentId` instead of `AgentKind`

Consumes:
- nothing (first slice)

### S01 + S02 → S04, S05

Produces:
- `AgentConfig` struct parsed from TOML (provider, model, base_url, api_key_env, system_prompt)
- `AgentsConfig` loading from `.ath/agents.toml`
- Dynamic registry construction from config

Consumes:
- `AgentId` from S01

### S01 → S03

Produces:
- `SkillsConfig` parsed from `.ath/skills.toml`
- `build_routing_table_from_config(skills_config) -> HashMap<String, AgentId>`
- Fallback to built-in defaults when config absent

Consumes:
- `AgentId` from S01

### S01 + S02 → S04

Produces:
- `GenericOpenAiProvider` implementing `AgentBackend`
- Config-driven: `base_url`, `model`, `api_key_env`
- Compatible with Ollama, Groq, Together, any OpenAI-compatible API

Consumes:
- `AgentId` from S01
- `AgentConfig` from S02

### S01 + S02 + S03 + S04 → S05

Produces:
- `ath agents list` — shows all agents with provider, model, status
- `ath agents test` — sends test request to each agent, reports success/failure
- End-to-end integration test with mixed agent types

Consumes:
- Everything from S01–S04
