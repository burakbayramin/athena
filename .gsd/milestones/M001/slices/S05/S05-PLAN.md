# S05: Phase Decomposition

**Goal:** Plan types, DAG algorithms, and error types for phase decomposition.
**Demo:** Plan types, DAG algorithms, and error types for phase decomposition.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Plan types, DAG algorithms, and error types for phase decomposition.

Purpose: Establish all data structures and pure graph algorithms that the LLM decomposition (Plan 02) and validation (Plan 02) will use. Pure logic with no LLM calls -- fully testable without mocks.
Output: ExecutionPlan/PhaseSpec/TaskSpec types in ath-types, topological sort + parallelism + critical path in ath-planner, DecomposeError type, extended ValidationError variants.
- [x] **T02: Plan 02**
  - DAG validation, LLM decomposition prompt, and retry loop for phase decomposition.

Purpose: The core decomposition pipeline -- send ProjectSpec to Claude, parse structured response, validate the DAG, retry with feedback on failure, assemble the final ExecutionPlan with computed fields.
Output: validate_plan (hard errors + warnings), decomposition prompt/schema, decompose_project_spec entry point with 3-attempt retry.
- [x] **T03: Plan 03**
  - Wire decomposition into the CLI pipeline and add plan display output.

Purpose: Connect decompose_project_spec to the existing CLI flow (after parse_input, before future Phase 7 execution). Display the execution plan as a readable terminal table so users see what Athena plans to do.
Output: display_execution_plan terminal formatter, CLI wiring from ProjectSpec through decomposition to display.

## Files Likely Touched

