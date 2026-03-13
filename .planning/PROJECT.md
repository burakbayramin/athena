# Athena

## What This Is

Athena is a Rust CLI tool that acts as an AI orchestra conductor — it takes a software project idea (natural language, spec document, or existing codebase), analyzes it into smart development phases, and autonomously executes those phases by coordinating three AI agents (Claude, Gemini, Codex/Copilot) working in parallel on isolated modules. Each phase goes through a cross-review quality gate before the next begins.

## Core Value

Intelligent phase analysis — breaking any software project into well-structured, dependency-aware phases with correct agent assignments and parallelization. Without smart decomposition, multi-agent execution is chaos.

## Requirements

### Validated

- ✓ Analyze project input (natural language, spec doc, or existing codebase) into structured phases — v1.0
- ✓ Perform dependency analysis to determine phase ordering and parallelizable work — v1.0
- ✓ Map required skills per task and assign to appropriate AI agents — v1.0
- ✓ Enforce module isolation so agents work on separate files/modules without conflicts — v1.0
- ✓ Call Claude, Gemini, and Codex APIs autonomously to execute assigned tasks — v1.0
- ✓ Write generated code to local git repo with commits per phase and agent metadata — v1.0
- ✓ Cross-review between phases with review gate blocking progression — v1.0
- ✓ Auto-retry on review failure with reviewer feedback (max 3 attempts) — v1.0
- ✓ Structured final report with phase table, agent assignments, review outcomes, and cost tracking — v1.0
- ✓ Accept user-provided API keys via environment variables or config file — v1.0
- ✓ Real-time terminal progress showing current phase, active agent, task status — v1.0
- ✓ Dry-run mode to preview plan without execution — v1.0
- ✓ Parallel execution of independent phases via tokio JoinSet — v1.0
- ✓ Actionable error messages distinguishing API errors, review failures, and schema violations — v1.0

### Active

(No active requirements — define next milestone with `/gsd:new-milestone`)

### Out of Scope

- Web dashboard UI — CLI only for v1
- Built-in/managed API keys (SaaS model) — user provides their own
- PR-based workflow — direct commits to local repo
- Mobile app — CLI distribution only
- Real-time collaboration — single-user tool
- Resumable execution from last completed phase — v2 scope
- Plugin system for custom agent definitions — v2 scope
- Support for local/self-hosted models (Ollama, etc.) — v2 scope

## Context

Shipped v1.0 MVP with 16,315 lines of Rust across 7 crates.
Tech stack: Rust, tokio, clap, git2, genai, backon, indicatif, tracing.
Three AI backends: Anthropic (Claude), Google (Gemini), OpenAI (Codex/Copilot).
Module isolation with strict file ownership prevents agent conflicts.
Cross-agent review gates ensure quality before phase progression.
Parallel execution via tokio JoinSet with isolation-gate pre-check.

## Constraints

- **Runtime**: Rust — compiled, cross-platform binary
- **Auth**: User-owned API keys only — no key management service
- **Isolation**: Strict module boundaries per agent — no file-level merge conflicts
- **Execution**: Fully autonomous with review gates between phases — no human intervention during phase execution

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for CLI | Performance, single binary, strong types for complex orchestration | ✓ Good — type system caught many orchestration bugs at compile time |
| Module isolation over merge-based | Eliminates conflict resolution complexity; agents work in parallel safely | ✓ Good — clean parallel execution with no file conflicts |
| Cross-review over self-review | Catches blind spots — different model perspectives improve quality | ✓ Good — different agent perspectives catch issues |
| Phase analysis as core value | Multi-agent execution is useless without intelligent decomposition | ✓ Good — DAG-based decomposition is the foundation |
| User-owned API keys | Simpler v1, no billing/auth infrastructure needed | ✓ Good — no SaaS infrastructure overhead |
| Typestate pattern for PhaseRunner | Compile-time enforcement of valid state transitions | ✓ Good — invalid transitions impossible |
| Arc<AgentRegistry> for parallel dispatch | Safe sharing across spawned tokio tasks | ✓ Good — clean concurrent access |
| CircuitBreaker per provider | Fail-fast on unhealthy providers without cascading failures | ✓ Good — prevents retry storms |

---
*Last updated: 2026-03-13 after v1.0 milestone*
