---
id: S01
parent: M008
milestone: M008
provides:
  - Working `ath init` command
  - Example agents.toml and skills.toml generation
  - API key environment detection
  - --force flag for overwrite
requires: []
affects: []
key_files:
  - crates/ath-cli/src/init.rs
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S01: Working init command

**`ath init` creates `.ath/` directory structure with example configs. 699 tests pass, 0 failures.**

## What Happened

Replaced placeholder with `init_at()` function. Creates `.ath/`, `.ath/memory/`, generates `agents.toml` (with Claude, Gemini, commented Codex and Ollama examples) and `skills.toml` (with commented route examples). Detects `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY` from env and reports status. `--force` flag overwrites existing files. Refactored to accept base path for test isolation.
