---
estimated_steps: 5
estimated_files: 1
---

# T02: Two-run end-to-end integration test

**Slice:** S06 — End-to-End Integration
**Milestone:** M002

## Description

The milestone's definition-of-done requires a two-run scenario proving that memory from Run 1 appears in Run 2's agent prompts. This test assembles all building blocks (S01–S05) into a single integration test proving the full lifecycle: observation capture → extraction → store persistence → keyword persistence → context injection on next run → store/index API verification.

## Steps

1. Build Run 1 mock setup: `MockBackend::new(sequenced)` with exactly 5 responses for Claude — 1 task output JSON, then 4 extraction JSONs (run_summary, conventions, decisions, agent_profiles) using the same JSON shapes as `MockExtractionLlm::with_all_stages()`. Separate `MockBackend::always_ok` for Gemini reviewer returning passing verdict.
2. Run 1 execution: Construct `MemoryContext` with temp dir, `AgentCoordinator` with registry, call `run_plan_with_memory`. Assert: result is `Ok`, observations JSONL exists via `ObservationReader::read_run`, store has entries (check `store.list()` is non-empty), keyword index file exists on disk.
3. Run 2 setup: Fresh `MemoryContext` pointing to same temp dir root. New `VikingStore` and `KeywordIndex::load()` from same paths. Wrap a `MockBackend::always_ok` (task output) in `CapturingBackend` for Claude. New `AgentCoordinator`.
4. Run 2 execution and assertion: Call `run_plan_with_memory`. Inspect captured requests — the task request's `context` field should contain `<athena_context>` with at least a `<recent_run>` section containing text from Run 1's extraction (e.g. "authentication" or "JWT" from the run summary).
5. Cross-run API verification: Using the shared temp dir, call `store.list()` and verify entries from Run 1 are present (e.g. `viking://runs/*/summary`). Load keyword index from disk and call `.search("authentication", 5)` — verify non-empty results. This proves CLI-equivalent operations work on orchestrator-produced data.

## Must-Haves

- [ ] Run 1 produces store entries and persisted keyword index
- [ ] Run 2's `AgentRequest.context` contains `<athena_context>` with Run 1 content
- [ ] Store list shows entries from extraction (run summary, conventions, decisions, or agent profiles)
- [ ] Keyword index search returns results for terms from Run 1
- [ ] Test is self-contained (temp dirs, mock backends, no external dependencies)

## Verification

- `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end` — passes
- `cargo test --workspace` — zero failures, zero regressions

## Inputs

- T01's `index_path` field on `MemoryContext` and `keywords.save()` in `post_run_extraction`
- `CapturingBackend` pattern from existing tests in `memory.rs`
- `MockExtractionLlm::with_all_stages()` JSON shapes from `crates/ath-memory/src/extract/pipeline.rs` (copied as string literals for `MockBackend::new(sequenced)`)
- `AgentRegistry` uses `mem::discriminant` — one backend per provider type, sequenced mock handles both task and extraction calls

## Observability Impact

- **Test as diagnostic surface**: The `two_run_end_to_end_memory_lifecycle` test itself is the primary diagnostic. A failure pinpoints which stage of the memory lifecycle broke — store persistence, keyword persistence, context injection, or store/index API access.
- **Failure inspection**: The test asserts at each stage boundary (Run 1 store entries, keyword file on disk, Run 2 context injection, cross-run API queries). A future agent can read the assertion that failed to know exactly which lifecycle stage regressed.
- **No new runtime signals**: This task adds only test code, no production-path changes. Existing tracing signals from T01 (`"keyword index save failed"`, extraction pipeline logs) remain the runtime diagnostic surfaces.

## Expected Output

- `crates/ath-orchestrator/src/memory.rs` — new `two_run_end_to_end_memory_lifecycle` test proving the full memory lifecycle across two runs
