---
phase: 04-input-parsing
plan: 01
subsystem: input
tags: [clap, serde, thiserror, input-modes, cli]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "ProjectSpec, AgentRequest, CLI structure, error patterns"
  - phase: 02-agent-clients
    provides: "AgentError type, AgentBackend trait, MockBackend"
provides:
  - "InputMode enum (NaturalLanguage, SpecFile, Codebase)"
  - "InputError enum with 9 variants and fix hints"
  - "resolve_input_mode function for CLI dispatch"
  - "AgentRequest.json_schema field for structured LLM output"
  - "CLI --spec and --codebase flags on ath run"
affects: [04-input-parsing, 05-phase-planner, 08-cli-ux]

# Tech tracking
tech-stack:
  added: [ignore 0.4]
  patterns: [input-mode-dispatch, error-with-hints, serde-skip-serializing-if]

key-files:
  created:
    - crates/ath-planner/src/input/mod.rs
    - crates/ath-planner/src/input/error.rs
  modified:
    - Cargo.toml
    - crates/ath-planner/Cargo.toml
    - crates/ath-planner/src/lib.rs
    - crates/ath-types/src/agent.rs
    - crates/ath-cli/src/main.rs
    - crates/ath-cli/Cargo.toml
    - crates/ath-agents/src/actor/claude.rs
    - crates/ath-agents/src/actor/codex.rs
    - crates/ath-agents/src/actor/gemini.rs
    - crates/ath-agents/src/mock.rs
    - crates/ath-agents/tests/integration.rs

key-decisions:
  - "Added serde(default) on json_schema field for backward-compatible deserialization of old JSON without the field"

patterns-established:
  - "InputMode dispatch: resolve_input_mode maps CLI args to typed enum"
  - "Error hint pattern: InputError::hint() returns user-facing fix suggestions"
  - "Backward-compatible struct extension: serde(default, skip_serializing_if) for new Option fields"

requirements-completed: [INPT-01, INPT-02, INPT-03]

# Metrics
duration: 6min
completed: 2026-03-12
---

# Phase 4 Plan 01: Input Parsing Infrastructure Summary

**InputMode enum with 3 variants, InputError with 9 variants, resolve_input_mode dispatch, AgentRequest.json_schema extension, and CLI --spec/--codebase flags**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-12T14:23:39Z
- **Completed:** 2026-03-12T14:30:06Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- InputMode enum discriminating NaturalLanguage, SpecFile, and Codebase input modes with serde round-trip support
- resolve_input_mode enforcing --spec exclusivity, --codebase requires description, and no-input error
- InputError covering all 9 failure modes with user-facing fix hints
- AgentRequest.json_schema field added backward-compatibly across entire workspace (5 crate files updated)
- CLI --spec and --codebase flags wired into ath run with resolve_input_mode dispatch

## Task Commits

Each task was committed atomically:

1. **Task 1: InputMode enum, InputError type, and AgentRequest json_schema extension** - `efc4cd7` (feat)
2. **Task 2: Extend CLI with --spec and --codebase flags** - `a624a1a` (feat)

## Files Created/Modified
- `crates/ath-planner/src/input/mod.rs` - InputMode enum, resolve_input_mode function, 10 unit tests
- `crates/ath-planner/src/input/error.rs` - InputError enum with 9 variants and hint() method
- `crates/ath-planner/src/lib.rs` - Added pub mod input
- `crates/ath-planner/Cargo.toml` - Added ath-agents, serde, serde_json, thiserror, tokio, ignore dependencies
- `crates/ath-types/src/agent.rs` - Added json_schema field to AgentRequest with 2 new tests
- `crates/ath-cli/src/main.rs` - Added --spec and --codebase flags, resolve_input_mode dispatch
- `crates/ath-cli/Cargo.toml` - Added ath-planner dependency
- `Cargo.toml` - Added ignore = "0.4" to workspace dependencies
- `crates/ath-agents/src/actor/claude.rs` - Updated AgentRequest construction with json_schema: None
- `crates/ath-agents/src/actor/codex.rs` - Updated AgentRequest construction with json_schema: None
- `crates/ath-agents/src/actor/gemini.rs` - Updated AgentRequest construction with json_schema: None
- `crates/ath-agents/src/mock.rs` - Updated AgentRequest construction with json_schema: None
- `crates/ath-agents/tests/integration.rs` - Updated AgentRequest construction with json_schema: None

## Decisions Made
- Added `serde(default)` alongside `skip_serializing_if` on json_schema field to ensure backward-compatible deserialization of JSON payloads that lack the field (old format still deserializes correctly)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- InputMode and InputError types ready for use by subsequent input parsing plans (spec file reader, codebase scanner, LLM parsing pipeline)
- AgentRequest.json_schema field ready for threading through call_provider in Plan 04-02
- CLI flags wired and producing correct InputMode, ready for full parsing implementation
- All 168 workspace tests pass

---
*Phase: 04-input-parsing*
*Completed: 2026-03-12*

## Self-Check: PASSED
