# S06: End-to-End Integration — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: The slice is proven by integration tests using mock backends and temp directories — no live runtime or human judgment required.

## Preconditions

- Rust toolchain installed (`cargo` available)
- Repository checked out at the S06 branch
- No API keys required (all tests use mock backends)

## Smoke Test

Run `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle` — should pass in <5 seconds, proving the full memory lifecycle works.

## Test Cases

### 1. Keyword index persisted after extraction

1. Run `cargo test -p ath-orchestrator -- memory::tests::keyword_index_persisted_after_extraction`
2. **Expected:** Test passes. Internally: `run_plan_with_memory` completes, `keyword.json` exists at `memory.index_path`, `KeywordIndex::load()` succeeds on that file.

### 2. Two-run end-to-end memory lifecycle

1. Run `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle`
2. **Expected:** Test passes. Internally:
   - Run 1: Sequenced mock returns task output + 4 extraction JSONs. After run: observations directory has `.jsonl` file, `store.list()` returns entries (run summary, conventions, decisions), `keyword.json` exists on disk.
   - Run 2: Fresh store/index loaded from same temp dir. `CapturingBackend` captures the task request. After run: captured `AgentRequest.context` contains `<athena_context>` with `<recent_run>` section referencing Run 1's content ("authentication"/"JWT").
   - Cross-run: `store.list()` returns Run 1 entries, `KeywordIndex::load()` + `.search("authentication")` returns non-empty results.

### 3. Memory errors don't fail the run (fail-soft)

1. Run `cargo test -p ath-orchestrator -- memory::tests::memory_errors_do_not_fail_run`
2. **Expected:** Test passes. A run with broken memory paths completes successfully — memory errors are logged but don't propagate.

### 4. Full workspace regression check

1. Run `cargo test --workspace`
2. **Expected:** All tests pass (588+ tests, 0 failures). No regressions from S06 changes.

### 5. Context injection contains memory from prior run

1. Run `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle`
2. Inspect the test source in `crates/ath-orchestrator/src/memory.rs` — find the assertion on `captured.context`
3. **Expected:** The assertion checks that `context` is `Some`, contains `<athena_context>`, and contains `<recent_run>` — proving injected context from Run 1 appears in Run 2's agent request.

### 6. Store and keyword index APIs work across runs

1. Run `cargo test -p ath-orchestrator -- memory::tests::two_run_end_to_end_memory_lifecycle`
2. Inspect the test source — find the cross-run API assertions after Run 2
3. **Expected:** `store.list()` returns non-empty (Run 1 entries persist), `KeywordIndex::load()` succeeds, `.search("authentication")` and `.search("JWT")` return non-empty results.

## Edge Cases

### Keyword save failure is fail-soft

1. Run `cargo test -p ath-orchestrator -- memory::tests::memory_errors_do_not_fail_run`
2. **Expected:** Test constructs a `MemoryContext` with broken paths. The run completes without error. Keyword save failure emits `tracing::warn` but doesn't propagate.

### Empty memory on first run

1. In `two_run_end_to_end_memory_lifecycle`, Run 1 starts with an empty store and index.
2. **Expected:** No crash, no injected context (nothing to inject yet). Extraction produces new entries. Run 2 picks them up.

## Failure Signals

- `keyword_index_persisted_after_extraction` fails → `keywords.save()` call in `post_run_extraction` is broken or `index_path` is misconfigured
- `two_run_end_to_end_memory_lifecycle` fails on Run 1 store assertions → extraction JSON shapes changed (compare with `MockExtractionLlm::with_all_stages()`)
- `two_run_end_to_end_memory_lifecycle` fails on Run 2 context assertion → `ContextInjector::read_recent_run` broken or URI naming convention changed
- `two_run_end_to_end_memory_lifecycle` fails on keyword search → `KeywordIndex::save`/`load` round-trip broken or extraction stages don't call `keywords.add()`
- Workspace test count drops significantly → something in S06 broke an import or type definition

## Not Proven By This UAT

- Live API calls to real LLM providers (tests use mock backends)
- CLI `ath memory` commands against real memory data (covered by S05 tests)
- Memory behavior with large datasets (hundreds of entries, multiple runs)
- Parallel memory-aware execution (deferred — D014)
- Cross-project memory sharing (out of scope for M002)

## Notes for Tester

- All tests use `tempdir` — no persistent state between test runs. Each test is fully self-contained.
- The 3 pre-existing warnings in `ath-memory` (dead code) are known and harmless — they come from test helper code.
- The two-run test is ~170 lines of setup. If it fails, the assertion message will tell you which lifecycle stage broke. Follow the diagnostic guide in S06-PLAN.md's "Verification (Diagnostic)" section.
