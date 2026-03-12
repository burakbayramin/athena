# Phase 1: Foundation - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Shared type system, config loading, and error hierarchy — the base every downstream crate imports from. Delivers: Cargo workspace scaffold, typed inter-agent schemas (ProjectSpec, AgentRequest, AgentResponse, ReviewVerdict, PhaseRecord), config loading from env/file, and unified error types. No agent calls, no git operations, no orchestration logic.

</domain>

<decisions>
## Implementation Decisions

### Workspace Layout
- Fine-grained multi-crate Cargo workspace with ath-* prefix
- Minimum 6 crates: ath-types, ath-config, ath-agents, ath-git, ath-planner, ath-orchestrator, ath-cli
- ath-types includes validation logic (impl blocks with validate() methods), not pure data
- No ath-common/prelude crate — each crate imports directly from specific crates
- Workspace-level dependency management in root Cargo.toml [workspace.dependencies]
- Tokio only in crates that need async (ath-agents, ath-engine, ath-cli) — ath-types and ath-config are sync
- Tests: inline #[cfg(test)] mod tests + tests/ directory for integration tests per crate
- Binary name: `ath` (not `athena`)

### Config Behavior
- TOML format for config files
- Dual config locations: global at ~/.config/ath/ + optional project-local override (.ath.toml in project root)
- Precedence: CLI flags > env vars > project-local config > global config
- Standard provider env var names: ANTHROPIC_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY (reuse existing env vars)
- Missing API keys: warn and skip that provider (graceful degradation) — run with whatever providers are available
- `ath init` creates config interactively with guided setup (prompts for keys and preferences)

### Schema Design
- Full contracts defined upfront in Phase 1 — no extensible/HashMap-based fields
- **ProjectSpec**: project name, description, goals list, constraints list, target language/framework, expected file manifest, per-goal skill requirement tags
- **AgentRequest/AgentResponse**: typed messages between orchestrator and agents
- **ReviewVerdict**: pass/fail boolean + text reason, severity levels (critical/warning/info — only critical blocks), code suggestion capability
- **Agent identification**: Enum with model variant — e.g., Agent::Claude("opus-4") — type-safe agent with flexible model selection
- **PhaseRecord**: full audit trail — start/end timestamps, token counts per agent, estimated cost, files produced per agent, all review attempts with each verdict
- JSON serialization for all inter-agent communication (no MessagePack, no binary formats)

### Error Style
- Simple and clear: red "Error:" prefix + message + fix suggestion on next line
- Every error includes a fix hint (actionable next step)
- --verbose adds: component chain context + raw API response that caused the error
- Color output: auto-detect TTY (colored when interactive, plain when piped/redirected), override with --no-color flag or NO_COLOR env var

### Claude's Discretion
- Cargo feature flag strategy for optional provider compilation
- Exact crate boundary for planner vs orchestrator vs engine
- Internal error type granularity (how many error variants per crate)
- Specific serde attributes and derive patterns

</decisions>

<specifics>
## Specific Ideas

- Binary should feel like a modern Rust CLI tool — think `cargo`, `just`, `rg` in terms of UX quality
- The orchestra conductor metaphor should be reflected in naming where natural (e.g., "dispatch", "conduct", "perform" rather than "execute", "run")
- Config should work seamlessly in CI (env vars only) and local dev (TOML file) without code changes

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project

### Established Patterns
- None — this phase establishes the patterns

### Integration Points
- Every subsequent phase depends on types and config from this phase
- ath-types is imported by all other crates
- ath-config is imported by ath-cli and ath-agents (for API key access)

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-03-12*
