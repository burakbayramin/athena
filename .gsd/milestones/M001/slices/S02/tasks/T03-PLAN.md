# T03: Plan 03

**Slice:** S02 — **Milestone:** M001

## Description

Write integration tests proving the full agent layer works end-to-end: trait-based dispatch, retry with backoff, circuit breaker tripping, and provider handle lifecycle. Then verify the complete phase with a human checkpoint.

Purpose: These tests prove ORCH-05 is satisfied without requiring real API keys. They exercise the full stack (handle -> actor -> retry -> circuit breaker -> mock provider) through the AgentBackend trait interface.

Output: Integration test file and human verification of the complete agent layer.
