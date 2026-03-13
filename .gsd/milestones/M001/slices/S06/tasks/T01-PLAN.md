# T01: Plan 01

**Slice:** S06 — **Milestone:** M001

## Description

Create the foundation for module isolation: add the assigned_agent field to TaskSpec, define IsolationError types, and implement the static skill taxonomy routing table.

Purpose: Establishes the type contracts and skill-to-agent mapping that the router and isolation manager build on.
Output: taxonomy.rs with routing table + tests, error.rs with IsolationError, TaskSpec with assigned_agent field.
