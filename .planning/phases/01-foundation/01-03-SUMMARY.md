---
phase: 01-foundation
plan: 03
subsystem: config
tags: [toml, config, env-vars, layered-config, serde, dirs]

# Dependency graph
requires:
  - phase: 01-01
    provides: Cargo workspace scaffold with ath-config crate manifest
provides:
  - ConfigStore with layered config loading (global, project-local, env vars)
  - TOML file parsing for ~/.config/ath/config.toml and .ath.toml
  - Environment variable loading for ANTHROPIC_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY
  - ConfigError enum with actionable fix hints
  - Provider availability discovery via available_providers()
affects: [01-02, 02-agents, 03-cli, all-provider-crates]

# Tech tracking
tech-stack:
  added: [toml 0.8, dotenvy 0.15, dirs 6, thiserror 2.0]
  patterns: [layered-config-merge, infallible-env-loading, option-chain-precedence]

key-files:
  created:
    - crates/ath-config/src/error.rs
    - crates/ath-config/src/file.rs
    - crates/ath-config/src/env.rs
    - crates/ath-config/src/store.rs
  modified:
    - crates/ath-config/src/lib.rs

key-decisions:
  - "RawFileConfig uses nested Option structs (ProvidersConfig, DefaultsConfig) matching TOML section structure"
  - "Env var loading is infallible -- missing vars produce None, never errors"
  - "load_from_layers() is public for testability without real files or env vars"
  - "Model defaults hardcoded as constants (opus-4, 2.5-pro, o3) with override from any config layer"

patterns-established:
  - "Config precedence chain: env > project-local > global > hardcoded defaults"
  - "Option chain merging: env.field().or(local.field()).or(global.field())"
  - "Error types include hint() method for actionable user-facing messages"

requirements-completed: [INPT-04]

# Metrics
duration: 3min
completed: 2026-03-12
---

# Phase 1 Plan 3: Config System Summary

**Layered ConfigStore loading from global TOML, project-local .ath.toml, and env vars with correct precedence and graceful degradation on missing keys**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-12T10:59:37Z
- **Completed:** 2026-03-12T11:02:59Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- ConfigError enum with FileNotFound, InvalidToml, NoConfigDir variants and actionable hint() method
- TOML file parsing with RawFileConfig supporting partial configs (all fields optional)
- Infallible environment variable loading for three provider API keys
- ConfigStore with layered merge: env > project-local > global > defaults
- 23 unit tests covering parsing, precedence, graceful degradation, and provider discovery

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ConfigError and TOML/env loading modules** - `09cb006` (feat)
2. **Task 2: Implement ConfigStore with layered merge and public API** - `ae3459a` (feat)

## Files Created/Modified
- `crates/ath-config/src/error.rs` - ConfigError enum with thiserror derives and hint() method
- `crates/ath-config/src/file.rs` - RawFileConfig struct, load_config_file() for TOML parsing
- `crates/ath-config/src/env.rs` - load_env_config() for infallible env var reading
- `crates/ath-config/src/store.rs` - ConfigStore with load(), load_from_layers(), available_providers()
- `crates/ath-config/src/lib.rs` - Module declarations and re-exports of ConfigStore, ConfigError

## Decisions Made
- Used nested Option structs (ProvidersConfig, DefaultsConfig) to mirror TOML section structure rather than flat fields
- Made load_from_layers() public so tests can exercise merge logic without filesystem or env var setup
- Env var loading is fully infallible -- returns RawFileConfig with None fields, never errors
- Model defaults are constants (opus-4, 2.5-pro, o3) overridable from any config layer

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ConfigStore ready for use by ath-cli (config display, init command) and ath-agents (API key access)
- available_providers() enables downstream code to discover which agents are usable
- Error types with hints ready for user-facing CLI error messages

## Self-Check: PASSED

- All 5 created/modified files verified present on disk
- Commit `09cb006` (Task 1) verified in git log
- Commit `ae3459a` (Task 2) verified in git log
- `cargo test -p ath-config` passes 23 tests
- `cargo check --workspace` passes

---
*Phase: 01-foundation*
*Completed: 2026-03-12*
