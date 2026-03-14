# S02: Agent Config & Dynamic Registry

**Goal:** Agents are defined in `.ath/agents.toml` config and loaded into a dynamic registry at runtime — replacing hardcoded provider if-chains in `run.rs`.
**Demo:** A run loads agent definitions from `.ath/agents.toml` (falling back to env-var-based defaults when no config file exists). `build_agent_registry()` constructs the registry from config instead of hardcoded per-provider blocks.

## Must-Haves

- `AgentConfig` struct: provider, model, api_key_env, optional base_url, optional display_name
- `AgentsConfig` struct: Vec of agent definitions parsed from TOML
- `load_agents_config()` function: loads `.ath/agents.toml`, returns parsed config or defaults
- Default config generation: when no `.ath/agents.toml` exists, synthesize default agents from existing env vars / ConfigStore
- `build_agent_registry()` in run.rs refactored to iterate config entries instead of hardcoded if-chains
- All 624 existing tests pass — no behavioral change when no config file is present

## Proof Level

- This slice proves: integration
- Real runtime required: no (all tested with unit/integration tests)
- Human/UAT required: no

## Verification

- `cargo test --workspace` — 624+ tests pass, 0 failures
- `cargo test -p ath-config -- agents` — new config parsing tests pass
- New tests cover: TOML parsing, defaults fallback, validation errors, registry construction from config

## Observability / Diagnostics

- Runtime signals: config load errors surfaced as `ConfigError` variants
- Inspection surfaces: `ConfigStore::agents()` returns loaded agent definitions
- Failure visibility: actionable error messages for missing api_key_env, invalid provider, malformed TOML
- Redaction constraints: API keys never stored in config structs — only env var names

## Integration Closure

- Upstream surfaces consumed: `AgentId` from S01, `ConfigStore` from ath-config, `AgentRegistry` from phase_runner
- New wiring introduced in this slice: `AgentsConfig` loaded by `ConfigStore`, consumed by `build_agent_registry()` in run.rs
- What remains before the milestone is truly usable end-to-end: S03 (skill routing config), S04 (generic provider), S05 (CLI + e2e)

## Tasks

- [x] **T01: Define AgentConfig types and TOML parsing in ath-config** `est:30m`
  - Why: Need a structured config format for agent definitions that replaces hardcoded provider fields
  - Files: `crates/ath-config/src/agents.rs` (new), `crates/ath-config/src/lib.rs`, `crates/ath-config/Cargo.toml`
  - Do: Define `AgentConfig` (provider, model, api_key_env, base_url, display_name) and `AgentsConfig` (vec of agents). Add `load_agents_config(path)` that parses TOML. Add `default_agents_from_env()` that builds defaults from existing env vars (ANTHROPIC_API_KEY → claude agent, etc.). Validate: provider required, model required, api_key_env required, no duplicate provider+model combos. Add comprehensive tests.
  - Verify: `cargo test -p ath-config -- agents`
  - Done when: `AgentsConfig` loads from TOML string, validates, and falls back to env-based defaults

- [x] **T02: Wire agents config into ConfigStore and refactor build_agent_registry** `est:30m`
  - Why: The registry construction in run.rs needs to consume config-defined agents instead of hardcoded if-chains
  - Files: `crates/ath-config/src/store.rs`, `crates/ath-cli/src/run.rs`
  - Do: Add `agents: AgentsConfig` field to ConfigStore, load from `.ath/agents.toml` in `ConfigStore::load()` (fall back to defaults). Refactor `build_agent_registry()` and `build_planning_backend()` in run.rs to iterate `config.agents` entries. Keep backward compatibility — if no `.ath/agents.toml` exists and env vars are set, behavior is identical to current.
  - Verify: `cargo test --workspace` — all 624+ tests pass
  - Done when: `build_agent_registry()` uses config entries, `build_planning_backend()` picks first available agent from config, existing tests unchanged

## Files Likely Touched

- `crates/ath-config/src/agents.rs` (new)
- `crates/ath-config/src/lib.rs`
- `crates/ath-config/src/store.rs`
- `crates/ath-config/Cargo.toml`
- `crates/ath-cli/src/run.rs`
