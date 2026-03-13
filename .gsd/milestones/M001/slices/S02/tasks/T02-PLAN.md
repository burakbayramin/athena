# T02: Plan 02

**Slice:** S02 — **Milestone:** M001

## Description

Implement the tokio actor infrastructure and all three provider actors (Claude, Gemini, Codex) with retry, backoff, and circuit breaker behavior.

Purpose: This is the core of ORCH-05 -- making Athena able to call real LLM APIs autonomously with production-grade reliability. The actor pattern isolates each provider so a slow/failing one does not block others.

Output: Actor module with shared infrastructure and three provider implementations, all implementing AgentBackend.
