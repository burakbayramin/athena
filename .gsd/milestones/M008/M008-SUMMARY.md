---
id: M008
title: "ath init Interactive Setup"
status: complete
slices_completed: 1
slices_total: 1
tests_before: 694
tests_after: 699
started: 2026-03-14
completed: 2026-03-14
---

# M008: ath init Interactive Setup — Summary

Replaced placeholder `ath init` with a working command that creates project structure, generates example configs, and validates the environment.

## What It Does

1. Creates `.ath/` and `.ath/memory/` directories
2. Generates `agents.toml` with Claude, Gemini examples (Codex, Ollama commented)
3. Generates `skills.toml` with commented route examples
4. Detects API keys from environment and reports status
5. Prints next-steps guidance
6. `--force` flag overwrites existing files

## Files Changed

- `crates/ath-cli/src/init.rs` — full implementation
- `crates/ath-cli/src/main.rs` — Default derive on GlobalArgs, test updates
- `crates/ath-cli/Cargo.toml` — toml dev-dependency
