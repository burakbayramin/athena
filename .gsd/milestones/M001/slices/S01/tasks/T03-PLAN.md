# T03: Plan 03

**Slice:** S01 — **Milestone:** M001

## Description

Implement ConfigStore in ath-config: layered config loading from global TOML file, project-local TOML file, and environment variables with correct precedence and graceful degradation on missing keys.

Purpose: INPT-04 requires users to configure API keys via env vars or config file. This is the central config system that ath-cli and ath-agents will use to access provider credentials and settings.

Output: Complete ath-config crate with ConfigStore, TOML file parsing, env var loading, precedence merge logic, and tests for all config paths.
