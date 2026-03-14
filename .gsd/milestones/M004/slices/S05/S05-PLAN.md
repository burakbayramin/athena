# S05: CLI & End-to-End Integration

**Goal:** `ath agents list` shows all configured agents and their status. `ath agents test` verifies connectivity. End-to-end: a config-defined agent can be used in a run.
**Demo:** `ath agents list` prints a table of agents (provider, model, status), `ath agents test` pings each.

## Must-Haves

- `ath agents list` subcommand showing configured agents
- `ath agents test` subcommand attempting a minimal call to each agent
- Both read from ConfigStore (agents config)
- Clean, informative CLI output

## Verification

- `cargo test --workspace` — 663+ tests pass, 0 failures
- `cargo test -p ath-cli -- agents` — new CLI tests pass
- CLI help includes agents subcommand

## Tasks

- [x] **T01: Add ath agents list/test subcommands** `est:30m`
  - Why: Users need to see what agents are configured and verify they work
  - Files: `crates/ath-cli/src/agents.rs` (new), `crates/ath-cli/src/main.rs`
  - Do: Create agents module with `list` and `test` subcommands. `list`: load config, iterate agents, print status table (provider, model, api_key status, base_url). `test`: build backends and send a trivial prompt to each. Wire into CLI with `ath agents list` and `ath agents test`.
  - Verify: `cargo test -p ath-cli -- agents`
  - Done when: `ath agents list` compiles, shows table. Tests verify CLI shape and list output.

## Files Likely Touched

- `crates/ath-cli/src/agents.rs` (new)
- `crates/ath-cli/src/main.rs`
