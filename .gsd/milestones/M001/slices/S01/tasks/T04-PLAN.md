# T04: Plan 04

**Slice:** S01 — **Milestone:** M001

## Description

Wire ath-cli binary to load config on startup, display provider availability, and format errors with the project's error style (red prefix + fix hint). Validates that ath-types and ath-config integrate correctly at the binary boundary.

Purpose: This is the integration point that proves the foundation works end-to-end. Success criteria SC-1 ("ath --version succeeds and config loads") and SC-2 ("invalid API key produces typed error") are validated here. This plan also satisfies the remaining pieces of INPT-04 (config accessible from CLI) and PLAN-04 (schemas importable from binary crate).

Output: Working `ath` binary that loads config, reports provider status, and displays errors correctly.
