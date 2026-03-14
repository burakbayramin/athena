# M008: ath init Interactive Setup — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Replace the placeholder `ath init` command with a working interactive setup flow that creates the `.ath/` directory structure, generates example config files, and validates environment.

## Why This Milestone

New users have no guided entry point. They need to manually create `.ath/` directory, config files, and know which env vars to set. `ath init` should handle all of this.

## User-Visible Outcome

- `ath init` creates `.ath/` directory with example agents.toml and skills.toml
- Detects available API keys from environment
- Prints summary of what was created and what's needed

## Scope

### In Scope
- Create `.ath/` directory structure
- Generate example `agents.toml` and `skills.toml`  
- Detect available API keys from environment
- Print setup summary

### Out of Scope
- Interactive prompts for API key input (use secure_env_collect separately)
- Project analysis/spec generation
