---
id: T01
parent: S04
milestone: M002
provides:
  - ContextInjector component in ath-memory for budget-constrained prompt context assembly
  - InjectionConfig with per-section token budgets (total=4000, project=200, semantic=2500, agent=300, run=500)
  - InjectedContext struct with text, estimated_tokens, sections_included
  - estimate_tokens and truncate_to_budget helpers
key_files:
  - crates/ath-memory/src/inject/mod.rs
  - crates/ath-memory/src/inject/injector.rs
  - crates/ath-memory/src/lib.rs
key_decisions:
  - No InjectionError variant needed — injector is fully fail-soft, returns InjectedContext always
  - Token estimation uses word_count * 4 / 3 heuristic — accurate enough for M002, swappable later
  - Recent run picked by lexicographic sort of viking://runs/*/summary URIs (last = most recent)
  - Agent notes found via keyword search for entries matching agents/*/profile pattern
patterns_established:
  - Fail-soft section assembly — each section independently tries store read, logs warn on failure, produces empty on missing
  - XML wrapper format with <athena_context> outer tag and named inner section tags (<project>, <relevant_context>, <agent_notes>, <recent_run>)
  - Section budget capping — each section capped to min(section_budget, remaining_total_budget), then truncated at word boundaries
observability_surfaces:
  - tracing::info "Context built" with tokens and sections_included fields
  - tracing::warn "Context section skipped" with error and URI context on store read failures
  - InjectedContext.sections_included and .estimated_tokens for programmatic inspection
duration: 20m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Build ContextInjector in ath-memory

**Built `ContextInjector` that assembles budget-constrained XML context from VikingStore + KeywordIndex with 4 sections (project identity, semantic results, agent notes, recent run), fail-soft on all reads.**

## What Happened

Created the `inject` submodule in `ath-memory` with `ContextInjector`, `InjectedContext`, and `InjectionConfig`. The injector reads from `VikingStore` and `KeywordIndex` to assemble a prompt context string within configurable token budgets.

Four sections are assembled in order: project identity (`viking://project/identity`), semantic search results (keyword hits joined with store reads), agent notes (best keyword match for `agents/*/profile`), and recent run summary (last `viking://runs/*/summary` by sort order). Each section is truncated to its per-section budget and capped against the remaining total budget. Missing URIs or store read errors produce empty sections with `tracing::warn` — the injector never returns an error.

Output is XML-wrapped with `<athena_context>` outer tag and named inner tags per the spec §4.5 format. Token counting uses the `words * 4 / 3` heuristic.

## Verification

- `cargo test -p ath-memory -- inject` — 13/13 tests pass:
  - empty_store_produces_empty_context
  - project_identity_section_populated
  - keyword_search_results_in_semantic_section
  - total_budget_respected
  - per_section_budget_enforced_oversized_content_truncated
  - missing_uris_produce_empty_sections_fail_soft
  - estimate_tokens_accuracy
  - truncate_to_budget_preserves_word_boundaries
  - truncate_to_budget_returns_full_text_within_budget
  - truncate_to_budget_zero_budget
  - multiple_sections_format_with_xml_wrapper
  - agent_notes_found_via_keyword_search
  - recent_run_section_from_store
- `cargo check --workspace` — clean (only pre-existing warnings in observe/buffer.rs)

Slice-level verification (partial — T01 is first task):
- ✅ `cargo test -p ath-memory -- inject` — all pass
- ⏳ `cargo test -p ath-orchestrator` — T02/T03 scope
- ⏳ `cargo test -p ath-orchestrator -- memory` — T03 scope
- ✅ `cargo check --workspace` — clean

## Diagnostics

- `ContextInjector` emits `tracing::info` with `tokens` and `sections` fields on "Context built" — grep for this in tracing output
- Store read failures emit `tracing::warn` with "Context section skipped" — grep to find why a section is empty
- `InjectedContext.sections_included` lists which sections populated; `.estimated_tokens` gives the total estimate
- `InjectedContext.is_empty()` returns true when no sections were included (empty store or no matches)

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/src/inject/mod.rs` — module root with re-exports
- `crates/ath-memory/src/inject/injector.rs` — ContextInjector, InjectedContext, InjectionConfig, helpers, 13 tests
- `crates/ath-memory/src/lib.rs` — added `pub mod inject` and re-exports for ContextInjector, InjectedContext, InjectionConfig
