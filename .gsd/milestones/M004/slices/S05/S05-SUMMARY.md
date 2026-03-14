---
id: S05
parent: M004
milestone: M004
provides:
  - ath agents list command showing all configured agents
  - ath agents test command verifying backend construction
  - Skills config status in agents list output
requires:
  - S01
  - S02
  - S03
  - S04
key_files:
  - crates/ath-cli/src/agents.rs
  - crates/ath-cli/src/main.rs
drill_down_paths:
  - .gsd/milestones/M004/slices/S05/tasks/T01-SUMMARY.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S05: CLI & End-to-End Integration

**CLI commands `ath agents list` and `ath agents test` for inspecting configured agents. 665 tests pass (2 new).**

## What Happened

Created agents.rs with `AgentsArgs` and two subcommands. `list` loads config, iterates agents, prints status table with provider/model, API key status, base_url, and type. `test` constructs backends to validate initialization. Skills config status shown when present. Wired into main.rs CLI dispatch.

## Verification

- `cargo test --workspace` — 665 passed, 0 failed
- `ath agents --help` shows list/test subcommands
- `ath agents list` shows configured agents with correct status
