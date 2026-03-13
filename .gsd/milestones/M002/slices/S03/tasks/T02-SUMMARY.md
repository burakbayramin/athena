---
id: T02
parent: S03
milestone: M002
provides:
  - extract_conventions with read-then-merge semantics on project conventions
  - extract_decisions with write-once per-run URI isolation
  - update_agent_profiles with read-then-merge across runs
  - extract_all orchestrator with per-stage error isolation and ExtractionResult reporting
  - ExtractionResult struct for inspecting pipeline success/failure
  - Full integration tests proving pipeline, merge, and error-isolation behavior
key_files:
  - crates/ath-memory/src/extract/pipeline.rs
  - crates/ath-memory/src/extract/prompts.rs
  - crates/ath-memory/src/extract/types.rs
key_decisions:
  - Convention merge appends run-specific evidence sections to existing overview text rather than attempting deep structural merge
  - Agent profile merge appends run sections with summary header showing task count
  - Slugify function takes first 4 words of decision/convention text for URL-safe URIs
  - MockExtractionLlm dispatches by prompt substring — with_all_stages and with_all_stages_run2 cover all four stage prompts for single and merge testing
patterns_established:
  - All extraction stages follow identical pipeline pattern — prompt → LLM → parse → merge-or-create → store → index
  - Merge stages (conventions, agent_profiles) use read-then-write — existing content is loaded, appended to, and overwritten
  - Write-once stages (decisions) use per-run URI paths (viking://runs/<uuid>/decisions/<slug>) to prevent cross-run collision
  - extract_all catches each stage error independently — a bad convention response doesn't block run summary or decisions
observability_surfaces:
  - tracing::instrument on extract_conventions, extract_decisions, update_agent_profiles, extract_all with run_id and observation_count fields
  - tracing::warn on each stage failure in extract_all with stage name and error details
  - tracing::info on extract_all completion with succeeded/failed counts
  - ExtractionResult struct exposes succeeded Vec<String> and failed Vec<(String, MemoryError)> for programmatic inspection
duration: 20m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Convention, decision, and agent profile stages with full pipeline integration test

**Completed all four extraction stages, extract_all orchestrator with per-stage error isolation, and integration tests proving full pipeline, merge behavior, and malformed response handling.**

## What Happened

Built three new extraction methods on `MemoryExtractor`: `extract_conventions` (reads existing conventions from store, merges evidence from new run), `extract_decisions` (writes each decision to `viking://runs/<run_id>/decisions/<slug>` for write-once isolation), and `update_agent_profiles` (reads existing profiles, appends per-run performance sections).

Added `extract_all` orchestrator that loads observations via `ObservationReader::read_run`, runs all four stages sequentially, catches and logs per-stage errors, and returns `ExtractionResult` with success/failure lists. A failed convention extraction does not prevent run summary, decisions, or agent profiles from completing.

Added three prompt builders (`build_conventions_prompt`, `build_decisions_prompt`, `build_agent_profiles_prompt`) and the `ExtractionResult` type. Extended `MockExtractionLlm` with `with_all_stages()` and `with_all_stages_run2()` factory methods for comprehensive testing.

## Verification

- `cargo test -p ath-memory -- extract` — 37 tests passed (unit + integration), 0 failed
- `cargo test -p ath-memory -- extract::pipeline::tests::integration` — 3 integration tests passed (full pipeline, merge, malformed response isolation)
- `cargo check --workspace` — clean, no regressions
- Integration test: fixture observations → extract_all → store entries at 5+ URIs (summary, 2 conventions, 1 decision, 2 agent profiles) → keyword index finds hits for summary, convention, and decision content
- Merge test: two extract_all calls → convention store entries contain accumulated evidence from both runs
- Error isolation test: malformed convention response → conventions stage fails, other 3 stages succeed, summary still in store

## Diagnostics

- **Store inspection:** Check `.ath/memory/store/project/conventions/*.md` for convention entries, `runs/<uuid>/decisions/*.md` for decisions, `agents/<kind>/*.md` for agent profiles
- **ExtractionResult:** `result.succeeded` and `result.failed` report per-stage status programmatically
- **Tracing:** grep for `Extraction stage failed` to find per-stage errors in logs, grep for `Malformed LLM response` for parse failures
- **Merge verification:** Convention files grow longer on each run — the overview section accumulates `### Run <uuid>` subsections

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/src/extract/pipeline.rs` — Added extract_conventions, extract_decisions, update_agent_profiles, extract_all, ExtractionResult, slugify helper, and 15 new tests (unit + integration)
- `crates/ath-memory/src/extract/prompts.rs` — Added build_conventions_prompt, build_decisions_prompt, build_agent_profiles_prompt and their tests
- `crates/ath-memory/src/extract/types.rs` — Added ExtractionResult struct, added Serialize derive to Convention and AgentProfileUpdate
- `crates/ath-memory/src/extract/mod.rs` — Exported ExtractionResult
- `crates/ath-memory/src/lib.rs` — Re-exported ExtractionResult at crate root
