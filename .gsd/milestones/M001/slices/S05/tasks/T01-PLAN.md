# T01: Plan 01

**Slice:** S05 — **Milestone:** M001

## Description

Plan types, DAG algorithms, and error types for phase decomposition.

Purpose: Establish all data structures and pure graph algorithms that the LLM decomposition (Plan 02) and validation (Plan 02) will use. Pure logic with no LLM calls -- fully testable without mocks.
Output: ExecutionPlan/PhaseSpec/TaskSpec types in ath-types, topological sort + parallelism + critical path in ath-planner, DecomposeError type, extended ValidationError variants.
