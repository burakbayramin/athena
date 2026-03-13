# S08: Cli And Progress

**Goal:** Refactor the CLI surface so `ath`, `ath run`, `ath init`, and `ath report` have a stable, testable command shape that matches the locked Phase 8 UX decisions.
**Demo:** Refactor the CLI surface so `ath`, `ath run`, `ath init`, and `ath report` have a stable, testable command shape that matches the locked Phase 8 UX decisions.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Refactor the CLI surface so `ath`, `ath run`, `ath init`, and `ath report` have a stable, testable command shape that matches the locked Phase 8 UX decisions.

Purpose: This plan turns the current placeholder `main.rs` into a command-dispatch entry point the later progress, verbose, and dry-run plans can build on.

Output: `ath-cli` command modules for run/init/report, help-first no-subcommand behavior, and parsing coverage for the expanded clap surface.
- [x] **T02: Plan 02**
  - Wire the default `ath run` path into the real execution engine and add real-time terminal progress reporting that satisfies `OUTP-02`.

Purpose: Phase 7 built the execution engine, but the CLI still stops after plan display. This plan makes Athena actually run work while keeping the terminal informative instead of silent.

Output: A real `ath run` execution path with progress events from the orchestrator and a default hybrid terminal reporter in `ath-cli`.
- [x] **T03: Plan 03**
  - Layer full verbose transcript visibility onto the default execution path without destroying the normal progress experience.

Purpose: The roadmap requires `ath run --verbose` to show full agent prompt/response transcripts, but the user explicitly wants those transcripts grouped and secondary to the default live board.

Output: Verbose transcript events, redaction helpers, and grouped transcript rendering on top of the existing progress/event pipeline.
- [x] **T04: Plan 04**
  - Implement an honest dry-run path that never spends API or git work, while still printing the full routed execution plan when local planning data exists.

Purpose: `PLAN-05` is only satisfied if `--dry-run` is cost-free. Since the current planning path is LLM-backed, dry-run needs a local serialized plan artifact and a clear failure mode when that artifact is absent.

Output: `dry_run.rs`, local serialized plan-cache helpers, assigned-agent-aware plan display, and tests proving no agent/git execution occurs in dry-run mode.

## Files Likely Touched

