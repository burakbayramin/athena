# S03: Memory Extraction Pipeline — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Extraction pipeline operates on fixture data with mock LLM — no live runtime, API calls, or human judgment needed. All behavior is deterministic and testable via cargo test.

## Preconditions

- Rust toolchain installed (`cargo` available)
- Working directory is the athena project root
- No external API keys or services required (mock LLM used throughout)

## Smoke Test

Run `cargo test -p ath-memory -- extract` — all 37 tests pass, proving the extraction pipeline is mechanically correct.

## Test Cases

### 1. Run Summary Extraction

1. Create fixture observations covering multiple phases and agents (via `ObservationWriter::write`)
2. Construct `MemoryExtractor` with `MockExtractionLlm`, in-memory `VikingStore`, and `KeywordIndex`
3. Call `extract_run_summary(run_id, observations)`
4. Read store at `viking://runs/<run_id>/summary`
5. **Expected:** Store entry exists with `abstract_text` containing the run summary text from mock LLM response. `overview_text` contains phase/agent details. Keyword index returns this entry when searched for summary-related terms.

### 2. Convention Detection with Merge

1. Run `extract_conventions(run_id_1, observations)` with mock LLM returning 2 conventions
2. Verify store has entries at `viking://project/conventions/<slug1>` and `viking://project/conventions/<slug2>`
3. Run `extract_conventions(run_id_2, observations)` with mock returning the same convention names
4. Read the convention store entries again
5. **Expected:** Convention entries contain accumulated evidence from both runs — overview text has subsections for both run_id_1 and run_id_2. Content grows, not replaces.

### 3. Decision Extraction (Write-Once Per Run)

1. Call `extract_decisions(run_id_1, observations)` — mock returns 1 decision
2. Call `extract_decisions(run_id_2, observations)` — mock returns 1 decision with same text
3. **Expected:** Two separate store entries exist at `viking://runs/<run_id_1>/decisions/<slug>` and `viking://runs/<run_id_2>/decisions/<slug>`. Decisions from different runs never overwrite each other.

### 4. Agent Profile Updates with Merge

1. Call `update_agent_profiles(run_id_1, observations)` — mock returns profiles for 2 agent kinds
2. Call `update_agent_profiles(run_id_2, observations)` — mock returns profiles for same agents
3. Read agent profile store entries
4. **Expected:** Each agent profile contains accumulated sections from both runs. Overview text shows per-run performance headers with task counts.

### 5. Full Pipeline via extract_all

1. Write fixture observations to a temp directory as JSONL (via `ObservationWriter`)
2. Call `extract_all(run_id, observations_root)` with mock LLM configured for all four stages
3. Inspect `ExtractionResult`
4. **Expected:** `result.succeeded` contains all four stage names: "run_summary", "conventions", "decisions", "agent_profiles". `result.failed` is empty. Store has entries at all expected URIs. Keyword index finds hits for content from each stage.

### 6. Malformed LLM Response Handling

1. Configure mock LLM to return invalid JSON for the conventions stage
2. Call `extract_all(run_id, observations)`
3. **Expected:** `result.failed` contains one entry for "conventions" with `MemoryError::ExtractionError`. `result.succeeded` contains "run_summary", "decisions", "agent_profiles" — other stages unaffected. No panic. Run summary is still in the store.

### 7. ExtractionError Has Actionable Hint

1. Trigger an extraction error (e.g., malformed JSON response)
2. Inspect the `MemoryError::ExtractionError` variant
3. **Expected:** Error contains `stage` field (e.g., "run_summary"), descriptive `message`, and `hint()` returns actionable text suggesting what to check or fix.

## Edge Cases

### Empty Observations

1. Call `extract_run_summary(run_id, &[])` with empty observation list
2. **Expected:** Method returns `Ok(())` as a no-op — nothing written to store, no LLM call made

### Observation Preprocessing Truncation

1. Create 200 observations when `ExtractionConfig.max_observations` is 100
2. Run preprocessing
3. **Expected:** Output contains exactly 100 observations, and they are the 100 newest (not oldest). Byte budget is also respected — if serialized output exceeds `max_bytes`, oldest lines are dropped further.

### Partial Response Deserialization

1. Provide JSON with only some fields set (e.g., `RunSummary` with `summary` but no `key_outcomes`)
2. Deserialize via serde
3. **Expected:** Missing fields get default values (empty strings, empty vecs). No deserialization error.

### Slugify Edge Cases

1. Generate slug from text "Use consistent error handling patterns"
2. **Expected:** Slug is "use-consistent-error-handling" (first 4 words, lowercased, hyphenated, non-alphanumeric stripped)

## Failure Signals

- Any test in `cargo test -p ath-memory -- extract` fails
- `cargo check --workspace` shows new warnings or errors in the extract module
- `ExtractionResult.failed` is non-empty when all mock LLM responses are valid
- Store entries missing at expected Viking URIs after `extract_all` completes
- Keyword index returns no hits for content that was just extracted
- Panic instead of `MemoryError::ExtractionError` on malformed input

## Requirements Proved By This UAT

- LLM extraction quality risk partially retired — pipeline produces valid, parseable memory entries from observation data (with mock LLM; real LLM quality is runtime concern)
- Keyword index integration proven — extracted entries are findable by content search

## Not Proven By This UAT

- Real LLM response quality — mock LLM produces perfect structured JSON; real models may produce edge-case formatting
- Runtime extraction performance — no benchmarks on observation count scaling
- Integration with orchestrator — extract_all is called standalone, not from within a phase run (S04 scope)
- CLI access to extracted memories (S05 scope)
- End-to-end two-run scenario (S06 scope)

## Notes for Tester

All test cases map directly to existing cargo tests — run `cargo test -p ath-memory -- extract` to execute the full suite. The integration tests (`integration_full_pipeline`, `integration_merge_conventions_across_runs`, `integration_malformed_response_isolated`) cover test cases 2-6 in a single pass. Individual unit tests cover cases 1 and 7. Edge cases are covered by preprocessing and type deserialization tests.
