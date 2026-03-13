# S01: Foundation

**Goal:** Create the Cargo workspace scaffold with all 7 ath-* crates, workspace-level dependency management, and minimal compilable stubs for each crate.
**Demo:** Create the Cargo workspace scaffold with all 7 ath-* crates, workspace-level dependency management, and minimal compilable stubs for each crate.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Create the Cargo workspace scaffold with all 7 ath-* crates, workspace-level dependency management, and minimal compilable stubs for each crate.

Purpose: Every subsequent plan needs a compilable workspace to build against. This establishes the crate layout, dependency graph, and shared dependency versions that all Phase 1 plans (and all future phases) depend on.

Output: A `cargo check --workspace` passing workspace with 7 crates and a binary target named `ath`.
- [x] **T02: Plan 02**
  - Implement all typed inter-agent schemas in ath-types: error hierarchy, agent identification, project spec, review verdict, and phase record -- with validation methods and round-trip serialization tests.

Purpose: PLAN-04 requires typed JSON schemas at every agent boundary. These types are the contracts that every downstream crate imports. Getting them right in Phase 1 prevents the #1 multi-agent failure mode (schema mismatches at boundaries).

Output: Complete ath-types crate with all inter-agent schemas, validation logic, and comprehensive round-trip tests.
- [x] **T03: Plan 03**
  - Implement ConfigStore in ath-config: layered config loading from global TOML file, project-local TOML file, and environment variables with correct precedence and graceful degradation on missing keys.

Purpose: INPT-04 requires users to configure API keys via env vars or config file. This is the central config system that ath-cli and ath-agents will use to access provider credentials and settings.

Output: Complete ath-config crate with ConfigStore, TOML file parsing, env var loading, precedence merge logic, and tests for all config paths.
- [x] **T04: Plan 04**
  - Wire ath-cli binary to load config on startup, display provider availability, and format errors with the project's error style (red prefix + fix hint). Validates that ath-types and ath-config integrate correctly at the binary boundary.

Purpose: This is the integration point that proves the foundation works end-to-end. Success criteria SC-1 ("ath --version succeeds and config loads") and SC-2 ("invalid API key produces typed error") are validated here. This plan also satisfies the remaining pieces of INPT-04 (config accessible from CLI) and PLAN-04 (schemas importable from binary crate).

Output: Working `ath` binary that loads config, reports provider status, and displays errors correctly.

## Files Likely Touched

