# M004: Agent & Skill Plugin System — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Replace Athena's hardcoded 3-agent system (Claude/Gemini/Codex) with an extensible plugin architecture where agents are defined via config, skills are mapped to agents via config, and any OpenAI-compatible provider (including Ollama/local models) can be added without code changes.

## Why This Milestone

Athena currently only works with three specific providers hardcoded into the type system. Users who want to use Ollama, local models, different Claude/Gemini models, or third-party OpenAI-compatible APIs (Groq, Together, Mistral) are stuck. The hardcoded `AgentKind` enum prevents extensibility at a fundamental level — every new provider requires code changes across 8 crates.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Define custom agents in `.ath/agents.toml` with provider, model, base_url, and API key reference
- Map skills to agents in `.ath/skills.toml` (or use sane defaults)
- Run `ath run` with Ollama or any OpenAI-compatible local model
- Run `ath agents list` to see all configured agents
- Run `ath agents test` to verify agent connectivity
- Mix built-in providers (Claude, Gemini, Codex) with custom providers in the same run

### Entry point / environment

- Entry point: `.ath/agents.toml` (config), `ath agents` (CLI), `ath run` (execution)
- Environment: local dev — CLI binary
- Live dependencies involved: LLM APIs (existing + any OpenAI-compatible endpoint)

## Completion Class

- Contract complete means: AgentId replaces AgentKind everywhere, dynamic registry loads from config, skill routing uses config, provider trait implemented for built-in + generic
- Integration complete means: a run uses a mix of built-in and config-defined agents, routing respects skill config
- Operational complete means: `ath agents list/test` work, Ollama connection verified, config errors produce actionable messages

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A run with a config-defined custom agent (generic OpenAI-compatible) executes and produces output
- Skill routing follows `.ath/skills.toml` when present, falls back to defaults when absent
- `ath agents list` shows both built-in and custom agents
- All existing tests pass with the refactored type system

## Risks and Unknowns

- **AgentKind enum is pervasive** — used in pattern matches across 8 crates, ~100 call sites. The refactor must not break the 612 existing tests. Strategy: introduce `AgentId` struct with `provider()` and `model()` methods that mirror existing API, implement `From<AgentKind>` for migration, then remove enum.
- **genai crate coupling** — the `ath-agents` actor system uses `genai::adapter::AdapterKind` enum internally. Need to map string provider names to adapter kinds dynamically.
- **Serde compatibility** — `AgentKind` is serialized in reports, observations, checkpoints. The new `AgentId` must deserialize old format (tagged enum) for backward compat.

## Existing Codebase / Prior Art

- `crates/ath-types/src/agent.rs` — `AgentKind` enum with Claude/Gemini/Codex variants, `AgentRequest`, `AgentResponse`
- `crates/ath-agents/src/actor/` — hardcoded ClaudeHandle, GeminiHandle, CodexHandle
- `crates/ath-agents/src/backend.rs` — `AgentBackend` trait (already provider-agnostic)
- `crates/ath-orchestrator/src/taxonomy.rs` — hardcoded skill→agent routing table
- `crates/ath-orchestrator/src/router.rs` — majority-vote routing using taxonomy
- `crates/ath-config/src/store.rs` — hardcoded 3 API keys + 3 model names
- `crates/ath-cli/src/run.rs` — hardcoded `build_agent_registry` with if-chains per provider

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions.

## Relevant Requirements

- REQ-PLUGINS: Plugin system for custom agent definitions — currently out of scope, this milestone validates it
- REQ-LOCAL-MODELS: Support for local/self-hosted models — currently out of scope, this milestone validates it

## Scope

### In Scope

- `AgentId` struct replacing `AgentKind` enum (provider + model strings)
- Backward-compatible serde for existing reports/checkpoints
- `.ath/agents.toml` config format for defining agents
- `.ath/skills.toml` config format for skill→agent mapping
- Generic OpenAI-compatible provider (covers Ollama, Groq, Together, etc.)
- `ath agents list|test` CLI subcommands
- Dynamic agent registry from config
- Default skill→agent routing when no config present

### Out of Scope / Non-Goals

- GUI/TUI for agent configuration
- Agent marketplace or sharing
- Fine-tuning or training custom models
- Streaming responses
- Multi-turn conversations

## Technical Constraints

- `AgentId` must be `Clone + Debug + Serialize + Deserialize + PartialEq + Eq + Hash`
- Old serialized `AgentKind` format must deserialize into `AgentId` (backward compat)
- Provider string is case-insensitive for matching (e.g., "anthropic" == "Anthropic")
- All 612 existing tests must pass after refactor

## Integration Points

- `ath-types` — `AgentId` replaces `AgentKind` in `AgentRequest`, `AgentResponse`, `AgentContribution`, `PhaseRecord`
- `ath-agents` — provider registry, generic OpenAI provider
- `ath-orchestrator` — routing, taxonomy, coordinator, memory
- `ath-config` — agent/skill config loading
- `ath-cli` — agent subcommands, registry construction
- `ath-memory` — observation types use `AgentKind`

## Open Questions

- **Provider name standardization**: "anthropic" vs "claude", "google" vs "gemini"? Leaning toward "anthropic"/"google"/"openai" for providers, model names separate.
- **API key reference in agents.toml**: literal key or env var name? Leaning toward env var reference (`api_key_env = "OLLAMA_API_KEY"`) for security.
