---
estimated_steps: 5
estimated_files: 5
---

# T01: Build ContextInjector in ath-memory

**Slice:** S04 — Orchestrator Integration (Observation + Injection)
**Milestone:** M002

## Description

Build the `ContextInjector` component in `ath-memory` that reads from `VikingStore` and `KeywordIndex` to construct a budget-constrained context string for agent prompts. This is a pure `ath-memory` component with no orchestrator dependency — proven independently before wiring.

The injector uses a section-based approach: project identity, semantic search results, agent notes, and recent run summary. Each section has a configurable token budget allocation. Token counting uses a word-count heuristic (`words * 4 / 3`) — good enough for M002, swappable later.

## Steps

1. Create `crates/ath-memory/src/inject/mod.rs` with module declarations and re-exports.

2. Create `crates/ath-memory/src/inject/injector.rs` with:
   - `InjectionConfig` struct with per-section token budgets (defaults: total=4000, project_identity=200, semantic_results=2500, agent_notes=300, recent_run=500). Include `From`/`Default` impls.
   - `InjectedContext` struct with `text: String`, `estimated_tokens: usize`, `sections_included: Vec<String>`. Add `is_empty()` method.
   - `ContextInjector` struct holding `&VikingStore` and `&KeywordIndex` references.
   - `ContextInjector::build_context(&self, query: &str, config: &InjectionConfig) -> InjectedContext` method:
     - Try reading `viking://project/identity` for project section (fail-soft: empty on missing)
     - Run `KeywordIndex::search(query, 10)` and join hits with `VikingStore::read()` for semantic section
     - Try reading `viking://agents/*/profile` for agent notes (single best match from keyword search)
     - Try reading most recent `viking://runs/*/summary` for recent run section
     - Each section truncated to its budget allocation using `estimate_tokens()` helper
     - Sections formatted with `<athena_context>` XML wrapper per spec
   - `estimate_tokens(text: &str) -> usize` helper: `text.split_whitespace().count() * 4 / 3`
   - `truncate_to_budget(text: &str, max_tokens: usize) -> String` helper: takes words until budget hit

3. Add `pub mod inject;` to `crates/ath-memory/src/lib.rs` and re-export `ContextInjector`, `InjectedContext`, `InjectionConfig`.

4. Add `InjectionError` variant to `MemoryError` in `error.rs` if needed — or confirm that fail-soft approach means no new error variant is needed (injector returns `InjectedContext` always, never `Result`).

5. Write unit tests covering:
   - Empty store produces empty context (`is_empty() == true`)
   - Project identity section populated from store
   - Keyword search results included in semantic section
   - Token budget respected (total never exceeded)
   - Per-section budgets enforced (oversized content truncated)
   - Missing URIs produce empty sections (fail-soft, no panic)
   - `estimate_tokens` accuracy on sample texts
   - `truncate_to_budget` preserves word boundaries
   - Multiple sections format correctly with XML wrapper

## Must-Haves

- [ ] `ContextInjector` builds context from `VikingStore` + `KeywordIndex`
- [ ] Token budget enforced at both section and total level
- [ ] Missing/empty store entries produce empty sections (never error)
- [ ] `InjectedContext` reports `estimated_tokens` and `sections_included`
- [ ] ≥8 unit tests pass

## Verification

- `cargo test -p ath-memory -- inject` — all tests pass
- `cargo check --workspace` — clean

## Observability Impact

- **Signals added:** `tracing::info` in `build_context` logging tokens injected and sections included (grep: "Context built"). `tracing::warn` on store read failures during injection (grep: "Context section skipped"). These signals let a future agent diagnose why context was empty or incomplete.
- **Inspection:** `InjectedContext.sections_included` reports which sections were populated. `InjectedContext.estimated_tokens` reports total token estimate. Both are available programmatically without log parsing.
- **Failure visibility:** All `VikingStore::read` and `KeywordIndex::search` errors are swallowed with `tracing::warn` — the injector never propagates errors. A future agent can grep for "Context section skipped" to find cases where store reads failed.

## Inputs

- `crates/ath-memory/src/store.rs` — `VikingStore::read(&VikingUri) -> Result<Option<LayeredContent>>`, `VikingStore::list() -> Result<Vec<VikingUri>>`
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::search(&self, query, top_k) -> Vec<KeywordHit>` with `KeywordHit { uri_str, score }`
- `docs/specs/memory-layer.md` §4.5 — injection strategy spec, §10 — token budget defaults
- S01 Summary — `VikingStore` is `&self` for reads, `KeywordIndex::search` is `&self`

## Expected Output

- `crates/ath-memory/src/inject/mod.rs` — module root with re-exports
- `crates/ath-memory/src/inject/injector.rs` — `ContextInjector`, `InjectedContext`, `InjectionConfig`, helpers, tests
- `crates/ath-memory/src/lib.rs` — updated with `pub mod inject` and re-exports
- `crates/ath-memory/src/error.rs` — updated only if a new variant is needed
