# S03: Skill Routing Config

**Goal:** Skill→agent routing is configurable via `.ath/skills.toml`, with the current hardcoded table as the default fallback.
**Demo:** When `.ath/skills.toml` exists, its tag→agent mappings override the defaults. When absent, existing hardcoded routing is unchanged.

## Must-Haves

- `SkillsConfig` type: tag→provider/model mappings, default agent
- TOML parsing for `.ath/skills.toml`
- `build_routing_table()` accepts optional config and merges with defaults
- Default agent configurable (currently hardcoded to Claude)
- All existing routing tests pass unchanged when no config present

## Verification

- `cargo test --workspace` — 642+ tests pass, 0 failures
- New tests cover: TOML parsing, config-driven routing, merge with defaults, custom default agent

## Tasks

- [x] **T01: Define SkillsConfig and config-driven routing table** `est:30m`
  - Why: Routing is hardcoded — users can't customize which agent handles which skill tags
  - Files: `crates/ath-config/src/skills.rs` (new), `crates/ath-config/src/lib.rs`, `crates/ath-orchestrator/src/taxonomy.rs`
  - Do: Define `SkillsConfig` with `routes: HashMap<String, AgentRef>` (where AgentRef is provider+model) and optional `default_agent`. Add `load_skills_config(path)`. In taxonomy.rs, add `build_routing_table_with_config(skills_config)` that merges config routes over defaults. Existing `build_routing_table()` unchanged (calls new function with None).
  - Verify: `cargo test -p ath-config -- skills` and `cargo test -p ath-orchestrator -- taxonomy`
  - Done when: Config-driven routing works, defaults unchanged, merging tested

- [x] **T02: Wire skills config into run.rs** `est:15m`
  - Why: The routing table construction in run.rs needs to use the skills config
  - Files: `crates/ath-config/src/store.rs`, `crates/ath-cli/src/run.rs`
  - Do: Add `skills: Option<SkillsConfig>` to ConfigStore, load from `.ath/skills.toml` if present. Pass to `build_routing_table_with_config()` in run.rs.
  - Verify: `cargo test --workspace`
  - Done when: Skills config flows from ConfigStore through to routing table construction

## Files Likely Touched

- `crates/ath-config/src/skills.rs` (new)
- `crates/ath-config/src/lib.rs`
- `crates/ath-config/src/store.rs`
- `crates/ath-orchestrator/src/taxonomy.rs`
- `crates/ath-cli/src/run.rs`
