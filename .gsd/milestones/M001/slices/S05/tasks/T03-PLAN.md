# T03: Plan 03

**Slice:** S05 — **Milestone:** M001

## Description

Wire decomposition into the CLI pipeline and add plan display output.

Purpose: Connect decompose_project_spec to the existing CLI flow (after parse_input, before future Phase 7 execution). Display the execution plan as a readable terminal table so users see what Athena plans to do.
Output: display_execution_plan terminal formatter, CLI wiring from ProjectSpec through decomposition to display.
