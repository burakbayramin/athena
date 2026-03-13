# T01: Plan 01

**Slice:** S08 — **Milestone:** M001

## Description

Refactor the CLI surface so `ath`, `ath run`, `ath init`, and `ath report` have a stable, testable command shape that matches the locked Phase 8 UX decisions.

Purpose: This plan turns the current placeholder `main.rs` into a command-dispatch entry point the later progress, verbose, and dry-run plans can build on.

Output: `ath-cli` command modules for run/init/report, help-first no-subcommand behavior, and parsing coverage for the expanded clap surface.
