---
id: M002
provides:
  - ath-memory crate with Viking store, vector index (HNSW), keyword index, observation system, extraction pipeline, context injector, and TOML config
  - Memory-aware orchestrator integration (observation capture, context injection, post-run extraction) — all fail-soft
  - ath memory CLI subcommands (tree, search, read, add, stats, gc)
  - Two-run end-to-end memory lifecycle proven by integration test
key_decisions:
  - "D006: hnsw crate (v0.11) over hnswlib-rs — pure Rust, cross-platform (hnswlib-rs depends on Unix-only off64)"
  - "D007: HNSW graph rebuilt from stored vectors on load — RNG type not serde-compatible"
  - "D008: File-per-run JSONL observations — maps to per-run extraction"
  - "D010: ExtractionLlm trait in ath-memory, not ath-agents — keeps memory crate provider-agnostic"
  - "D012: extract_all isolates per-stage errors — failed stages don't block others"
  - "D013: Memory-aware code in memory.rs — phase_runner.rs untouched, zero risk to existing tests"
  - "D015: XML wrapper format for injected context — structured, parseable by agents"
  - "D016: Token estimation via word_count * 4 / 3 heuristic — no tokenizer dependency"
patterns_established:
  - VikingUri as canonical memory URI type with FromStr/Display/Serialize/Deserialize
  - LayeredContent L0/L1/L2 markdown format with YAML frontmatter
  - MemoryError with hint() for actionable diagnostics (matches ath-config pattern)
  - Fail-soft memory wrappers — observation and injection never crash the run
  - BackendLlmAdapter bridging AgentBackend → ExtractionLlm for provider-agnostic extraction
  - MemoryContext as opt-in bundle threaded into memory-aware execution
  - Config load with graceful defaults on missing/malformed files
observability_surfaces:
  - ".ath/memory/store/<segments>.md — plain markdown, directly inspectable"
  - ".ath/memory/observations/<run-uuid>.jsonl — grep-able JSONL with type tags"
  - "keyword.json at index_path — presence confirms persistence success"
  - "ath memory stats — primary health-check command"
  - "MemoryError::hint() surfaced in CLI error display"
  - "tracing::warn on all memory failures (grep 'Memory operation failed', 'Extraction stage failed', 'keyword index save failed')"
requirement_outcomes:
  - id: REQ-MEM-PERSIST
    from_status: active
    to_status: validated
    proof: "VikingStore persists LayeredContent as markdown in .ath/memory/store/. Two-run e2e test proves data survives across runs. 14 store CRUD tests + keyword_index_persisted_after_extraction test."
  - id: REQ-MEM-INJECT
    from_status: active
    to_status: validated
    proof: "ContextInjector reads store + keyword index, assembles budget-constrained XML context. two_run_end_to_end_memory_lifecycle test proves Run 2 prompts contain Run 1 content. 13 injector unit tests + memory_context_injects_context_from_store integration test."
  - id: REQ-MEM-CLI
    from_status: active
    to_status: validated
    proof: "6 subcommands (tree, search, read, add, stats, gc) all pass 21 CLI tests covering happy path, empty state, and error cases."
  - id: REQ-MEM-BUDGET
    from_status: active
    to_status: validated
    proof: "InjectionConfig with per-section and total token budgets. ContextInjector enforces limits via word_count * 4/3 estimation. 13 unit tests including budget overflow scenarios."
  - id: REQ-MEM-KEYWORD
    from_status: active
    to_status: validated
    proof: "KeywordIndex with HashMap-based inverted index and TF scoring. 7 keyword tests. Extraction pipeline writes to keyword index. System fully functional without embedding API."
duration: ~4h
verification_result: passed
completed_at: 2026-03-14
---

# M002: Memory Layer

**Persistent memory subsystem that accumulates knowledge across runs — Viking store, keyword search, observation capture, LLM extraction, budget-constrained context injection, and CLI inspection — proven by 588 passing tests including a two-run end-to-end lifecycle test.**

## What Happened

Six slices built the memory layer from storage primitives to full orchestrator integration:

**S01 (Viking Store & Vector Index)** established the `ath-memory` crate with three storage engines: `VikingStore` for markdown-backed layered content (L0 abstract / L1 overview / L2 detail), `MemoryIndex` wrapping a pure-Rust HNSW graph for vector search, and `KeywordIndex` with TF-scored inverted index for embedding-free fallback. All persist to disk — store as human-readable markdown, indexes as JSON. Pivoted from `hnswlib-rs` to the `hnsw` crate after discovering Unix-only dependencies. 54 tests + 1 doc-test.

**S02 (Observation System)** added typed event capture: 7 `ObservationType` variants covering the full agent interaction surface, append-only JSONL persistence per run, and a thread-safe `ObservationBuffer` with cap-enforced eviction. 27 tests.

**S03 (Memory Extraction Pipeline)** built the LLM-powered post-run analysis: four extraction stages (run summary, conventions, decisions, agent profiles) each following a prompt → LLM → parse → merge-or-write → index pipeline. Per-stage error isolation means one bad LLM response doesn't block other stages. Provider-agnostic via the `ExtractionLlm` trait defined in ath-memory. 37 tests with `MockExtractionLlm`.

**S04 (Orchestrator Integration)** wired everything together without touching existing code. `ContextInjector` assembles budget-constrained XML context from store + keyword index. `MemoryContext` bundles all memory state for opt-in threading through `run_plan_with_memory`. `BackendLlmAdapter` bridges the existing `AgentBackend` trait to `ExtractionLlm` for extraction. All memory operations are fail-soft — observations and injection never crash a run. 13 orchestrator integration tests added alongside 135 existing tests (all passing).

**S05 (CLI & Config)** delivered 6 `ath memory` subcommands (tree, search, read, add, stats, gc) and TOML-based configuration with graceful defaults. `MemoryConfig` wraps injection, extraction, and GC settings. All subcommands handle empty/missing state with informative messages. 21 CLI tests + 7 config tests.

**S06 (End-to-End Integration)** closed the loop: fixed keyword index persistence after extraction, then proved the complete lifecycle with a two-run integration test — Run 1 captures observations and extracts memory; Run 2's agent prompts contain injected context from Run 1.

## Cross-Slice Verification

| Success Criterion | Evidence |
|---|---|
| Agent prompts contain automatically injected context from prior runs | `two_run_end_to_end_memory_lifecycle` test asserts Run 2's `AgentRequest.context` contains `<athena_context>` with `<recent_run>` referencing Run 1 content |
| Memory persists across runs in `.ath/memory/` filesystem structure | Two-run test uses shared temp dir with fresh store/index instances per run. `keyword_index_persisted_after_extraction` test verifies index file on disk. 14 store CRUD tests verify markdown persistence |
| User can inspect, search, and manually add memory entries via CLI | 21 CLI tests prove all 6 subcommands: `tree` (2 tests), `search` (3 tests), `read` (3 tests), `add` (2 tests), `stats` (2 tests), `gc` (4 tests) + error display tests |
| Token budget is respected — injected context stays within configured limits | 13 `ContextInjector` unit tests enforce per-section and total token budgets. `InjectionConfig` defaults: total=4000, project=200, semantic=2500, agent=300, run=500 |
| System works without embedding API (keyword fallback) | Entire test suite runs without any embedding API. `KeywordIndex` with TF scoring (7 tests). Extraction pipeline writes to keyword index. Context injection queries keyword index |
| Viking store, observation system, extraction, and injection are wired together | Two-run e2e test exercises full chain: observation capture → JSONL write → extraction → store write → keyword index update → context injection |
| `ath memory` CLI commands work on real memory data | CLI tests create real store/index data in temp dirs and verify subcommand output |
| Existing test suite still passes (no regressions) | `cargo test --workspace` — 588 passed, 0 failed. All 135 pre-existing orchestrator tests pass unchanged |

**Workspace verification:** `cargo test --workspace` — 588 tests passed, 0 failed. `cargo check --workspace` — clean (3 pre-existing dead_code warnings only).

## Requirement Changes

- REQ-MEM-PERSIST: active → validated — VikingStore markdown persistence + two-run e2e test proving cross-run survival
- REQ-MEM-INJECT: active → validated — ContextInjector + two-run e2e test proving Run 2 prompts contain Run 1 context
- REQ-MEM-CLI: active → validated — 6 subcommands with 21 passing tests
- REQ-MEM-BUDGET: active → validated — InjectionConfig with configurable limits, enforced by ContextInjector
- REQ-MEM-KEYWORD: active → validated — KeywordIndex fallback, full system functional without embedding API

## Forward Intelligence

### What the next milestone should know
- `MemoryContext` is the single entry point for memory-aware orchestration — construct with store, index, buffer, and configs. The `run_plan_with_memory` function mirrors `run_plan_with_progress` with memory hooks added.
- `memory.rs` in `ath-orchestrator` duplicates the phase execution loop from `phase_runner.rs`. If phase_runner changes, memory.rs must be updated in lockstep. Consider extracting shared helpers.
- All memory operations are fail-soft. This is intentional — memory is enrichment, not correctness. But it means extraction failures are silent unless you check logs or `ExtractionResult.failed`.
- The `ExtractionLlm` trait is the LLM boundary in ath-memory. Real implementation goes through `BackendLlmAdapter` which selects Claude > Gemini > Codex from the agent registry.

### What's fragile
- **memory.rs duplication** — mirrors phase_runner.rs execution loop. Any change to phase execution flow requires parallel updates.
- **Convention/agent profile merge is append-only** — files grow monotonically across runs with no deduplication. Will need compaction strategy at scale.
- **HNSW graph rebuild on load** — O(n log n) from stored vectors. Fine for <100k entries but would need optimization for larger indexes.
- **MockBackend response shapes in two-run test** — must match MockExtractionLlm JSON schemas exactly. Schema changes break the e2e test.

### Authoritative diagnostics
- `cargo test -p ath-orchestrator -- memory` — 13 tests covering all integration points, including the e2e lifecycle
- `cargo test -p ath-memory` — 138 unit + 1 doc test for all storage/index/observation/extraction/injection components
- `ath memory stats` — runtime health check for memory store (entry count, index size, observation count, disk usage)
- `.ath/memory/observations/<run-id>.jsonl` — raw event log, grep-able by type tag
- `keyword.json` at index path — presence = persistence worked after extraction

### What assumptions changed
- **hnswlib-rs is cross-platform** → depends on Unix-only `off64` crate. Replaced with `hnsw` (pure Rust). Required custom URI↔ID mapping, separate vector persistence, and graph rebuild on load.
- **HNSW graph can be serialized directly** → RNG type (`Pcg64`) not serde-compatible. Vectors + mappings saved as JSON, graph rebuilt.
- **Phase runner would be modified with memory hooks** → kept all code in `memory.rs` for isolation. More duplication but zero risk to existing 135 tests.
- **Review requests would be captured as observations** → reviewer dispatch doesn't go through memory-aware path. Tests assert ≥3 observations per task which matches actual behavior.

## Files Created/Modified

- `crates/ath-memory/` — new crate (Cargo.toml, src/lib.rs, src/uri.rs, src/types.rs, src/error.rs, src/store.rs, src/index.rs, src/keyword.rs, src/config.rs, src/observe/*, src/extract/*, src/inject/*)
- `crates/ath-orchestrator/src/memory.rs` — memory-aware orchestrator integration (MemoryContext, BackendLlmAdapter, run_plan_with_memory, post_run_extraction, 13 tests)
- `crates/ath-orchestrator/src/coordinator.rs` — 3 struct fields changed to pub(crate) for memory.rs access
- `crates/ath-orchestrator/Cargo.toml` — added ath-memory and tracing dependencies
- `crates/ath-orchestrator/src/lib.rs` — added pub mod memory
- `crates/ath-cli/src/memory.rs` — 6 CLI subcommands (tree, search, read, add, stats, gc), 21 tests
- `crates/ath-cli/src/main.rs` — Memory CLI wiring, MemoryError in actionable_hint
- `crates/ath-cli/Cargo.toml` — added ath-memory, filetime dependencies
- `Cargo.toml` — added ath-memory workspace member, filetime workspace dependency
