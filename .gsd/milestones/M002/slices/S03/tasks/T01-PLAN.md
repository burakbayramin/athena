---
estimated_steps: 7
estimated_files: 7
---

# T01: Scaffold extract module with ExtractionLlm trait, response types, and run summary stage

**Slice:** S03 — Memory Extraction Pipeline
**Milestone:** M002

## Description

Establish the extraction module in `ath-memory` with the core LLM abstraction trait, all response types for the four extraction stages, observation preprocessing, and the first working extraction stage (run summary). This task proves the full prompt→LLM→parse→store→index pipeline with one stage before T02 extends to the remaining three.

The `ExtractionLlm` trait is defined locally in `ath-memory` (not imported from `ath-agents`) to avoid coupling to provider infrastructure. It's a minimal async trait: takes a prompt string and optional JSON schema, returns a string. The orchestrator will bridge `AgentBackend` to this trait in S04.

## Steps

1. Add `async-trait` dependency to `ath-memory/Cargo.toml`
2. Add `ExtractionError` variant to `MemoryError` in `error.rs` with stage name, message, and hint
3. Create `extract/mod.rs` — module declarations and re-exports
4. Create `extract/types.rs` — `ExtractionLlm` async trait, response structs (`RunSummary`, `Convention`, `Decision`, `AgentProfileUpdate`) with `#[serde(default)]` on optional fields, `ExtractionConfig` (max observation count, serialization budget)
5. Create `extract/prompts.rs` — observation preprocessing function (truncate to N, serialize to JSON string within token budget), `build_run_summary_prompt()` that formats observations into the extraction prompt
6. Create `extract/pipeline.rs` — `MemoryExtractor` struct holding `VikingStore`, `KeywordIndex`, and `dyn ExtractionLlm`. Implement `extract_run_summary` method: calls prompt builder → LLM → parses `RunSummary` → writes `LayeredContent` to `viking://runs/<run_id>/summary` → updates keyword index. Add `MockExtractionLlm` in `#[cfg(test)]` that dispatches canned responses based on prompt content.
7. Wire `pub mod extract;` into `lib.rs` and add re-exports

## Must-Haves

- [ ] `ExtractionLlm` trait is `async`, `Send + Sync`, takes `(&self, prompt: &str, json_schema: Option<&serde_json::Value>) -> Result<String, MemoryError>`
- [ ] Response types derive `Deserialize` with `#[serde(default)]` on optional fields
- [ ] Observation preprocessing respects configurable max count (default 100)
- [ ] `extract_run_summary` writes `LayeredContent` with abstract_text from `RunSummary.abstract_text` and overview_text from `RunSummary.overview`
- [ ] Keyword index updated with combined text from the summary entry
- [ ] Malformed LLM response → `tracing::warn` + `ExtractionError`, not panic
- [ ] `ExtractionError` added to `MemoryError` with `hint()`
- [ ] `MockExtractionLlm` returns deterministic JSON for testing

## Verification

- `cargo test -p ath-memory -- extract` — all new tests pass
- `cargo check --workspace` — clean compilation, no regressions
- Unit test: observation preprocessing truncates at configured max
- Unit test: run summary stage with mock LLM produces correct store entry and keyword index entry

## Observability Impact

- **New tracing spans:** `tracing::instrument` on `extract_run_summary`, observation preprocessing functions
- **Failure signals:** `MemoryError::ExtractionError` with stage name, message, and `hint()` — agents can grep for `ExtractionError` to find extraction failures
- **Diagnostic warnings:** `tracing::warn` on malformed LLM responses includes raw response content for debugging
- **Inspection surface:** `viking://runs/<run_id>/summary` — the stored markdown file is directly readable at `.ath/memory/store/runs/<uuid>/summary.md`
- **How a future agent verifies this task:** `cargo test -p ath-memory -- extract` exercises the full prompt→parse→store pipeline; `KeywordIndex::search()` with run content terms returns the summary entry

## Inputs

- `crates/ath-memory/src/store.rs` — `VikingStore::write()` and `VikingStore::read()` for persistence
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::add()` for search indexing
- `crates/ath-memory/src/observe/storage.rs` — `ObservationReader::read_run()` for loading observations
- `crates/ath-memory/src/observe/types.rs` — `Observation`, `ObservationType` for input data
- `docs/specs/memory-layer.md` §4.4 — extraction prompt format guidance
- S01-SUMMARY — `VikingStore` and `KeywordIndex` API surface
- S02-SUMMARY — `ObservationReader` entry point

## Expected Output

- `crates/ath-memory/Cargo.toml` — `async-trait` added to dependencies
- `crates/ath-memory/src/error.rs` — `ExtractionError` variant added
- `crates/ath-memory/src/extract/mod.rs` — module structure and re-exports
- `crates/ath-memory/src/extract/types.rs` — `ExtractionLlm` trait + all response structs + `ExtractionConfig`
- `crates/ath-memory/src/extract/prompts.rs` — observation preprocessing + run summary prompt builder
- `crates/ath-memory/src/extract/pipeline.rs` — `MemoryExtractor` with `extract_run_summary` + mock + tests
- `crates/ath-memory/src/lib.rs` — `pub mod extract` added
