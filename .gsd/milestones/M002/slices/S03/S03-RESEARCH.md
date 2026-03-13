# S03: Memory Extraction Pipeline — Research

**Date:** 2026-03-14

## Summary

The extraction pipeline reads observations from a completed run (via `ObservationReader`), sends them to an LLM for structured analysis, and writes the results to `VikingStore` + `MemoryIndex` + `KeywordIndex`. The pipeline has four extraction stages from the spec: run summary, convention detection, decision extraction, and agent profile updates. Each stage takes observation data, produces a JSON-schema-constrained LLM response, and maps the output to `LayeredContent` entries at specific `viking://` URIs.

The critical design decision is how to invoke the LLM. `ath-memory` currently depends only on `ath-types` — adding `ath-agents` would create coupling to provider infrastructure (`genai`, `backon`, circuit breakers). Instead, define a minimal async `ExtractionLlm` trait in `ath-memory` that accepts a prompt and optional JSON schema, returns a string. The orchestrator (which already depends on both `ath-memory` and `ath-agents`) provides the real implementation; tests use a deterministic mock. This matches the existing pattern where `AgentBackend` lives in `ath-agents` and consumers depend on traits, not impls.

The LLM extraction quality risk (identified in the roadmap) is the primary risk this slice retires. The strategy: constrain outputs via JSON schema, validate parsed responses against expected fields, and handle partial/malformed responses gracefully with fallback defaults. Integration tests use fixture observations with a mock LLM that returns canned JSON.

## Recommendation

**Approach: Trait-based extraction with four-stage pipeline and mock-first testing.**

1. Define `ExtractionLlm` trait in `ath-memory::extract` — `async fn complete(&self, prompt: &str, json_schema: Option<&serde_json::Value>) -> Result<String, MemoryError>`
2. Build `MemoryExtractor` struct that owns references to `VikingStore`, `MemoryIndex`, `KeywordIndex`, and an `ExtractionLlm` impl
3. Implement four extraction stages as separate methods on `MemoryExtractor`:
   - `extract_run_summary(observations) -> RunSummary` → writes `viking://runs/<uuid>/summary`
   - `extract_conventions(observations) -> Vec<Convention>` → writes/updates `viking://project/conventions/*`
   - `extract_decisions(observations) -> Vec<Decision>` → writes `viking://runs/<uuid>/decisions/*`
   - `update_agent_profiles(observations) -> Vec<AgentProfileUpdate>` → writes/updates `viking://agents/<kind>/*`
4. `extract_all(run_id, observations_root)` orchestrates all four stages
5. All tests use `MockExtractionLlm` that returns fixture JSON — no real API calls

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| JSON schema for extraction responses | `serde_json::Value` + `serde::Deserialize` | Already in workspace; `AgentRequest.json_schema` established the pattern |
| Observation reading | `ObservationReader::read_run()` | Built in S02 exactly for this purpose |
| Store persistence | `VikingStore::write()` | Built in S01, handles atomic writes and directory creation |
| Index updates | `MemoryIndex::upsert()` + `KeywordIndex::add()` | Built in S01, handles upsert semantics |
| URI construction | `VikingUri::from_str()` | Built in S01, validates segments |
| Async trait | `async-trait` crate | Already a workspace dependency, used by `AgentBackend` |

## Existing Code and Patterns

- `crates/ath-memory/src/observe/storage.rs` — `ObservationReader::read_run(root, run_id)` returns `Vec<Observation>` for a given run UUID. This is the extraction pipeline's input.
- `crates/ath-memory/src/observe/types.rs` — `ObservationType` enum with 7 variants. The extraction prompts need to reference these variant names since the LLM sees serialized JSONL. Variants are internally tagged (`"type":"AgentResponse"` etc.).
- `crates/ath-memory/src/store.rs` — `VikingStore::write(&LayeredContent)` handles atomic persistence. `read()` returns `Option<LayeredContent>` for merge-on-update scenarios (e.g., agent profiles accumulate across runs).
- `crates/ath-memory/src/index.rs` — `MemoryIndex::upsert(uri_str, &[f32])` requires a vector. The extraction pipeline doesn't have embeddings (that's an API call). The pipeline should accept an optional embedding function or skip vector indexing when no embedder is available — keyword index is always updated.
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::add(uri_str, &text)` — always available, no API dependency. The extraction pipeline should always update this.
- `crates/ath-agents/src/backend.rs` — `AgentBackend::send(AgentRequest) -> AgentResponse` — the real LLM call path. The orchestrator will bridge this to the `ExtractionLlm` trait.
- `crates/ath-types/src/agent.rs` — `AgentRequest` has `json_schema: Option<serde_json::Value>` — establishes the pattern for structured output. `AgentKind` is used in observations so the extraction pipeline can group by agent.

## Constraints

- **`ath-memory` must not depend on `ath-agents`** — would create coupling to `genai`, `backon`, circuit breakers. Define extraction LLM trait locally; orchestrator provides impl.
- **`MemoryIndex::upsert` requires vectors** — embedding is an API call not available in `ath-memory`. The extraction pipeline must accept an optional embedder callback or trait, and skip vector indexing when none is provided. Keyword index is always updated.
- **`MemoryIndex` requires `&mut self`** — all extraction writes need exclusive access. Not a problem for post-run extraction (single-threaded), but the trait design should not require `Send` on the index.
- **Observation JSONL may be large** — a complex run could produce hundreds of observations. The extraction prompt must truncate or summarize to fit within LLM context windows. Define a max observation count (configurable, default ~100) and a serialization budget.
- **JSON schema validation** — LLM responses may not conform to schema despite `json_schema` being set. Parse with `serde_json::from_str` and handle deserialization errors as soft failures with tracing warnings, not hard errors.
- **`async-trait` needed** — the `ExtractionLlm` trait must be async (LLM calls are network I/O). Add `async-trait` to `ath-memory`'s deps.
- **`tokio` not needed in `ath-memory`** — the extraction pipeline is async but doesn't need a runtime. The caller (orchestrator) provides the runtime. Keep `tokio` out of `ath-memory` deps.
- **No `PartialEq` on `LayeredContent`** — the struct derives `Debug, Clone, Serialize, Deserialize` but not `PartialEq`. Tests that check content equality need field-by-field assertions or we add `PartialEq` derive.

## Common Pitfalls

- **Overloading the extraction prompt** — Sending all observations as raw JSONL to a single prompt risks hitting context limits and producing low-quality summaries. Mitigate by: (a) summarizing observations before sending (count by type, group by phase), (b) using separate focused prompts per extraction stage, (c) configurable max observation count.
- **Tight coupling between prompt format and response parsing** — If extraction prompts change, response structs must change too. Mitigate by: defining response types as `#[derive(Deserialize)]` structs with `#[serde(default)]` on optional fields, so partial responses degrade gracefully.
- **Merging vs overwriting on repeated extraction** — Convention entries and agent profiles should accumulate across runs, not be overwritten. The pipeline should `read()` existing content, merge, then `write()`. Run summaries are write-once (unique URI per run).
- **Vector index without embeddings** — The `MemoryIndex` requires actual embedding vectors. If no embedding API is available, the pipeline should skip vector indexing entirely and only update the keyword index. This is the "graceful degradation" requirement from M002-CONTEXT.
- **UUID-based URIs for runs** — Run summary URIs will be `viking://runs/<uuid>/summary`. Need to ensure UUID strings are valid URI segments (they are — lowercase hex + hyphens).

## Open Risks

- **LLM extraction quality** — This is the risk this slice retires. The mock tests prove the pipeline works mechanically, but real LLM output quality can only be validated with integration tests against a live API (deferred to S06 end-to-end testing). The structured JSON schema constraint mitigates but doesn't eliminate this risk.
- **Observation serialization size** — A run with 100+ observations could produce 50KB+ of JSONL. Truncation strategy (which observations to keep, which to summarize) affects extraction quality. Start with a simple "take first N" approach and iterate.
- **Embedding dimension coupling** — If the embedding model changes (different `text-embedding-3-small` version or switch to a different model), the vector index dimension changes. The pipeline should pass dimension through config, not hardcode it.
- **Convention merge conflicts** — Two consecutive runs might detect contradictory conventions. The pipeline has no conflict resolution strategy yet — last-write-wins is acceptable for MVP, flagged for S05/S06.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust async patterns | `wshobson/agents@rust-async-patterns` (4K installs) | available — not critical for this work |
| serde JSON | none found | not needed — well-understood |
| HNSW / vector search | none found | S01 already solved this |

No skills are needed for this slice — the work is standard Rust async trait design + JSON serde + test fixtures.

## Sources

- `docs/specs/memory-layer.md` §4.4 "Memory Extractor" — extraction pipeline spec with prompt templates
- `crates/ath-agents/src/backend.rs` — `AgentBackend` trait pattern for async LLM abstraction
- `crates/ath-types/src/agent.rs` — `AgentRequest.json_schema` field confirming structured output support
- S01-SUMMARY — `MemoryIndex` API constraints (`&mut self`, `SearchResult` not `MemoryHit`, persist format)
- S02-SUMMARY — `ObservationReader::read_run()` entry point, `ObservationType` serde format
- `.gsd/DECISIONS.md` — D007 (HNSW rebuild on load), D008 (file-per-run JSONL), D009 (ath-memory → ath-types dep)
