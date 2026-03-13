# S02: Agent Clients

**Goal:** Define the foundational contracts for the agent layer: the AgentError enum, CircuitBreaker state machine, AgentBackend trait, and MockBackend test double.
**Demo:** Define the foundational contracts for the agent layer: the AgentError enum, CircuitBreaker state machine, AgentBackend trait, and MockBackend test double.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Define the foundational contracts for the agent layer: the AgentError enum, CircuitBreaker state machine, AgentBackend trait, and MockBackend test double.

Purpose: These are the contracts that all provider actors implement against. They must exist and be tested before any real provider code is written. MockBackend enables testing orchestration logic (Phases 6-7) without real API calls.

Output: Four new source files in ath-agents with full unit test coverage.
- [x] **T02: Plan 02**
  - Implement the tokio actor infrastructure and all three provider actors (Claude, Gemini, Codex) with retry, backoff, and circuit breaker behavior.

Purpose: This is the core of ORCH-05 -- making Athena able to call real LLM APIs autonomously with production-grade reliability. The actor pattern isolates each provider so a slow/failing one does not block others.

Output: Actor module with shared infrastructure and three provider implementations, all implementing AgentBackend.
- [x] **T03: Plan 03**
  - Write integration tests proving the full agent layer works end-to-end: trait-based dispatch, retry with backoff, circuit breaker tripping, and provider handle lifecycle. Then verify the complete phase with a human checkpoint.

Purpose: These tests prove ORCH-05 is satisfied without requiring real API keys. They exercise the full stack (handle -> actor -> retry -> circuit breaker -> mock provider) through the AgentBackend trait interface.

Output: Integration test file and human verification of the complete agent layer.

## Files Likely Touched

