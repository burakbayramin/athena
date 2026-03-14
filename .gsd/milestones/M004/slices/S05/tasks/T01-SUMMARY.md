---
id: T01
result: passed
---

# T01: Add ath agents list/test subcommands

Created `crates/ath-cli/src/agents.rs` with `AgentsArgs` and `AgentsCommand::List`/`Test` subcommands. `list` shows configured agents with status, base_url, type (built-in/custom). `test` constructs backends and validates they initialize. Wired into main.rs as `ath agents list` and `ath agents test`. 2 new CLI parse tests.
