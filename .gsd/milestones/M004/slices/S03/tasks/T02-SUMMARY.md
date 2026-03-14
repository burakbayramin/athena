---
id: T02
result: passed
---

# T02: Wire skills config into run.rs

Added `skills: Option<SkillsConfig>` to ConfigStore, loaded from `.ath/skills.toml` when present. Added `assign_agents_and_check_isolation_with_skills()` in run.rs that passes skills config to `build_routing_table_with_config()`. Existing `assign_agents_and_check_isolation()` delegates with `None` for backward compat. All ConfigStore literals updated with `skills: None`.
