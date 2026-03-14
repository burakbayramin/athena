---
id: S03
parent: M004
milestone: M004
provides:
  - SkillsConfig type for configurable skill→agent routing
  - .ath/skills.toml config file support
  - build_routing_table_with_config() merges config over defaults
  - Configurable default agent via skills config
requires:
  - S01
  - S02
affects:
  - S05
key_files:
  - crates/ath-config/src/skills.rs
  - crates/ath-orchestrator/src/taxonomy.rs
  - crates/ath-cli/src/run.rs
  - crates/ath-config/src/store.rs
key_decisions:
  - "D033: Skills config routes override defaults, don't replace them — unspecified tags keep hardcoded routing"
  - "D034: Route tags normalized to lowercase in SkillsConfig::parse() for case-insensitive matching"
drill_down_paths:
  - .gsd/milestones/M004/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S03/tasks/T02-SUMMARY.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S03: Skill Routing Config

**Configurable skill→agent routing via `.ath/skills.toml` with defaults fallback. 658 tests pass (17 new).**

## What Happened

**T01** created `skills.rs` in ath-config with `SkillsConfig` (optional default agent, routes HashMap of tag→AgentRef). Added `build_routing_table_with_config()` to taxonomy.rs that merges config routes over hardcoded defaults. Config can add new routes or override existing ones. 10 skills config tests + 7 taxonomy config tests.

**T02** wired `skills: Option<SkillsConfig>` into ConfigStore (loaded from `.ath/skills.toml` when present). Added `assign_agents_and_check_isolation_with_skills()` in run.rs and called it from `run_command()` with the loaded skills config.

## Verification

- `cargo test --workspace` — 658 passed, 0 failed
- Config routes override defaults, unspecified tags retain hardcoded routing
- No `.ath/skills.toml` → identical behavior to before

## Forward Intelligence

- S04 (generic OpenAI provider) will let custom providers in skills.toml actually work
- S05 will need to display skills routing in CLI output
