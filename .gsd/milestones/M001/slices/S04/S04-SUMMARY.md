---
id: S04
parent: M001
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
# S04: Input Parsing

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

# Phase 4 Plan 02: LLM Parsing Pipeline Summary

**parse_to_project_spec with 3-attempt retry, LLM prompt templates for all input modes, spec file reader with 50KB cap, and json_schema threading to genai JsonSpec**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T14:33:19Z
- **Completed:** 2026-03-12T14:37:25Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- parse_to_project_spec function with retry-on-error-feedback loop (3 attempts max) using request_builder closure pattern
- System prompts and request builders for natural language, spec file, and codebase input modes with json_schema for structured output
- Spec file reader with metadata-based 50KB size check before reading into memory
- call_provider extended with json_schema parameter, builds ChatOptions::JsonSpec when provided
- 22 ath-planner tests + 180 total workspace tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: LLM prompt templates, spec file reader, and parse_to_project_spec with retry** - `b8a8b22` (feat)
2. **Task 2: Thread json_schema through call_provider to genai ChatOptions** - `ef33395` (feat)

## Files Created/Modified
- `crates/ath-planner/src/input/prompt.rs` - System prompts, JSON schema, and request builders for all 3 input modes
- `crates/ath-planner/src/input/spec_file.rs` - Async spec file reader with 50KB size cap enforcement
- `crates/ath-planner/src/input/mod.rs` - parse_to_project_spec core function with retry logic, pub mod declarations
- `crates/ath-planner/Cargo.toml` - Added chrono, uuid, tokio/fs features; tempfile dev-dep
- `crates/ath-agents/src/actor/mod.rs` - call_provider json_schema param, ChatOptions::JsonSpec construction

## Decisions Made
- Used request_builder closure pattern (`Fn(Option<&str>) -> AgentRequest`) so all input modes share the same retry logic without code duplication
- json_schema is threaded through run_with_retry_and_breaker to call_provider rather than bypassing the actor pattern, keeping the existing architecture intact

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- parse_to_project_spec ready for integration with CLI flow
- Codebase scanner (Plan 04-03) can use build_codebase_request with parse_to_project_spec
- json_schema flows end-to-end from AgentRequest through actors to genai provider
- All 180 workspace tests pass

---
*Phase: 04-input-parsing*
*Completed: 2026-03-12*

## Self-Check: PASSED

# Phase 4 Plan 03: Codebase Scanner and CLI Integration Summary

**Codebase scanner with gitignore-aware traversal, key file extraction with size caps, and full input parsing pipeline wired through CLI**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T14:41:30Z
- **Completed:** 2026-03-12T14:46:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Codebase scanner using ignore crate WalkBuilder for gitignore-respecting traversal with SKIP_DIRS filtering
- Key file identification via filename heuristics (manifests, READMEs) and entry point patterns (src/main.rs, etc.)
- Per-file 5KB and total 30KB content caps to prevent token explosion when sending to LLM
- parse_input high-level function dispatching NaturalLanguage, SpecFile, and Codebase modes
- display_project_spec_summary for colored terminal output of ProjectSpec
- CLI updated to resolve input mode from flags with confirmation message
- 33 ath-planner tests + 191 total workspace tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Codebase scanner with gitignore-aware traversal and key file extraction** - `6c1462f` (feat)
2. **Task 2: Wire all three input modes through CLI with ProjectSpec summary display** - `9e8fa71` (feat)

## Files Created/Modified
- `crates/ath-planner/src/input/codebase.rs` - Codebase scanner with scan_codebase, is_key_file, size caps, and 11 tests
- `crates/ath-planner/src/input/mod.rs` - parse_input dispatcher, display_project_spec_summary, codebase module declaration
- `crates/ath-planner/Cargo.toml` - Added colored dependency for terminal formatting
- `crates/ath-cli/src/main.rs` - Updated Run command to use resolve_input_mode with display_project_spec_summary import

## Decisions Made
- scan_codebase is sync (uses std::fs for file I/O), called from async context via tokio::task::spawn_blocking to avoid blocking the runtime
- Key file detection uses two strategies: filename matching (KEY_FILE_NAMES) for manifests/configs and relative path matching (KEY_ENTRY_PATTERNS) for entry points

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Full input parsing pipeline complete: natural language, spec file, and codebase modes all wired
- parse_input ready for integration with real AgentBackend in Phase 7 orchestrator
- Codebase scanner produces tree + key files for LLM consumption via build_codebase_request
- All 191 workspace tests pass

---
*Phase: 04-input-parsing*
*Completed: 2026-03-12*

## Self-Check: PASSED

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
