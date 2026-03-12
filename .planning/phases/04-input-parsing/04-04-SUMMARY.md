---
phase: 04-input-parsing
plan: 04
subsystem: cli
tags: [tokio, async, cli-wiring, parse-input, claude-handle]

# Dependency graph
requires:
  - phase: 04-input-parsing (plans 01-03)
    provides: "InputMode resolution, parse_input pipeline, codebase scanner, ClaudeHandle agent backend"
provides:
  - "Full CLI wiring: ath run -> resolve_input_mode -> ClaudeHandle::new -> parse_input -> display_project_spec_summary"
  - "Async tokio runtime for CLI entry point"
  - "Clear auth error when Anthropic API key is missing"
affects: [05-phase-planner, 07-review-engine, 08-cli-ux]

# Tech tracking
tech-stack:
  added: [tokio (ath-cli), ath-agents (ath-cli dependency)]
  patterns: [async main with tokio::main, anyhow error mapping for domain errors]

key-files:
  created: []
  modified:
    - crates/ath-cli/Cargo.toml
    - crates/ath-cli/src/main.rs

key-decisions:
  - "Error mapping via anyhow::anyhow! for both AgentError and InputError -- preserves human-readable messages without adding From impls to anyhow"

patterns-established:
  - "Async CLI: #[tokio::main] async fn main() delegates to async fn run(cli) -> Result<()>"

requirements-completed: [INPT-01, INPT-02, INPT-03]

# Metrics
duration: 4min
completed: 2026-03-12
---

# Phase 4 Plan 04: CLI Pipeline Wiring Summary

**Full CLI wiring from `ath run` through ClaudeHandle and parse_input to ProjectSpec display, closing the gap between mode resolution and actual input parsing**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T20:08:47Z
- **Completed:** 2026-03-12T20:12:47Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Wired the complete input parsing pipeline through the CLI: resolve_input_mode -> ClaudeHandle::new -> parse_input -> display_project_spec_summary
- Added async tokio runtime to CLI entry point (main and run functions)
- Removed all placeholder/deferral code from the Run command arm
- Missing API key produces clear error: "Authentication failed for claude: Anthropic API key not configured"

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire ClaudeHandle and parse_input through CLI Run command** - `cc5aacb` (feat)

## Files Created/Modified
- `crates/ath-cli/Cargo.toml` - Added ath-agents and tokio dependencies
- `crates/ath-cli/src/main.rs` - Converted to async, wired full parse pipeline, removed placeholder code

## Decisions Made
- Error mapping via `anyhow::anyhow!("{}", e)` for both AgentError and InputError to preserve human-readable messages without adding From implementations

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 4 (Input Parsing) is now fully complete with all three input modes wired through the CLI
- All 191 workspace tests pass
- Ready for Phase 5 (Phase Planner) which will build on the ProjectSpec output
- ROADMAP Success Criteria 1-3 for Phase 4 are now satisfiable from the CLI (pending a configured API key)

---
*Phase: 04-input-parsing*
*Completed: 2026-03-12*
