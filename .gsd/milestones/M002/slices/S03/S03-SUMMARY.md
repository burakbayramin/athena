---
id: S03
parent: M002
milestone: M002
provides:
  - MemoryExtractor with four extraction stages (run summary, conventions, decisions, agent profiles)
  - ExtractionLlm async trait as provider-agnostic LLM boundary in ath-memory
  - extract_all orchestrator with per-stage error isolation and ExtractionResult reporting
  - Response types (RunSummary, Convention, Decision, AgentProfileUpdate) with serde(default)
  - Observation preprocessing with configurable max count and byte budget
  - MockExtractionLlm for deterministic testing
  - ExtractionError variant on MemoryError with actionable hint()
requires:
  - slice: S01
    provides: VikingStore (read/write), KeywordIndex (add/search), VikingUri parser, LayeredContent
  - slice: S02
    provides: ObservationType enum, ObservationReader (read_run), JSONL storage format
affects:
  - S04
  - S05
  - S06
key_files:
  - crates/ath-memory/src/extract/mod.rs
  - crates/ath-memory/src/extract/types.rs
  - crates/ath-memory/src/extract/prompts.rs
  - crates/ath-memory/src/extract/pipeline.rs
  - crates/ath-memory/src/error.rs
key_decisions:
  - ExtractionLlm trait defined in ath-memory, not imported from ath-agents — keeps memory crate provider-agnostic
  - Convention merge appends run-specific evidence sections to existing overview text rather than deep structural merge
  - Agent profile merge appends per-run performance sections with task count headers
  - Decisions use per-run URI paths (viking://runs/<uuid>/decisions/<slug>) to prevent cross-run collision
  - extract_all catches each stage error independently — failed convention extraction doesn't block other stages
patterns_established:
  - All extraction stages follow identical pipeline pattern — prompt → LLM → parse → merge-or-create → store → index
  - Merge stages (conventions, agent_profiles) use read-then-write — existing content loaded, appended to, and overwritten
  - Write-once stages (decisions) use per-run URI paths to prevent cross-run collision
  - Response types use serde(default) on all optional fields for graceful partial-response handling
  - MockExtractionLlm dispatches canned responses by prompt substring matching
observability_surfaces:
  - tracing::instrument on all extraction methods with run_id and observation_count fields
  - tracing::warn on malformed LLM responses includes raw response content — grep for "Malformed LLM response"
  - tracing::warn on per-stage failures in extract_all — grep for "Extraction stage failed"
  - tracing::info on extract_all completion with succeeded/failed counts
  - MemoryError::ExtractionError with stage name, message, and actionable hint()
  - ExtractionResult struct exposes succeeded/failed lists for programmatic inspection
  - Viking store files at .ath/memory/store/runs/<uuid>/summary.md, project/conventions/*.md, agents/<kind>/*.md
drill_down_paths:
  - .gsd/milestones/M002/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S03/tasks/T02-SUMMARY.md
duration: 35m
verification_result: passed
completed_at: 2026-03-14
---

# S03: Memory Extraction Pipeline

**Post-run extraction pipeline reads observations, calls LLM to produce structured memory entries across four stages, writes them to Viking store with merge semantics, and updates keyword index — all proven by integration tests with mock LLM.**

## What Happened

Built the `extract` submodule in `ath-memory` across two tasks.

**T01** established the foundation: `ExtractionLlm` async trait (provider-agnostic, defined locally in ath-memory), four response types with `serde(default)` for partial-response resilience, observation preprocessing (configurable max count and byte budget, keeps newest), and the first extraction stage (`extract_run_summary`). Added `ExtractionError` to `MemoryError` with stage name and actionable `hint()`. Created `MockExtractionLlm` that dispatches canned JSON by prompt substring.

**T02** completed the remaining three stages: `extract_conventions` (reads existing conventions, merges evidence from new run), `extract_decisions` (writes each decision to per-run URIs for write-once isolation), and `update_agent_profiles` (reads existing profiles, appends per-run performance sections). Built `extract_all` orchestrator with per-stage error isolation — a bad convention response doesn't block run summary, decisions, or agent profiles. Added full integration tests proving pipeline correctness, merge behavior across runs, and error isolation.

## Verification

- `cargo test -p ath-memory -- extract` — **37 tests pass** (19 from T01 + 18 from T02)
  - 7 type deserialization tests (full, partial, empty, defaults for all response types)
  - 6 preprocessing tests (truncation, byte budget, empty input, newest-kept)
  - 12 pipeline unit tests (store writes, index updates, malformed handling, merge, write-once, error isolation)
  - 3 integration tests (full pipeline, merge across runs, malformed response isolation)
  - 9 prompt builder tests
- `cargo test -p ath-memory -- extract::pipeline::tests::integration` — **3 integration tests pass**: full pipeline writes entries at 5+ URIs, keyword index finds hits; merge test shows conventions accumulate across runs; error isolation test confirms 3 stages succeed when one fails
- `cargo check --workspace` — clean (3 pre-existing dead_code warnings in observe/buffer.rs only)
- Diagnostic/failure-path: `extract_run_summary_malformed_response_returns_error` verifies `ExtractionError` with actionable hint

## Deviations

None.

## Known Limitations

- Extraction uses mock LLM only — real LLM quality is not proven by this slice (intentional; proof strategy defers to runtime observation)
- Convention merge is append-only (no deduplication or conflict resolution) — conventions grow monotonically
- No embedding vectors involved — extraction writes to keyword index only; vector index integration deferred to when embedding support is added
- Slugify uses first 4 words — collisions possible with very similar decision text within a single run

## Follow-ups

None — downstream work is covered by S04 (orchestrator integration), S05 (CLI), and S06 (end-to-end).

## Files Created/Modified

- `crates/ath-memory/src/extract/mod.rs` — module declarations and re-exports
- `crates/ath-memory/src/extract/types.rs` — ExtractionLlm trait, response structs, ExtractionConfig, ExtractionResult
- `crates/ath-memory/src/extract/prompts.rs` — observation preprocessing, prompt builders for all four stages
- `crates/ath-memory/src/extract/pipeline.rs` — MemoryExtractor with all four stages, extract_all orchestrator, MockExtractionLlm, 24 tests
- `crates/ath-memory/src/error.rs` — added ExtractionError variant with hint()
- `crates/ath-memory/src/lib.rs` — added pub mod extract and re-exports

## Forward Intelligence

### What the next slice should know
- `ExtractionLlm` trait is the boundary — S04 needs to provide a real implementation that wraps whatever agent provider is available, implementing `async fn complete(&self, prompt: &str, json_schema: Option<&str>) -> Result<String>`
- `extract_all` takes a `run_id: &str` and `observations_root: &Path` — the orchestrator needs to call this after a run completes, passing the observations directory
- `ExtractionResult` reports per-stage success/failure — the orchestrator should log this but not fail the run on extraction errors

### What's fragile
- Convention merge appends raw markdown sections — if upstream observation format changes significantly, the merged content may become incoherent across runs
- `MockExtractionLlm` dispatches by prompt substring ("run summary", "conventions", "decisions", "agent profile") — new stages or renamed prompts would need mock updates

### Authoritative diagnostics
- `cargo test -p ath-memory -- extract` is the single verification command — 37 tests covering all stages, merge, and error paths
- `ExtractionResult.failed` is the programmatic inspection point — each entry has stage name and full error
- Grep tracing output for "Extraction stage failed" or "Malformed LLM response" to diagnose runtime issues

### What assumptions changed
- No assumptions changed — S01 and S02 APIs were consumed exactly as specified in the boundary map
