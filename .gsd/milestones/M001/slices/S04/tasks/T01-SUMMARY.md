---
id: T01
parent: S04
milestone: M001
provides: []
requires: []
affects: []
key_files: []
key_decisions: []
patterns_established: []
observability_surfaces: []
drill_down_paths: []
duration: 
verification_result: passed
completed_at: 
blocker_discovered: false
---
# T01: Plan 01

**# Phase 4 Plan 01: Input Parsing Infrastructure Summary**

## What Happened

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
