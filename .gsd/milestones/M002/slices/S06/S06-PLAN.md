# S06: End-to-End Integration

**Goal:** Full two-run memory lifecycle works — Run 1 captures observations and extracts memory, Run 2's agent prompts contain injected context from Run 1, CLI APIs show accumulated memory.
**Demo:** An integration test proves the complete cycle: run → extract → persist → reload → inject → verify via store/index APIs.

## Must-Haves

- Keyword index persisted to disk after `post_run_extraction` (the persistence gap fix)
- Two-run integration test where Run 2's `AgentRequest.context` contains content extracted from Run 1
- Store entries from Run 1 are readable via `VikingStore` APIs in Run 2's context
- Keyword index search returns results across runs after persistence
- All existing tests pass (zero regressions)

## Proof Level

- This slice proves: final-assembly (full memory lifecycle end-to-end)
- Real runtime required: no (mock backends, temp directories)
- Human/UAT required: no (integration test is sufficient proof)

## Verification

- `cargo test -p ath-orchestrator -- memory::tests::keyword_index_persisted_after_extraction` — proves the persistence fix
- `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end` — proves the full lifecycle
- `cargo test --workspace` — zero regressions

## Integration Closure

- Upstream surfaces consumed: `VikingStore` + `KeywordIndex` (S01), `ObservationBuffer` + `ObservationReader` (S02), `MemoryExtractor` + extraction JSON schemas (S03), `ContextInjector` + `MemoryContext` + `run_plan_with_memory` (S04), CLI memory APIs (S05)
- New wiring introduced in this slice: `keywords.save()` call in `post_run_extraction`, `index_path` field on `MemoryContext`
- What remains before the milestone is truly usable end-to-end: nothing — this is the final assembly slice

## Tasks

- [x] **T01: Fix keyword index persistence in post_run_extraction** `est:25m`
  - Why: `post_run_extraction` modifies the keyword index during extraction but never calls `keywords.save()`, so keyword search after an orchestrated run operates on an empty/stale index. This is the only real code change S06 requires.
  - Files: `crates/ath-orchestrator/src/memory.rs`
  - Do: Add `index_path: PathBuf` field to `MemoryContext`. After `extract_all` completes in `post_run_extraction`, call `keywords_guard.save(&memory.index_path)` (fail-soft — log warning on error, don't propagate). Update `make_memory_context` test helper to set `index_path` to `tmp.path().join("index").join("keyword.json")`. Update all existing test sites that construct `MemoryContext` to include `index_path`. Add a focused test that runs extraction via `run_plan_with_memory` and then asserts `keyword.json` exists on disk and contains entries.
  - Verify: `cargo test -p ath-orchestrator -- memory` — all existing + new test pass
  - Done when: `keyword.json` file exists on disk after `run_plan_with_memory` completes, and `KeywordIndex::load` on that file returns a non-empty index

- [x] **T02: Two-run end-to-end integration test** `est:40m`
  - Why: This is the milestone's definition-of-done test — proving that memory accumulated in Run 1 appears in Run 2's agent prompts, and that store/index APIs see data from both runs.
  - Files: `crates/ath-orchestrator/src/memory.rs`
  - Do: Build `two_run_end_to_end_memory_lifecycle` test. Run 1: `MockBackend::new(sequenced)` with 5 responses (1 task output + 4 extraction JSONs in stage order: run_summary → conventions → decisions → agent_profiles), plus reviewer mock. Verify observations flushed and store entries created. Run 2: Fresh `MemoryContext` pointing to same temp dir. `CapturingBackend` wrapping `MockBackend::always_ok`. Verify captured `AgentRequest.context` contains `<athena_context>` with `<recent_run>` section referencing Run 1's content. CLI verification: call `store.list()`, `store.read()`, `KeywordIndex::load()` + `.search()` against shared data dir — verify entries from Run 1 exist and keyword search returns results.
  - Verify: `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end` passes; `cargo test --workspace` — zero failures
  - Done when: Test proves the full lifecycle: capture → extract → persist → reload → inject → verify

## Observability / Diagnostics

- **Keyword save warning**: `tracing::warn` emitted when `keywords.save()` fails in `post_run_extraction`, includes `run_id`, error message, and `index_path`. A future agent can grep for `"keyword index save failed"` to detect persistence issues.
- **Extraction pipeline log**: existing `tracing::info` on extraction completion now also implies keyword index was persisted (success) or that a warn was emitted (failure). No new log for the happy path — the save is silent on success.
- **Inspection surface**: after a run, `keyword.json` at the configured `index_path` is the on-disk artifact. Its absence after a successful extraction indicates a persistence failure. `KeywordIndex::load(path)` is the programmatic inspection surface.
- **Failure-path check**: `memory_errors_do_not_fail_run` test already proves fail-soft behavior; `keyword_index_persisted_after_extraction` proves the happy path.
- **Redaction**: no secrets involved — keyword index contains URI strings and TF weights only.

## Verification (Diagnostic)

- `cargo test -p ath-orchestrator -- memory::tests::memory_errors_do_not_fail_run` — proves fail-soft: broken paths don't crash the run, memory errors silently logged
- `cargo test -p ath-orchestrator -- memory::tests::keyword_index_persisted_after_extraction` — proves persistence: keyword.json exists on disk after extraction
- In `two_run_end_to_end` test: if Run 2's `AgentRequest.context` is `None`, inspect `store.list()` output — empty means extraction failed silently (check logs for `"Extraction stage failed"`); non-empty means `ContextInjector` didn't match (check query/budget). If keyword.json is absent, `"keyword index save failed"` in logs points to the persistence failure.

## Files Likely Touched

- `crates/ath-orchestrator/src/memory.rs`
