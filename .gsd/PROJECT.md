# Athena

## What This Is

Athena is a Rust CLI tool that acts as an AI orchestra conductor — it takes a software project idea (natural language, spec document, or existing codebase), analyzes it into smart development phases, and autonomously executes those phases by coordinating multiple AI agents working in parallel on isolated modules. Each phase goes through a cross-review quality gate before the next begins. Agents are configurable via `.ath/agents.toml` — built-in providers (Claude, Gemini, Codex) and any OpenAI-compatible endpoint (Ollama, Groq, Together, etc.) are supported.

## Core Value

Intelligent phase analysis — breaking any software project into well-structured, dependency-aware phases with correct agent assignments and parallelization. Without smart decomposition, multi-agent execution is chaos.

## Requirements

### Validated

- ✓ Analyze project input (natural language, spec doc, or existing codebase) into structured phases — v1.0 (M001)
- ✓ Perform dependency analysis to determine phase ordering and parallelizable work — v1.0 (M001)
- ✓ Map required skills per task and assign to appropriate AI agents — v1.0 (M001)
- ✓ Enforce module isolation so agents work on separate files/modules without conflicts — v1.0 (M001)
- ✓ Call Claude, Gemini, and Codex APIs autonomously to execute assigned tasks — v1.0 (M001)
- ✓ Write generated code to local git repo with commits per phase and agent metadata — v1.0 (M001)
- ✓ Cross-review between phases with review gate blocking progression — v1.0 (M001)
- ✓ Auto-retry on review failure with reviewer feedback (max 3 attempts) — v1.0 (M001)
- ✓ Structured final report with phase table, agent assignments, review outcomes, and cost tracking — v1.0 (M001)
- ✓ Accept user-provided API keys via environment variables or config file — v1.0 (M001)
- ✓ Real-time terminal progress showing current phase, active agent, task status — v1.0 (M001)
- ✓ Dry-run mode to preview plan without execution — v1.0 (M001)
- ✓ Parallel execution of independent phases via tokio JoinSet — v1.0 (M001)
- ✓ Actionable error messages distinguishing API errors, review failures, and schema violations — v1.0 (M001)
- ✓ Memory persistence across runs in `.ath/memory/` filesystem structure — M002
- ✓ Automatic context injection from prior runs into agent prompts — M002
- ✓ CLI inspection, search, and manual memory entry — M002
- ✓ Token budget enforcement for injected context — M002
- ✓ Keyword fallback when no embedding API is available — M002
- ✓ Resumable execution from last completed phase group — M003

### Out of Scope

- Web dashboard UI — CLI only for v1
- Built-in/managed API keys (SaaS model) — user provides their own
- PR-based workflow — direct commits to local repo
- Mobile app — CLI distribution only
- Real-time collaboration — single-user tool
- ~~Plugin system for custom agent definitions~~ — validated in M004
- ~~Support for local/self-hosted models (Ollama, etc.)~~ — validated in M004

## Context

Shipped v1.0 MVP with 16,315 lines of Rust across 7 crates (58 source files, 415 tests).
Milestone M001 (Migration) completed 2026-03-13 — all 10 slices delivered, all 14 requirements validated.
Milestone M002 (Memory Layer) completed 2026-03-14 — all 6 slices delivered, 5 requirements validated.
Milestone M003 (Resumable Execution) completed 2026-03-14 — all 3 slices delivered, 1 requirement validated.
Milestone M004 (Agent & Skill Plugin System) completed 2026-03-14 — all 5 slices delivered, 2 requirements validated.
Milestone M005 (Streaming Output) completed 2026-03-14 — 2 slices delivered. Streaming infrastructure end-to-end.
Milestone M006 (Multi-Turn Conversations) completed 2026-03-14 — 2 slices delivered. Conversation threading in retry loop. 679 tests passing.
Tech stack: Rust, tokio, clap, git2, genai, backon.
Extensible agent system: built-in providers (Anthropic/Claude, Google/Gemini, OpenAI/Codex) plus any OpenAI-compatible endpoint via GenericHandle.
Module isolation with strict file ownership prevents agent conflicts.
Cross-agent review gates ensure quality before phase progression.
Parallel execution via tokio JoinSet with isolation-gate pre-check.
Durable run reports persisted at `.ath/runs/<run-id>/report.json` with cost estimation.

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

## Milestones

| ID | Name | Status | Completed |
|----|------|--------|-----------|
| M001 | Migration | ✅ Complete | 2026-03-13 |
| M002 | Memory Layer | ✅ Complete | 2026-03-14 |
| M003 | Resumable Execution | ✅ Complete | 2026-03-14 |
| M004 | Agent & Skill Plugin System | ✅ Complete | 2026-03-14 |
| M005 | Streaming Output | ✅ Complete | 2026-03-14 |
| M006 | Multi-Turn Conversations | ✅ Complete | 2026-03-14 |

---
*Last updated: 2026-03-14 — M004 (Agent & Skill Plugin System) complete*
