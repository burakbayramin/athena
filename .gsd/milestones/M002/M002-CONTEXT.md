# M002: Memory Layer — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Add a persistent, structured, searchable memory system (`ath-memory` crate) to Athena so that AI agents accumulate knowledge across runs — project conventions, agent performance patterns, user preferences, and architectural decisions — and automatically inject relevant context into agent prompts.

## Why This Milestone

Every Athena run currently starts from zero. Agents re-discover project conventions, repeat mistakes that were already caught by reviewers, and ignore patterns that worked well in previous runs. Memory makes each subsequent run smarter: fewer review retries, better agent routing, higher first-pass code quality.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Run `ath run` and see that agent prompts are automatically enriched with relevant context from prior runs (project conventions, past decisions, agent notes)
- Run `ath memory tree` to browse the accumulated memory structure
- Run `ath memory search "auth"` to find relevant memory entries by semantic or keyword search
- Run `ath memory read viking://project/conventions/error-handling.md` to read a specific memory entry
- Run `ath memory add viking://user/instructions/prefer-axum.md "Always use Axum for HTTP"` to manually add standing instructions
- Run `ath memory stats` to see memory size, entry count, and index health

### Entry point / environment

- Entry point: `ath run` (automatic injection), `ath memory` subcommands (manual inspection)
- Environment: local dev — CLI binary
- Live dependencies involved: Embedding API (OpenAI text-embedding-3-small or equivalent) for vector indexing; existing LLM APIs for post-run extraction

## Completion Class

- Contract complete means: Viking store reads/writes, observation capture, extraction pipeline, context injection, and retrieval all pass unit + integration tests with fixtures
- Integration complete means: A full `ath run` captures observations, extracts memory post-run, and the next run's agent prompts contain injected context from the first run
- Operational complete means: Memory persists across runs, CLI inspection commands work, token budgets are respected

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A two-run scenario: first run completes and extracts memory; second run's agent prompts include injected context from the first run's discoveries
- `ath memory` subcommands (tree, search, read, add, stats) work correctly on real memory data
- Token budget is respected — injected context stays within configured limits

## Risks and Unknowns

- **Zvec has no Windows support and heavy C++ build dependency** — Need a pure Rust vector search alternative or hybrid approach. Athena targets cross-platform.
- **Embedding API dependency** — Memory indexing requires an embedding API call. Need to handle cases where no embedding API is configured (graceful degradation to keyword search).
- **Extraction quality** — LLM-powered post-run extraction may produce inconsistent or noisy results. Need structured prompts and validation.
- **Token budget management** — Injecting too much context wastes tokens; too little defeats the purpose. Need empirical tuning.

## Existing Codebase / Prior Art

- `crates/ath-orchestrator/src/coordinator.rs` — Integration point for post-run extraction (`post_run`) and pre-agent-call context injection
- `crates/ath-orchestrator/src/phase_runner.rs` — Integration point for observation hooks around `execute_phase_tasks` and review cycles
- `crates/ath-types/src/` — Shared types; memory types will live in `ath-types` or `ath-memory`
- `crates/ath-cli/src/main.rs` — CLI entry point where `ath memory` subcommands will be added
- `crates/ath-config/` — Config loading; memory config will extend existing patterns
- `docs/specs/memory-layer.md` — Full technical specification

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions — it is an append-only register; read it during planning, append to it during execution.

## Relevant Requirements

- Memory persistence across runs — new requirement for M002
- Automatic context injection into agent prompts — new requirement for M002
- CLI memory inspection commands — new requirement for M002
- Graceful degradation without embedding API — new requirement for M002

## Scope

### In Scope

- `ath-memory` crate with Viking store, observation system, extraction pipeline, context injector
- Vector index for semantic search (pure Rust, no C++ dependencies)
- CLI subcommands: `ath memory tree|search|read|add|stats|gc`
- Integration with `ath-orchestrator` for observation capture and context injection
- Token budget management for context injection
- Post-run memory extraction via LLM
- `.ath/memory/` filesystem structure

### Out of Scope / Non-Goals

- Cross-project memory sharing (v3 scope)
- Interactive TUI memory editor (v3 scope)
- Memory diffing between runs (v3 scope)
- Retrieval trajectory visualization (v3 scope)
- Local/self-hosted embedding models (v3 scope)

## Technical Constraints

- Pure Rust — no C/C++ build dependencies for vector search (cross-platform binary target)
- Memory stored in `.ath/memory/` under the project directory
- Embedding API is optional — system must work with keyword/FTS fallback when no embedding key is configured
- Token budgets must be configurable via `.ath/memory/config.toml`

## Integration Points

- `ath-orchestrator` — observation hooks in phase_runner, context injection in execute_phase_tasks, post-run extraction in coordinator
- `ath-agents` — AgentRequest gets optional `context` field populated by injector
- `ath-cli` — new `memory` subcommand group
- `ath-config` — memory configuration loading
- `ath-types` — shared memory types if needed across crates

## Open Questions

- **Vector search library**: hnswlib-rs, hora, or hnsw (pure Rust)? Need to evaluate API ergonomics, index persistence, and search quality at Athena's scale (hundreds to low thousands of entries).
- **Embedding API config**: Same API key pool as agent LLMs, or separate `ATHENA_EMBEDDING_API_KEY`? Leaning toward reusing existing keys with a configurable model override.
- **Memory size limits**: Auto-compact after N runs? After M megabytes? Need sensible defaults.
