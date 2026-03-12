# Athena

## What This Is

Athena is a Rust CLI tool that acts as an AI orchestra conductor — it takes a software project idea (natural language, spec document, or existing codebase), analyzes it into smart development phases, and autonomously executes those phases by coordinating three AI agents (Claude, Gemini, Codex/Copilot) working in parallel on isolated modules. Each phase goes through a cross-review quality gate before the next begins.

## Core Value

Intelligent phase analysis — breaking any software project into well-structured, dependency-aware phases with correct agent assignments and parallelization. Without smart decomposition, multi-agent execution is chaos.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Analyze project input (natural language, spec doc, or existing codebase) into structured phases
- [ ] Perform dependency analysis to determine phase ordering and parallelizable work
- [ ] Map required skills per task (Python, React, Security, DevOps, etc.)
- [ ] Assign tasks to appropriate AI agents based on their strengths (Claude: architecture/logic, Gemini: research/docs/APIs, Codex: code generation/boilerplate)
- [ ] Enforce module isolation so agents work on separate files/modules without conflicts
- [ ] Call Claude, Gemini, and Codex APIs autonomously to execute assigned tasks
- [ ] Write generated code directly to a local git repo with commits per phase
- [ ] Cross-review between phases — each agent's output reviewed by a different agent
- [ ] Block phase progression if review fails; loop until issues resolved
- [ ] Produce structured report: project summary, phase table (phase/task/dependency/skill/agent/parallelism), review checklist
- [ ] Accept user-provided API keys via environment variables or config file
- [ ] Support any software project type (web apps, CLIs, APIs, mobile — language/framework agnostic)

### Out of Scope

- Web dashboard UI — CLI only for v1
- Built-in/managed API keys (SaaS model) — user provides their own
- PR-based workflow — direct commits to local repo
- Mobile app — CLI distribution only
- Real-time collaboration — single-user tool

## Context

- Built in Rust for performance, single-binary distribution, and strong type system
- Three AI backends: Anthropic (Claude), Google (Gemini), OpenAI (Codex/Copilot)
- Module isolation strategy: each agent owns specific files/modules with strict boundaries, no shared edits
- Cross-review model: Agent A's output is reviewed by Agent B (not self-review)
- The "orchestra conductor" metaphor is central — Athena coordinates, agents perform
- Phase analysis is the foundation — if decomposition is wrong, everything downstream fails

## Constraints

- **Runtime**: Rust — compiled, cross-platform binary
- **Auth**: User-owned API keys only — no key management service
- **Isolation**: Strict module boundaries per agent — no file-level merge conflicts
- **Execution**: Fully autonomous with review gates between phases — no human intervention during phase execution

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for CLI | Performance, single binary, strong types for complex orchestration | — Pending |
| Module isolation over merge-based | Eliminates conflict resolution complexity; agents work in parallel safely | — Pending |
| Cross-review over self-review | Catches blind spots — different model perspectives improve quality | — Pending |
| Phase analysis as core value | Multi-agent execution is useless without intelligent decomposition | — Pending |
| User-owned API keys | Simpler v1, no billing/auth infrastructure needed | — Pending |

---
*Last updated: 2026-03-12 after initialization*
