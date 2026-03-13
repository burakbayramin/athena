# M002: Memory Layer

**Vision:** Athena accumulates knowledge across runs and automatically enriches agent prompts with relevant context — making each run smarter than the last.

## Success Criteria

- Agent prompts contain automatically injected context from prior runs
- Memory persists across runs in `.ath/memory/` filesystem structure
- User can inspect, search, and manually add memory entries via CLI
- Token budget is respected — injected context stays within configured limits
- System works without embedding API (keyword fallback)

## Key Risks / Unknowns

- **Pure Rust vector search at Athena's scale** — Need to validate that a pure Rust HNSW library can persist indexes, handle hundreds of entries, and return results in <100ms
- **LLM extraction quality** — Post-run extraction may produce noisy or inconsistent memory entries; structured prompts and JSON schema validation needed
- **Integration complexity** — Hooking observation capture into the existing orchestrator without breaking the typestate machine or parallel dispatch

## Proof Strategy

- Pure Rust vector search → retire in S01 by proving Viking store + vector index read/write/search works end-to-end with persisted data
- LLM extraction quality → retire in S03 by proving extraction pipeline produces valid, parseable memory entries from real observation data
- Integration complexity → retire in S04 by proving observation capture works inside a full `run_phase` cycle without breaking existing tests

## Verification Classes

- Contract verification: unit tests for store/index/extraction/injection, integration tests with fixtures
- Integration verification: two-run scenario where second run's prompts contain first run's context
- Operational verification: memory persists across process restarts, CLI commands work on real data
- UAT / human verification: inspect injected context quality in verbose mode

## Milestone Definition of Done

This milestone is complete only when all are true:

- All slice deliverables are complete and tests pass
- Viking store, observation system, extraction, and injection are wired together
- `ath memory` CLI commands work on real memory data
- A two-run integration test proves end-to-end memory flow
- Token budgets are enforced and configurable
- Existing test suite still passes (no regressions)

## Requirement Coverage

- Covers: Memory persistence, automatic context injection, CLI inspection, keyword fallback
- Partially covers: none
- Leaves for later: Cross-project memory, TUI editor, memory diffing, local embedding models
- Orphan risks: none

## Slices

- [x] **S01: Viking Store & Vector Index** `risk:high` `depends:[]`
  > After this: `ath-memory` crate exists with Viking URI parsing, L0/L1/L2 read/write, and vector search — all proven by unit tests with persisted data on disk

- [x] **S02: Observation System** `risk:medium` `depends:[]`
  > After this: Observation types, append-only JSONL writer, and observation buffer are implemented — proven by unit tests that capture and serialize observation events

- [x] **S03: Memory Extraction Pipeline** `risk:high` `depends:[S01,S02]`
  > After this: Post-run extraction reads observations, calls LLM to summarize, writes structured entries to Viking store, and updates vector index — proven by integration test with fixture observations

- [x] **S04: Orchestrator Integration (Observation + Injection)** `risk:high` `depends:[S01,S02,S03]`
  > After this: `run_phase` captures observations automatically, context injector reads memory and enriches agent prompts within token budget — proven by integration test where a phase run produces observations and a subsequent call gets injected context

- [x] **S05: CLI & Config** `risk:low` `depends:[S01,S02,S03]`
  > After this: `ath memory tree|search|read|add|stats|gc` subcommands work, memory config is loaded from `.ath/memory/config.toml` — proven by CLI integration tests

- [ ] **S06: End-to-End Integration** `risk:medium` `depends:[S04,S05]`
  > After this: Full two-run scenario works — first run captures observations and extracts memory, second run's agent prompts contain injected context, CLI commands show accumulated memory — proven by end-to-end integration test

## Boundary Map

### S01 → S03

Produces:
- `VikingStore` trait with `read(uri) -> LayeredContent` and `write(uri, content)` methods
- `MemoryIndex` with `upsert(uri, content)` and `search(query, top_k) -> Vec<MemoryHit>` methods
- `VikingUri` parser for `viking://` paths
- `LayeredContent` struct with `abstract_text`, `overview_text`, `detail` fields

Consumes:
- nothing (first slice)

### S02 → S03

Produces:
- `ObservationType` enum covering agent interactions, review events, file ops, errors, decisions
- `ObservationBuffer` that accepts events and flushes to JSONL
- `ObservationStorage` for reading back serialized observations

Consumes:
- nothing (independent slice)

### S01 + S02 → S03

Produces:
- `MemoryExtractor` that reads observations, calls LLM, writes to Viking store, updates index
- Extraction prompts for run summary, convention detection, decision extraction, agent profile updates

Consumes:
- `VikingStore` + `MemoryIndex` from S01
- `ObservationType` + `ObservationStorage` from S02

### S01 + S03 → S04

Produces:
- `ContextInjector` that builds enriched prompts within token budget
- Observation hooks integrated into `phase_runner.rs` and `coordinator.rs`
- Modified `execute_phase_tasks` that injects memory context into agent requests

Consumes:
- `VikingStore` + `MemoryIndex` from S01
- `MemoryExtractor` from S03

### S01 + S02 + S03 → S05

Produces:
- `ath memory` CLI subcommand group (tree, search, read, add, stats, gc)
- `config.toml` schema and loading for memory configuration

Consumes:
- `VikingStore` + `MemoryIndex` from S01
- `ObservationStorage` from S02
- `MemoryExtractor` from S03 (for stats)

### S04 + S05 → S06

Produces:
- End-to-end integration test proving the full memory lifecycle
- Any final wiring needed between observation capture, extraction, injection, and CLI

Consumes:
- Everything from S01–S05
