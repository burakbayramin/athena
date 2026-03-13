---
estimated_steps: 5
estimated_files: 3
---

# T02: Convention, decision, and agent profile stages with full pipeline integration test

**Slice:** S03 — Memory Extraction Pipeline
**Milestone:** M002

## Description

Complete the remaining three extraction stages and the `extract_all` orchestrator. Conventions and agent profiles use read-then-merge semantics (accumulate across runs). Decisions are write-once per run. The `extract_all` method runs all four stages sequentially, logging and continuing on individual stage failures so that a bad convention extraction doesn't prevent a good run summary from being stored.

The integration test creates fixture observations covering multiple agents and phases, runs `extract_all` with the mock LLM, and verifies entries exist at all expected URIs with correct content, keyword index is searchable, and merge behavior works.

## Steps

1. Implement `extract_conventions` on `MemoryExtractor` — builds prompt from observations, parses `Vec<Convention>`, reads existing conventions from store (merge), writes each to `viking://project/conventions/<slug>`, updates keyword index
2. Implement `extract_decisions` on `MemoryExtractor` — builds prompt, parses `Vec<Decision>`, writes each to `viking://runs/<run_id>/decisions/<slug>`, updates keyword index. Write-once semantics (unique per run UUID).
3. Implement `update_agent_profiles` on `MemoryExtractor` — builds prompt grouped by agent kind, parses `Vec<AgentProfileUpdate>`, reads existing profiles (merge), writes to `viking://agents/<kind>/profile`, updates keyword index
4. Implement `extract_all(run_id, observations_root)` — loads observations via `ObservationReader::read_run`, calls preprocessing, runs all four stages, catches and logs per-stage errors, returns summary of what succeeded/failed
5. Write integration test: create temp dir → write fixture observations via `ObservationWriter` → construct `MemoryExtractor` with real `VikingStore`, real `KeywordIndex`, mock LLM → call `extract_all` → assert store entries at all expected URIs → assert keyword search returns hits → test merge by running extraction twice with different observations → verify conventions accumulated not replaced

## Must-Haves

- [ ] `extract_conventions` reads existing entries and merges (does not overwrite)
- [ ] `extract_decisions` writes to per-run URIs (no cross-run collision)
- [ ] `update_agent_profiles` reads existing profile and merges
- [ ] `extract_all` continues on individual stage failure — other stages still produce results
- [ ] `extract_all` returns a result struct indicating which stages succeeded/failed
- [ ] Integration test exercises full pipeline: observations → extract_all → store + index verification
- [ ] Merge behavior test: second extraction accumulates conventions, doesn't replace first run's
- [ ] Malformed response from one stage doesn't prevent other stages from running

## Verification

- `cargo test -p ath-memory -- extract` — all extraction tests pass (unit + integration)
- `cargo check --workspace` — clean, no regressions
- Integration test: fixture observations → `extract_all` → store entries at 4+ URIs → keyword index returns hits for summary, convention, and decision content
- Merge test: two `extract_all` calls → convention store entries contain accumulated content

## Observability Impact

- Signals added/changed: `tracing::instrument` on each extraction method, `tracing::warn` on stage failure with error details, `tracing::info` on `extract_all` completion with stage success/fail counts
- How a future agent inspects this: check store files at `.ath/memory/store/runs/` and `.ath/memory/store/project/conventions/`; `ExtractionResult` struct reports per-stage status
- Failure state exposed: `ExtractionResult` contains `Vec<(stage_name, MemoryError)>` for failed stages

## Inputs

- `crates/ath-memory/src/extract/pipeline.rs` — `MemoryExtractor` with `extract_run_summary` from T01
- `crates/ath-memory/src/extract/types.rs` — response types from T01
- `crates/ath-memory/src/extract/prompts.rs` — prompt builder pattern from T01
- `crates/ath-memory/src/observe/storage.rs` — `ObservationWriter` + `ObservationReader` for fixture creation
- `crates/ath-memory/src/observe/types.rs` — `Observation`, `ObservationType` for test fixtures

## Expected Output

- `crates/ath-memory/src/extract/pipeline.rs` — three new extraction methods + `extract_all` orchestrator + `ExtractionResult` type + integration tests
- `crates/ath-memory/src/extract/prompts.rs` — prompt builders for conventions, decisions, agent profiles
- `crates/ath-memory/src/extract/types.rs` — `ExtractionResult` struct if not already defined in T01
