# T02: Plan 02

**Slice:** S06 — **Milestone:** M001

## Description

Implement the agent router that assigns each task to exactly one agent using majority-vote skill tag matching with priority tiebreaking and circuit-breaker-aware fallback.

Purpose: Enables automatic task-to-agent routing so tasks are dispatched to the most capable agent without human intervention.
Output: router.rs with route_task, assign_all_tasks, and RoutingDecision -- fully tested.
