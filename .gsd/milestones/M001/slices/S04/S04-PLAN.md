# S04: Input Parsing

**Goal:** Set up the input parsing infrastructure: InputMode enum, InputError type, CLI flag extensions, and AgentRequest json_schema field.
**Demo:** Set up the input parsing infrastructure: InputMode enum, InputError type, CLI flag extensions, and AgentRequest json_schema field.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Set up the input parsing infrastructure: InputMode enum, InputError type, CLI flag extensions, and AgentRequest json_schema field.

Purpose: Establish the type contracts and error hierarchy that all subsequent input parsing tasks build on.
Output: Compilable ath-planner input module skeleton, extended CLI flags, extended AgentRequest.
- [x] **T02: Plan 02**
  - Build the LLM parsing pipeline and spec file reader. Natural language and spec file inputs both flow through a shared parse_to_project_spec function that calls Claude with structured output (JsonSpec), validates the result, and retries with error feedback on failure.

Purpose: Deliver INPT-01 (natural language parsing) and INPT-02 (spec file parsing) as working features.
Output: parse_to_project_spec function, LLM prompt templates, spec file reader, call_provider JsonSpec threading.
- [x] **T03: Plan 03**
  - Build the codebase scanner and wire all three input modes through the CLI. Codebase analysis uses the `ignore` crate for gitignore-respecting traversal, identifies key files, and sends tree + contents to the LLM. The CLI is updated to run the full input parsing pipeline for all modes and display a ProjectSpec summary.

Purpose: Deliver INPT-03 (codebase analysis) and complete the full input parsing pipeline.
Output: Codebase scanner, full parse_input function, CLI integration with ProjectSpec summary display.
- [x] **T04: Plan 04**
  - Wire the full input parsing pipeline through the CLI so that `ath run` actually produces a ProjectSpec.

Purpose: Close the gap identified in 04-VERIFICATION.md where the CLI resolves the input mode but never calls `parse_input` or constructs a real AgentBackend. ROADMAP Success Criteria 1-3 for Phase 4 require the user to run `ath run` and get a ProjectSpec -- currently the CLI stops at mode resolution.

Output: A working CLI that constructs a ClaudeHandle from ConfigStore, calls parse_input with the resolved InputMode, and displays the resulting ProjectSpec summary.

## Files Likely Touched

