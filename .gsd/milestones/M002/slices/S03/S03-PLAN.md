# S03: Memory Extraction Pipeline

**Goal:** Post-run extraction reads observations, calls LLM to produce structured memory entries, writes them to Viking store, and updates keyword index.
**Demo:** Integration test with mock LLM: fixture observations in → four extraction stages run → store contains run summary, conventions, decisions, and agent profile entries → keyword index finds them by content.

## Must-Haves

- `ExtractionLlm` async trait in `ath-memory` — no dependency on `ath-agents`
- `MemoryExtractor` struct with four extraction stages: run summary, conventions, decisions, agent profiles
- Response types (`RunSummary`, `Convention`, `Decision`, `AgentProfileUpdate`) with `serde(default)` for graceful partial-response handling
- Observation preprocessing with configurable max count and serialization budget
- Each stage: builds prompt → calls `ExtractionLlm` → parses JSON → writes `LayeredContent` to `VikingStore` → updates `KeywordIndex`
- Convention and agent profile stages merge with existing entries (read-then-write), run summary and decisions are write-once
- `extract_all(run_id, observations_root)` orchestrates all four stages
- `MockExtractionLlm` for deterministic testing — no real API calls
- Graceful handling of malformed LLM responses (log warning, skip entry, don't crash)
- Add `ExtractionError` variant to `MemoryError` with hint

## Proof Level

- This slice proves: contract — extraction pipeline mechanically correct with mock LLM
- Real runtime required: no (mock LLM, fixture observations)
- Human/UAT required: no

## Verification

- `cargo test -p ath-memory -- extract` — all extraction tests pass
- `cargo test -p ath-memory -- extract::tests::integration` — full pipeline integration test: write fixture observations → `extract_all` with mock → verify store entries at expected URIs → verify keyword index finds entries
- `cargo check --workspace` — clean, no regressions
- Diagnostic/failure-path check: unit test verifies `MemoryError::ExtractionError` returned (not panic) when LLM returns malformed JSON, and `hint()` includes actionable resolution text

## Observability / Diagnostics

- Runtime signals: `tracing::instrument` on all extraction methods, `tracing::warn` on malformed LLM responses with the raw response content
- Inspection surfaces: Viking store files at `.ath/memory/store/runs/<uuid>/summary.md`, `project/conventions/*.md`, `agents/<kind>/*.md`
- Failure visibility: `MemoryError::ExtractionError` with stage name and cause; extraction continues on individual stage failure

## Integration Closure

- Upstream surfaces consumed: `VikingStore` (write, read for merge), `KeywordIndex` (add), `ObservationReader::read_run()`, `ObservationType` enum
- New wiring introduced in this slice: `extract` submodule in `ath-memory`, `ExtractionLlm` trait as the LLM boundary
- What remains before the milestone is truly usable end-to-end: S04 wires observations + injection into orchestrator, S05 adds CLI, S06 proves end-to-end

## Tasks

- [x] **T01: Scaffold extract module with ExtractionLlm trait, response types, and run summary stage** `est:45m`
  - Why: Establishes the extraction module structure, the LLM abstraction boundary, and proves the prompt→parse→store pipeline with the first extraction stage
  - Files: `crates/ath-memory/src/extract/mod.rs`, `crates/ath-memory/src/extract/types.rs`, `crates/ath-memory/src/extract/prompts.rs`, `crates/ath-memory/src/extract/pipeline.rs`, `crates/ath-memory/src/error.rs`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/Cargo.toml`
  - Do: Define `ExtractionLlm` async trait. Create response types with `serde(default)`. Build observation preprocessing (truncate to max N, serialize to JSON string within budget). Implement `extract_run_summary` that builds prompt, calls LLM, parses response, writes `LayeredContent` to `viking://runs/<uuid>/summary`. Add `MockExtractionLlm`. Add `ExtractionError` to `MemoryError`. Write unit tests for preprocessing + run summary stage.
  - Verify: `cargo test -p ath-memory -- extract` passes, `cargo check --workspace` clean
  - Done when: Run summary extraction works end-to-end with mock LLM — observation fixtures in, store entry at correct URI out, keyword index updated

- [x] **T02: Convention, decision, and agent profile stages with full pipeline integration test** `est:45m`
  - Why: Completes all four extraction stages and proves the full pipeline with an integration test
  - Files: `crates/ath-memory/src/extract/pipeline.rs`, `crates/ath-memory/src/extract/prompts.rs`, `crates/ath-memory/src/extract/types.rs`
  - Do: Implement `extract_conventions` (detect patterns, merge with existing), `extract_decisions` (extract with rationale, write-once per run), `update_agent_profiles` (per-agent stats, merge across runs). Implement `extract_all` orchestrator that runs all four stages and continues on individual failure. Write integration test: create fixture observations covering multiple agents and phases → run `extract_all` → verify store has entries at all expected URIs → verify keyword index returns results → verify merge behavior for conventions.
  - Verify: `cargo test -p ath-memory -- extract` passes (all stages + integration), `cargo check --workspace` clean
  - Done when: `extract_all` with mock LLM produces correct entries for all four stages, merge-on-update works for conventions and agent profiles, malformed response handling doesn't crash

## Files Likely Touched

- `crates/ath-memory/Cargo.toml`
- `crates/ath-memory/src/lib.rs`
- `crates/ath-memory/src/error.rs`
- `crates/ath-memory/src/extract/mod.rs`
- `crates/ath-memory/src/extract/types.rs`
- `crates/ath-memory/src/extract/prompts.rs`
- `crates/ath-memory/src/extract/pipeline.rs`
