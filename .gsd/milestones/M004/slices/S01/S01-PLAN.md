# S01: AgentId Type Refactor

**Goal:** `AgentKind` enum replaced by `AgentId` struct across all 8 crates — backward-compatible serde, all tests pass
**Demo:** All 612 tests pass with `AgentId` instead of `AgentKind`. Old serialized `AgentKind` JSON deserializes into `AgentId`.

## Must-Haves

- `AgentId` struct with `provider: String`, `model: String`
- `provider_name()` and `model()` methods matching existing API
- Backward-compatible Deserialize (old `{"Claude":"opus-4"}` → `AgentId { provider: "anthropic", model: "opus-4" }`)
- Clean Serialize (new format: `{"provider":"anthropic","model":"opus-4"}`)
- Helper constructors: `AgentId::claude(m)`, `AgentId::gemini(m)`, `AgentId::codex(m)`
- `is_claude()`, `is_gemini()`, `is_codex()` methods for code that needs provider-specific behavior
- `Hash + Eq` for use as HashMap keys
- All ~370 occurrences across 37 files updated
- All 612 tests pass

## Proof Level

- This slice proves: contract (all tests pass with new type)
- Real runtime required: no
- Human/UAT required: no

## Verification

- `cargo test --workspace` — all tests pass (612+)
- `cargo check --workspace` — clean
- Backward compat serde test: old JSON → AgentId → correct provider/model

## Tasks

- [x] **T01: Define AgentId in ath-types with backward-compatible serde** `est:20m`
  - Why: The type definition with serde compat is the foundation — everything else depends on it
  - Files: `crates/ath-types/src/agent.rs`, `crates/ath-types/src/lib.rs`
  - Do:
    1. Add `AgentId` struct alongside existing `AgentKind` (don't remove yet)
    2. Implement Display, Hash, Eq, Clone, Debug
    3. Custom Deserialize that handles both old enum format and new struct format
    4. Serialize as `{"provider":"...","model":"..."}`
    5. Helper constructors and `is_*()` methods
    6. `impl From<AgentKind> for AgentId` for gradual migration
    7. Add comprehensive tests
  - Verify: `cargo test -p ath-types`
  - Done when: AgentId exists with serde compat proven by tests

- [x] **T02: Replace AgentKind with AgentId across all crates** `est:60m`
  - Why: The actual mass migration — ~370 occurrences across 37 files
  - Files: all 37 files listed in the rg output
  - Do:
    1. In ath-types: change all uses of AgentKind to AgentId in AgentRequest, AgentResponse, AgentContribution, ReviewVerdict, plan types
    2. Keep AgentKind as a deprecated type alias or remove entirely
    3. In ath-agents: update backend trait, actor handles, mock
    4. In ath-orchestrator: update taxonomy, router, coordinator, phase_runner, memory, progress, checkpoint, review
    5. In ath-memory: update observation types
    6. In ath-config: no changes needed (just strings)
    7. In ath-git: update commit metadata
    8. In ath-planner: update prompt construction
    9. In ath-cli: update run, progress, cost, verbose, report, dry_run
    10. Fix all pattern matches: `AgentKind::Claude(_)` → `id.is_claude()` or `id.provider() == "anthropic"`
  - Verify: `cargo test --workspace`
  - Done when: zero `AgentKind` usage remains, all tests pass

## Files Likely Touched

All 37 files from the rg output.
