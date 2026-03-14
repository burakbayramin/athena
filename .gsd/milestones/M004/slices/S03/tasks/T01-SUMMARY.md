---
id: T01
result: passed
---

# T01: Define SkillsConfig and config-driven routing table

Created `crates/ath-config/src/skills.rs` with `SkillsConfig` (optional default agent, routes HashMap). TOML parsing with tag normalization to lowercase. Added `build_routing_table_with_config()` and `default_agent_with_config()` to taxonomy.rs — config routes override defaults, unspecified tags keep defaults. 10 new skills config tests + 7 new taxonomy config tests.
