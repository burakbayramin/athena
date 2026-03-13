# S06: End-to-End Integration — Research

**Date:** 2026-03-14

## Summary

S06 proves the full memory lifecycle: Run 1 captures observations and extracts memory → Run 2's agent prompts contain injected context from Run 1 → CLI commands show accumulated memory. The infrastructure is solid — S01–S05 delivered all building blocks. The main work is wiring a two-run integration test and fixing one real gap: the orchestrator never persists the keyword index after extraction.

The `MemoryContext` → `run_plan_with_memory` → `post_run_extraction` pipeline is complete and functional. The store (markdown files) persists naturally across runs via filesystem. But `KeywordIndex` entries added during extraction live only in memory and vanish when the `MemoryContext` is dropped. This means keyword-based search (`read_semantic_results` in `ContextInjector`) and `ath memory search` after an orchestrated run both operate on an empty or stale index. The fix is a `keywords.save()` call at the end of `post_run_extraction`. Small change, high impact.

The test design has a subtlety: `AgentRegistry` uses `mem::discriminant` for lookup (not full `AgentKind` equality), so you can only register one backend per provider type. A single-phase run with extraction calls the Claude backend 5 times (1 task + 4 extraction stages). `MockBackend::new(sequenced)` handles this cleanly. For two runs sharing the same store directory, the second `MemoryContext` points to the same filesystem root and discovers Run 1's store entries via `ContextInjector`.

## Recommendation

**Two tasks:**

1. **Fix keyword index persistence gap** — Add `keywords.save()` to `post_run_extraction()` in `memory.rs`, and save the index path in `MemoryContext`. Unit test to verify the file appears. This is the only real code change S06 requires — everything else is wiring verification.

2. **Two-run end-to-end integration test** — In `crates/ath-orchestrator/src/memory.rs` (alongside existing tests), build a test that:
   - Run 1: `run_plan_with_memory` with sequenced mock (task output + 4 extraction JSONs). Verify observations flushed and store entries created.
   - Run 2: New `MemoryContext` pointing to same store dir. `run_plan_with_memory` with `CapturingBackend`. Verify `AgentRequest.context` contains `<athena_context>` with content from Run 1 (recent_run summary section at minimum).
   - CLI verification: Use the `ath-memory` APIs directly (not CLI binary) to call `store.list()`, `store.read()`, `keywords.search()` against the shared data dir. Verify entries from both runs are present.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Mock LLM for extraction | `MockExtractionLlm::with_all_stages()` in `extract/pipeline.rs` | Provides canned JSON for all 4 extraction stages. It's `pub(crate)` so can't be used from orchestrator, but the response JSON format is reusable — copy the JSON literals into `MockBackend::new(sequenced)` responses. |
| Mock agent backend | `MockBackend::new(vec![...])` in `ath-agents` | Sequenced mode returns different responses per call — exactly what's needed for task-then-extraction flows. |
| Capturing request inspection | `CapturingBackend` pattern in `memory.rs` tests | Already proven in S04 tests. Wraps inner backend, records all `AgentRequest`s for post-hoc assertion. |
| Temp directory isolation | `tempfile::TempDir` | All existing tests use this pattern for store/observations/output dirs. |

## Existing Code and Patterns

- `crates/ath-orchestrator/src/memory.rs` — Full memory-aware orchestrator pipeline. `MemoryContext` bundles all state, `run_plan_with_memory` is the entry point, `post_run_extraction` does flush + extract. 11 existing tests. **New tests go here.**
- `crates/ath-orchestrator/src/memory.rs::CapturingBackend` — Test helper that intercepts `AgentRequest`s. **Reuse for Run 2 verification.**
- `crates/ath-memory/src/extract/pipeline.rs::MockExtractionLlm` — `pub(crate)` mock with canned responses. Can't import, but response JSON is the spec for what `MockBackend` sequenced responses should return.
- `crates/ath-memory/src/inject/injector.rs::ContextInjector` — Four sections: project identity (`viking://project/identity`), semantic search (keyword index), agent notes (`viking://agents/*/profile`), recent run (`viking://runs/*/summary`). **Run 2 should see at least `recent_run` and potentially `relevant_context` from Run 1.**
- `crates/ath-memory/src/keyword.rs::KeywordIndex::save/load` — Serializes to JSON file. **Must be called at end of extraction for cross-run persistence.**
- `crates/ath-cli/src/memory.rs::cmd_add` — Example of saving keyword index: `index.save(&index_path)`. **Pattern to follow in orchestrator.**
- `AgentRegistry` uses `mem::discriminant` for key — one backend per provider type, model string ignored. **Critical for test setup: can't have separate task + extraction backends for same provider.**

## Constraints

- **One backend per provider in registry** — `AgentRegistry::register` uses `mem::discriminant(&kind)` as key. Can't register both `Claude("opus-4")` and `Claude("default")`. `pick_extraction_backend` finds whatever Claude is registered.
- **`MockExtractionLlm` is `pub(crate)`** — Can't import from `ath-orchestrator` tests. Must duplicate canned JSON response strings in `MockBackend::new(sequenced)` payloads.
- **`MemoryContext` is consumed by `run_plan_with_memory`** — Can't reuse between runs. Must construct a fresh one per run pointing to same filesystem root.
- **Sequenced mock ordering matters** — For a single-phase run, Claude backend receives exactly: 1 task request, then 4 extraction requests (run_summary, conventions, decisions, agent_profiles). Responses must be queued in that order.
- **Keyword index path convention** — CLI uses `memory_dir.join("index").join("keyword.json")`. Orchestrator should follow the same convention for consistency.
- **`post_run_extraction` takes `&MemoryContext`** — Keyword index is behind `Arc<Mutex<KeywordIndex>>`, so saving requires locking. The lock is already acquired inside `post_run_extraction` for the extractor, so the save should happen after the extractor drops.

## Common Pitfalls

- **Sequenced mock exhaustion** — If the mock runs out of responses, `MockBackend` panics. The extraction pipeline makes 4 LLM calls (run_summary → conventions → decisions → agent_profiles), matched by prompt content. With `MockBackend::always_ok` this isn't a problem but with sequenced you need exactly the right count. Count carefully: 1 task + 4 extraction = 5 per run.
- **Extraction prompt matching** — `MockExtractionLlm` dispatches on prompt content substring matching. `MockBackend` has no such intelligence — it returns responses in queue order. Since `extract_all` calls stages in fixed order (run_summary → conventions → decisions → agent_profiles), the sequenced responses must match that order.
- **Store path vs. index path divergence** — VikingStore root is `memory_dir/store/`, keyword index is `memory_dir/index/keyword.json`. Don't confuse them when constructing `MemoryContext`.
- **Arc/Mutex teardown** — After `run_plan_with_memory` completes, the `MemoryContext` is dropped but `Arc<VikingStore>` and `Arc<Mutex<KeywordIndex>>` may still have references. For the two-run test, create completely fresh instances per run; don't try to reuse `Arc`s.

## Open Risks

- **Keyword index persistence gap might cause CLI search to return empty after orchestrated runs** — This is the primary bug S06 fixes. Low risk to fix (single `save()` call), but the integration surface is the orchestrator's `post_run_extraction` which is fail-soft — the save must also be fail-soft.
- **Extraction response order sensitivity** — If `extract_all`'s stage order changes in the future, sequenced mock tests break silently (wrong JSON for wrong stage). Documenting the expected order mitigates this.
- **`memory.rs` duplication from `phase_runner.rs`** — Any change to phase execution flow in `phase_runner.rs` must be mirrored in `memory.rs`. S06 shouldn't need to touch this, but it's the highest fragility in the memory subsystem.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust | `apollographql/skills@rust-best-practices` (2.3K installs) | Available — not needed for integration test work |
| Rust async | `wshobson/agents@rust-async-patterns` (4K installs) | Available — not needed, existing patterns are sufficient |

No directly relevant skills for this integration-test-focused slice. The work is assembling existing pieces, not introducing new technology.

## Sources

- `AgentRegistry` uses `mem::discriminant` for lookup (source: `crates/ath-orchestrator/src/phase_runner.rs:460`)
- `pick_extraction_backend` candidates: `Claude("default")`, `Gemini("default")`, `Codex("default")` (source: `crates/ath-orchestrator/src/memory.rs:856-859`)
- Keyword index never saved by orchestrator — only by CLI `cmd_add` (source: grep across `memory.rs` and `cli/memory.rs`)
- `ContextInjector::read_recent_run` walks `viking://runs/*/summary` URIs from store listing (source: `crates/ath-memory/src/inject/injector.rs:366-400`)
- `extract_all` stage order: run_summary → conventions → decisions → agent_profiles (source: `crates/ath-memory/src/extract/pipeline.rs:309-380`)
- All 586 workspace tests pass as of research time
