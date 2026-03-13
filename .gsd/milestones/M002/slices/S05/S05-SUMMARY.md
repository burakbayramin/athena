---
id: S05
parent: M002
milestone: M002
provides:
  - MemoryConfig with TOML deserialization and graceful defaults (missing/malformed file → defaults)
  - GcConfig struct with configurable retention_days (default 30)
  - "ath memory tree — lists store entries with box-drawing tree format"
  - "ath memory search <query> — keyword search with score display and --top flag"
  - "ath memory read <uri> — displays L0/L1/L2 layers of a Viking entry"
  - "ath memory add <uri> <content> — writes to store AND keyword index"
  - "ath memory stats — entry count, index size, observation count, disk usage"
  - "ath memory gc --days N — deletes old observation .jsonl files"
  - MemoryError wired into actionable_hint() in CLI error display
requires:
  - slice: S01
    provides: VikingStore (list/read/write/delete), KeywordIndex (search/add/save/load)
  - slice: S02
    provides: Observation .jsonl file format (for gc and stats)
  - slice: S03
    provides: ExtractionConfig (deserialization added here)
affects:
  - S06
key_files:
  - crates/ath-memory/src/config.rs
  - crates/ath-cli/src/memory.rs
  - crates/ath-cli/src/main.rs
key_decisions:
  - "D017: Added serde::Deserialize directly to InjectionConfig and ExtractionConfig — no intermediate mapping structs"
  - "D018: gc only targets .jsonl files in observations/ — non-jsonl files ignored for safety"
  - "D019: Stats uses direct filesystem walks rather than adding methods to VikingStore — keeps store API clean"
  - "D020: Used filetime crate for backdating file mtime in gc tests — std has no stable cross-platform set_modified"
patterns_established:
  - "Config load pattern: try read → Default on missing → warn+Default on parse error"
  - "CLI subcommand pattern: MemoryArgs wraps MemoryCommands enum, dispatch via memory_command()"
  - "cmd_gc pattern: list dir → filter .jsonl → check mtime → delete + accumulate stats"
  - "format_bytes helper: B/KB/MB tiers with 1-decimal formatting"
observability_surfaces:
  - "tracing::warn on config parse failures (grep 'failed to parse config file')"
  - "MemoryError::hint() surfaced through display_error Fix: line"
  - "ath memory stats — primary health-check command"
  - "ath memory gc — reports files deleted and bytes freed"
  - "Empty store/index produce informative messages ('(empty store)', '(no keyword index found)', 'No results')"
drill_down_paths:
  - .gsd/milestones/M002/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S05/tasks/T02-SUMMARY.md
duration: ~35m
verification_result: passed
completed_at: 2026-03-14
---

# S05: CLI & Config

**`ath memory` CLI with 6 subcommands (tree/search/read/add/stats/gc) and TOML-based memory config with graceful defaults.**

## What Happened

T01 built the foundation: `MemoryConfig` in `ath-memory` wrapping `InjectionConfig`, `ExtractionConfig`, and `GcConfig` — all `#[serde(default)]` for graceful partial/missing configs. Added `Deserialize` derive to the upstream config structs directly. Created `memory.rs` in `ath-cli` with clap `MemoryArgs`/`MemoryCommands` and four subcommands: `tree` (store entries grouped by directory with box-drawing chars), `search` (keyword index lookup with score display), `read` (URI → L0/L1/L2 layer display), `add` (write to store AND update keyword index). Wired `Memory(MemoryArgs)` into the CLI `Commands` enum and added `MemoryError` to `actionable_hint()`.

T02 added `stats` (entry count, index size, observation count, disk usage via filesystem walk) and `gc` (delete `.jsonl` observations older than configurable retention period, `--days` flag). Added `filetime` workspace dependency for mtime manipulation in gc tests. All 6 subcommands handle missing directories gracefully with informative messages rather than errors.

## Verification

- `cargo test -p ath-memory -- config` — 7 tests pass (full TOML, partial, empty, missing file, malformed, valid, GcConfig defaults)
- `cargo test -p ath-cli -- memory` — 21 tests pass (all 6 subcommands with happy path, empty state, error cases, edge cases)
- `cargo check --workspace` — clean (only pre-existing dead_code warnings in observe buffer)
- `cargo test --workspace` — all tests pass, zero failures, zero regressions
- Malformed config.toml → `tracing::warn` + defaults (not panic) — verified by unit test
- MemoryError hints surface via `display_error` — verified by 2 CLI tests asserting hint text

## Deviations

None.

## Known Limitations

- `gc` uses file modification time, not embedded timestamps — manual file copies could have unexpected mtime
- `stats` disk usage counts all files under `.ath/memory/`, including index files — not broken down by category
- No `--format json` output mode for any subcommand (CLI output is human-readable only)

## Follow-ups

- S06 end-to-end integration test is the remaining work before the milestone is complete

## Files Created/Modified

- `crates/ath-memory/src/config.rs` — new: MemoryConfig, GcConfig, TOML loading, 7 unit tests
- `crates/ath-memory/src/lib.rs` — added `pub mod config` and re-exports
- `crates/ath-memory/Cargo.toml` — added `toml.workspace = true`
- `crates/ath-memory/src/inject/injector.rs` — added `Deserialize` + `#[serde(default)]` to InjectionConfig
- `crates/ath-memory/src/extract/types.rs` — added `Deserialize` + `#[serde(default)]` to ExtractionConfig
- `crates/ath-cli/src/memory.rs` — new: all 6 subcommands, helpers, 21 tests
- `crates/ath-cli/src/main.rs` — added mod memory, Memory variant, MemoryError in actionable_hint, 2 error tests
- `crates/ath-cli/Cargo.toml` — added `ath-memory` dependency, `filetime` dev-dependency
- `Cargo.toml` — added `filetime` workspace dependency

## Forward Intelligence

### What the next slice should know
- All 6 `ath memory` subcommands work on real store data — S06 can use them to verify end-to-end memory lifecycle
- `MemoryConfig::load(root)` gracefully defaults when config is missing — S06 doesn't need to create config files for basic operation
- The `memory_command()` function in `ath-cli/src/memory.rs` returns `Result<String>` with the formatted output — easy to capture for assertions

### What's fragile
- `gc` depends on filesystem mtime accuracy — on filesystems with coarse timestamps, files near the threshold might behave unpredictably
- `tree` formatting assumes URI paths with at most one directory level for clean display — deeply nested paths will render flat under their top-level prefix

### Authoritative diagnostics
- `ath memory stats` — single command to verify memory store health (entry count, index size, observation count, disk usage)
- `cargo test -p ath-cli -- memory` — 21 tests covering all subcommands, the most comprehensive verification surface

### What assumptions changed
- No assumptions changed — S05 was low-risk and delivered as planned
