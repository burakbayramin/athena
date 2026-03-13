---
id: T01
parent: S01
milestone: M002
provides:
  - ath-memory crate scaffold with core types
  - VikingUri parser with traversal protection
  - LayeredContent and MemoryHit structs
  - MemoryError enum with hint() method
key_files:
  - crates/ath-memory/Cargo.toml
  - crates/ath-memory/src/lib.rs
  - crates/ath-memory/src/uri.rs
  - crates/ath-memory/src/types.rs
  - crates/ath-memory/src/error.rs
  - Cargo.toml
key_decisions:
  - VikingUri uses custom Serialize/Deserialize (string representation) rather than deriving on struct fields, so URIs serialize as "viking://project/conventions" not as a segments array
  - Path traversal rejected both at parse time (.. segments) and at resolve time (normalized path escapes root)
  - MemoryError::from(io::Error) provides a blanket conversion with empty path for ergonomic ? usage; callers needing path context should construct IoError directly
patterns_established:
  - VikingUri FromStr/Display/Serialize/Deserialize pattern for use across store and index modules
  - MemoryError with hint() following ath-config's ConfigError pattern
  - LayeredContent builder pattern (new + with_detail)
observability_surfaces:
  - MemoryError::hint() returns actionable resolution strings for all 6 variants
  - Error Display impls embed contextual fields (URI, path, expected/actual dimensions)
  - All error variants tested for hint content and display output
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Scaffold ath-memory crate with VikingUri and LayeredContent types

**Built `ath-memory` crate with VikingUri parser (traversal-protected), LayeredContent/MemoryHit types, and MemoryError enum — 23 tests + 1 doc-test pass, workspace clean.**

## What Happened

Created the `ath-memory` crate following workspace conventions. VikingUri parses `viking://` URIs, validates segments (no `..`, no empty, no `.`), and resolves to filesystem paths with traversal protection via path normalization. LayeredContent holds L0/L1/L2 text layers with timestamps and serializes through serde_json. MemoryHit wraps a search result with score. MemoryError covers all 6 planned variants with contextual fields and actionable hints. Crate wired into workspace root as a workspace dependency.

## Verification

- `cargo test -p ath-memory` — 23 unit tests + 1 doc-test pass
- `cargo check --workspace` — all 8 workspace crates compile cleanly
- URI tests cover: valid simple/deep/single-segment parse, scheme rejection, traversal rejection, empty segment rejection, dot segment rejection, trailing slash rejection, no-path rejection, resolve_path within root, serde round-trip, serde rejection of invalid URIs
- LayeredContent round-trips through serde_json with and without L2 detail
- MemoryHit round-trips through serde_json
- All 6 MemoryError hints verified as non-empty and actionable (>20 chars)

### Slice-Level Verification (partial — T01 of 3)

- ✅ `cargo test -p ath-memory` — all tests pass
- ✅ `cargo check --workspace` — no regressions
- ✅ URI parsing (valid/invalid/traversal)
- ✅ `MemoryError::hint()` actionable for all variants
- ✅ Path traversal produces `MemoryError::PathTraversal` with URI in message
- ⬜ Store round-trip (T02)
- ⬜ Store overwrite, missing entry handling (T02)
- ⬜ HNSW insert→search→persist→reload→search (T03)
- ⬜ Keyword fallback search (T03)
- ⬜ Dimension mismatch rejection (T03)
- ⬜ Empty index search (T03)

## Diagnostics

- Inspect error behavior: `cargo test -p ath-memory -- error` runs all error tests
- Inspect URI parsing: `cargo test -p ath-memory -- uri` runs all URI tests
- MemoryError Display output embeds the offending URI/path/dimensions for log-level diagnosis
- MemoryError::hint() can be called on any error to get user-facing resolution guidance

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/Cargo.toml` — new crate manifest with workspace deps
- `crates/ath-memory/src/lib.rs` — crate root with module declarations and re-exports
- `crates/ath-memory/src/uri.rs` — VikingUri parser with FromStr/Display/Serialize/Deserialize and 14 tests
- `crates/ath-memory/src/types.rs` — LayeredContent and MemoryHit structs with 3 serde tests
- `crates/ath-memory/src/error.rs` — MemoryError enum with 6 variants, hint(), From<io::Error>, and 6 tests
- `Cargo.toml` — added ath-memory workspace dependency
- `.gsd/milestones/M002/slices/S01/S01-PLAN.md` — added failure-path verification steps
- `.gsd/milestones/M002/slices/S01/tasks/T01-PLAN.md` — added Observability Impact section
