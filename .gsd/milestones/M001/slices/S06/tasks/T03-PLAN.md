# T03: Plan 03

**Slice:** S06 — **Milestone:** M001

## Description

Implement the IsolationManager: pre-dispatch file ownership validation across parallel phases and post-execution audit comparing actual vs declared output files.

Purpose: Prevents two agents from writing to the same file in concurrent phases, catching conflicts before any LLM call is made. Post-audit warns about discrepancies without blocking.
Output: isolation.rs with check_isolation and audit_outputs -- fully tested.
