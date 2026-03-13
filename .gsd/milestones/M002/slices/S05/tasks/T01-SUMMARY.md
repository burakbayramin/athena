---
id: T01
parent: S05
milestone: M002
provides:
  - MemoryConfig with TOML deserialization and graceful defaults
  - GcConfig struct with retention_days default 30
  - ath memory tree/search/read/add CLI subcommands
  - MemoryError wired into actionable_hint()
key_files:
  - crates/ath-memory/src/config.rs
  - crates/ath-cli/src/memory.rs
  - crates/ath-cli/src/main.rs
key_decisions:
  - Added serde::Deserialize directly to InjectionConfig and ExtractionConfig rather than using intermediate structs — simpler, no mapping layer needed
  - Tree format uses BTreeMap for deterministic alphabetical ordering of top-level directories
  - cmd_add stores user content as abstract_text with empty overview — matches LayeredContent::new signature
patterns_established:
  - Config load pattern: try read → return Default on missing file → warn+Default on parse error
  - CLI subcommand pattern: MemoryArgs wraps MemoryCommands enum, dispatch via memory_command()
observability_surfaces:
  - tracing::warn on config parse failures (grep "failed to parse config file")
  - MemoryError::hint() surfaced through display_error Fix: line
  - Empty store/index produce informative messages ("(empty store)", "(no keyword index found)", "No results")
duration: 20m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Config schema, tree/search/read/add subcommands, and CLI wiring

**Built MemoryConfig with TOML loading, four `ath memory` subcommands (tree/search/read/add), and wired MemoryError into CLI error display.**

## What Happened

Added `toml` dependency to ath-memory and created `config.rs` with `MemoryConfig` wrapping `InjectionConfig`, `ExtractionConfig`, and new `GcConfig` — all `#[serde(default)]` for graceful partial configs. Added `Deserialize` derive to `InjectionConfig` and `ExtractionConfig` (they only had `Debug, Clone`).

Created `memory.rs` in ath-cli with clap `MemoryArgs`/`MemoryCommands` and four subcommand implementations:
- `tree` — lists store entries grouped by top-level directory with box-drawing characters
- `search` — loads keyword index, searches, joins with store for abstract display
- `read` — parses URI, reads entry, displays all three layers
- `add` — writes to store AND updates keyword index

Wired `Memory(MemoryArgs)` into the `Commands` enum, added sync dispatch arm, and added `MemoryError` to `actionable_hint()`.

## Verification

- `cargo test -p ath-memory -- config` — 7 tests pass (full TOML, partial, empty, missing file, malformed file, valid file, GcConfig defaults)
- `cargo test -p ath-cli -- memory` — 13 tests pass (tree shows entries, tree empty, tree format structure, search finds results, search no results, search missing index, read displays content, read nonexistent, read with detail, add writes to store and index, add updates existing index, memory_error_shows_actionable_hint, memory_io_error_shows_fix_hint)
- `cargo check --workspace` — clean (no new warnings)
- `cargo test --workspace` — all 580+ tests pass, zero regressions

## Diagnostics

- Config issues: grep tracing output for `"failed to parse config file"` — warns with error details, returns defaults
- MemoryError in CLI: any memory error through `display_error` shows `Fix:` with actionable guidance
- Empty states: tree shows `(empty store)`, search shows `(no keyword index found)` or `No results for '<query>'`

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/src/config.rs` — new: MemoryConfig, GcConfig, TOML loading, 7 unit tests
- `crates/ath-memory/src/lib.rs` — added `pub mod config` and re-exports for MemoryConfig, GcConfig
- `crates/ath-memory/Cargo.toml` — added `toml.workspace = true`
- `crates/ath-memory/src/inject/injector.rs` — added `#[derive(serde::Deserialize)]` and `#[serde(default)]` to InjectionConfig
- `crates/ath-memory/src/extract/types.rs` — added `#[derive(serde::Deserialize)]` and `#[serde(default)]` to ExtractionConfig
- `crates/ath-cli/src/memory.rs` — new: MemoryArgs, MemoryCommands, 4 subcommand impls, 11 tests
- `crates/ath-cli/src/main.rs` — added mod memory, Memory variant, MemoryError in actionable_hint, 2 error display tests
- `crates/ath-cli/Cargo.toml` — added `ath-memory` dependency
