# T02: Plan 02

**Slice:** S05 — **Milestone:** M001

## Description

DAG validation, LLM decomposition prompt, and retry loop for phase decomposition.

Purpose: The core decomposition pipeline -- send ProjectSpec to Claude, parse structured response, validate the DAG, retry with feedback on failure, assemble the final ExecutionPlan with computed fields.
Output: validate_plan (hard errors + warnings), decomposition prompt/schema, decompose_project_spec entry point with 3-attempt retry.
