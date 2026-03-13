---
id: T01
parent: S03
milestone: M002
provides:
  - ExtractionLlm async trait (local to ath-memory, no ath-agents coupling)
  - Response types (RunSummary, Convention, Decision, AgentProfileUpdate) with serde(default)
  - Observation preprocessing with configurable max count and byte budget
  - extract_run_summary pipeline stage (prompt → LLM → parse → store → keyword index)
  - MockExtractionLlm for deterministic testing
  - ExtractionError variant on MemoryError with actionable hint()
key_files:
  - crates/ath-memory/src/extract/types.rs
  - crates/ath-memory/src/extract/prompts.rs
  - crates/ath-memory/src/extract/pipeline.rs
  - crates/ath-memory/src/extract/mod.rs
  - crates/ath-memory/src/error.rs
key_decisions:
  - ExtractionLlm trait defined in ath-memory, not imported from ath-agents — keeps memory crate provider-agnostic
  - MockExtractionLlm dispatches canned responses by matching prompt substrings — flexible for multi-stage testing in T02
  - Observation preprocessing keeps newest N (not oldest) and drops oldest when over byte budget
patterns_established:
  - Extraction stages follow prompt → LLM → parse → store → index pipeline pattern
  - Response types use serde(default) on all optional fields for graceful partial-response handling
  - tracing::instrument on pipeline methods with run_id and observation_count fields
observability_surfaces:
  - tracing::instrument on extract_run_summary and preprocess_observations
  - tracing::warn on malformed LLM responses includes raw response content
  - MemoryError::ExtractionError with stage name, message, and actionable hint()
  - viking://runs/<run_id>/summary — stored markdown readable at .ath/memory/store/runs/<uuid>/summary.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Scaffolded extract module with ExtractionLlm trait, response types, and run summary stage

**Built the extraction pipeline foundation: LLM abstraction trait, four response types, observation preprocessing, and working run summary stage with mock LLM.**

## What Happened

Created the `extract` submodule in `ath-memory` with four files:

1. **types.rs** — `ExtractionLlm` async trait (takes prompt + optional JSON schema, returns string), `ExtractionConfig` (max observation count, byte budget), and all four response structs (`RunSummary`, `Convention`, `Decision`, `AgentProfileUpdate`) with `#[serde(default)]` on optional fields.

2. **prompts.rs** — `preprocess_observations()` truncates to configurable max count (keeping newest), serializes to JSONL, and drops oldest lines until within byte budget. `build_run_summary_prompt()` formats observations into the extraction prompt following memory-layer.md §4.4 guidance.

3. **pipeline.rs** — `MemoryExtractor` struct holds `VikingStore`, `KeywordIndex`, and `dyn ExtractionLlm`. `extract_run_summary` runs the full pipeline: preprocess → prompt → LLM → parse `RunSummary` → write `LayeredContent` to `viking://runs/<run_id>/summary` → update keyword index. `MockExtractionLlm` dispatches canned JSON responses based on prompt content matching.

4. **mod.rs** — Module declarations and re-exports.

Also added `ExtractionError` variant to `MemoryError` in `error.rs` with stage name, message, and actionable `hint()`. Wired `pub mod extract` and re-exports into `lib.rs`. `async-trait` was already a workspace dependency.

## Verification

- `cargo test -p ath-memory -- extract` — **19 tests pass**:
  - 7 type deserialization tests (full, partial, empty, defaults for all response types)
  - 6 preprocessing tests (truncation at max count, byte budget, empty input, newest-kept, default config)
  - 6 pipeline tests (store entry written, keyword index updated, malformed response returns error, empty observations noop, error hint actionable, mock LLM deterministic)
- `cargo check --workspace` — clean (3 pre-existing dead_code warnings in observe/buffer.rs, none from extract)

**Slice-level verification (partial — T01 of T02):**
- ✅ `cargo test -p ath-memory -- extract` — passes
- ⏳ `cargo test -p ath-memory -- extract::tests::integration` — integration test not yet created (T02 scope)
- ✅ `cargo check --workspace` — clean
- ✅ Diagnostic/failure-path: `extract_run_summary_malformed_response_returns_error` verifies `ExtractionError` with actionable hint

## Diagnostics

- **Tracing spans:** `extract_run_summary` and `preprocess_observations` are instrumented with `run_id` and observation count fields
- **Malformed response:** `tracing::warn` logs raw response content + parse error — grep for `Malformed LLM response` in logs
- **Error inspection:** `MemoryError::ExtractionError` includes `stage` field ("run_summary", etc.) and `hint()` returns actionable resolution text
- **Store inspection:** read `viking://runs/<run_id>/summary` or check `.ath/memory/store/runs/<uuid>/summary.md` on disk

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/src/extract/mod.rs` — module declarations and re-exports
- `crates/ath-memory/src/extract/types.rs` — ExtractionLlm trait, response structs, ExtractionConfig
- `crates/ath-memory/src/extract/prompts.rs` — observation preprocessing and run summary prompt builder
- `crates/ath-memory/src/extract/pipeline.rs` — MemoryExtractor with extract_run_summary + MockExtractionLlm + tests
- `crates/ath-memory/src/error.rs` — added ExtractionError variant with hint()
- `crates/ath-memory/src/lib.rs` — added pub mod extract and re-exports
- `crates/ath-memory/Cargo.toml` — async-trait already present (no change needed)
