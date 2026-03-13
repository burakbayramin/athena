# S06: Module Isolation

**Goal:** Create the foundation for module isolation: add the assigned_agent field to TaskSpec, define IsolationError types, and implement the static skill taxonomy routing table.
**Demo:** Create the foundation for module isolation: add the assigned_agent field to TaskSpec, define IsolationError types, and implement the static skill taxonomy routing table.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Create the foundation for module isolation: add the assigned_agent field to TaskSpec, define IsolationError types, and implement the static skill taxonomy routing table.

Purpose: Establishes the type contracts and skill-to-agent mapping that the router and isolation manager build on.
Output: taxonomy.rs with routing table + tests, error.rs with IsolationError, TaskSpec with assigned_agent field.
- [x] **T02: Plan 02**
  - Implement the agent router that assigns each task to exactly one agent using majority-vote skill tag matching with priority tiebreaking and circuit-breaker-aware fallback.

Purpose: Enables automatic task-to-agent routing so tasks are dispatched to the most capable agent without human intervention.
Output: router.rs with route_task, assign_all_tasks, and RoutingDecision -- fully tested.
- [x] **T03: Plan 03**
  - Implement the IsolationManager: pre-dispatch file ownership validation across parallel phases and post-execution audit comparing actual vs declared output files.

Purpose: Prevents two agents from writing to the same file in concurrent phases, catching conflicts before any LLM call is made. Post-audit warns about discrepancies without blocking.
Output: isolation.rs with check_isolation and audit_outputs -- fully tested.

## Files Likely Touched

