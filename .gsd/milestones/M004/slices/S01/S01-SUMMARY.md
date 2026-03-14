---
id: S01
parent: M004
milestone: M004
provides:
  - AgentId struct replacing AgentKind enum across all 8 crates
  - Backward-compatible serde (old {"Claude":"opus-4"} deserializes into AgentId)
  - Helper constructors claude(), gemini(), codex() and detection methods is_claude(), is_gemini(), is_codex()
  - AgentKind deprecated type alias for migration
  - AgentRegistry keyed by provider string instead of discriminant
requires: []
affects:
  - S02
  - S03
  - S04
  - S05
key_files:
  - crates/ath-types/src/agent.rs
  - crates/ath-orchestrator/src/phase_runner.rs
  - crates/ath-orchestrator/src/router.rs
  - crates/ath-orchestrator/src/review.rs
  - crates/ath-orchestrator/src/taxonomy.rs
  - crates/ath-cli/src/cost.rs
key_decisions:
  - "D026: AgentId stores provider as lowercase string, model as-is — provider matching is case-insensitive"
  - "D027: AgentRegistry keyed by provider string (not discriminant) — extensible to any number of providers"
  - "D028: Custom serde with visit_map handles both legacy enum format and new struct format in single deserializer"
  - "D029: priority() returns 3 for custom providers — lowest priority, built-in providers keep 0/1/2"
patterns_established:
  - "AgentId::claude/gemini/codex constructors for built-in providers"
  - "is_claude()/is_gemini()/is_codex() for provider-specific branching (replaces pattern matching)"
  - "agent.provider() for grouping/keying (replaces mem::discriminant)"
observability_surfaces:
  - "AgentId::Display shows 'Provider/model' format"
drill_down_paths:
  - .gsd/milestones/M004/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T02-SUMMARY.md
duration: 45m
verification_result: passed
completed_at: 2026-03-14
---

# S01: AgentId Type Refactor

**Replaced `AgentKind` enum with `AgentId` struct across all 8 crates — ~370 occurrences in 37 files migrated, backward-compatible serde, 624 tests pass (12 new).**

## What Happened

**T01** defined `AgentId` in ath-types as a struct with `provider` and `model` strings. Custom serde deserializer handles both the legacy tagged enum format (`{"Claude":"opus-4"}`) and the new struct format (`{"provider":"anthropic","model":"opus-4"}`). Added convenience constructors, `is_*()` detection methods, Display, Hash, Eq. `AgentKind` kept as deprecated type alias.

**T02** performed the mass migration. Replaced all enum constructors with method constructors, all pattern matches with `is_*()` checks, all `mem::discriminant` grouping with `provider()` string keys. The `AgentRegistry` HashMap key changed from `Discriminant<AgentKind>` to `String` (provider name). Review's majority-author selection changed from discriminant comparison to provider string comparison. Cost estimation's match destructuring converted to if/else chains.

## Verification

- `cargo test --workspace` — 624 passed, 0 failed (612 existing + 12 new)
- `cargo check --workspace` — clean (only pre-existing dead_code warnings)
- Zero remaining `AgentKind::` enum constructor usages in codebase
- Backward compat: `{"Claude":"opus-4"}` deserializes into `AgentId { provider: "anthropic", model: "opus-4" }`
- Round-trip: serialize → deserialize produces equal AgentId for both built-in and custom providers

## Deviations

None.

## Files Created/Modified

37 files across 8 crates. Key changes:
- `crates/ath-types/src/agent.rs` — AgentId struct, custom serde, 12 new tests
- `crates/ath-types/src/lib.rs` — updated re-exports
- `crates/ath-types/src/{phase,plan,report,review}.rs` — AgentKind → AgentId in types
- `crates/ath-orchestrator/src/phase_runner.rs` — AgentRegistry keyed by provider string
- `crates/ath-orchestrator/src/router.rs` — provider string grouping instead of discriminant
- `crates/ath-orchestrator/src/review.rs` — provider string for majority counting
- `crates/ath-orchestrator/src/taxonomy.rs` — if/else instead of match for priority
- `crates/ath-cli/src/cost.rs` — if/else pricing instead of match destructuring
- All other files: mechanical AgentKind→AgentId renaming

## Forward Intelligence

### What the next slice should know
- `AgentId::new("ollama", "llama3.3")` creates a custom provider agent. It works everywhere — the type system is now fully extensible.
- `AgentRegistry::register` uses `provider()` as key — registering two Claude models replaces the first. This is intentional (one backend per provider).
- `AgentKind` is a deprecated type alias. It compiles but triggers warnings. Remove it after S02-S05 are done if no external consumers need it.

### What's fragile
- The deprecated `AgentKind` type alias suppresses deprecation warnings with `#[allow(deprecated)]` in lib.rs. Tests that use it will see warnings.
- `priority()` hardcodes built-in providers at 0/1/2 and custom at 3. If a user wants their custom provider to have higher priority than Codex, they can't without config (addressed in S03).
