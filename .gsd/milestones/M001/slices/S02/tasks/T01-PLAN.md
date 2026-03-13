# T01: Plan 01

**Slice:** S02 — **Milestone:** M001

## Description

Define the foundational contracts for the agent layer: the AgentError enum, CircuitBreaker state machine, AgentBackend trait, and MockBackend test double.

Purpose: These are the contracts that all provider actors implement against. They must exist and be tested before any real provider code is written. MockBackend enables testing orchestration logic (Phases 6-7) without real API calls.

Output: Four new source files in ath-agents with full unit test coverage.
