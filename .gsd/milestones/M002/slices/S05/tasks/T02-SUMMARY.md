---
id: T02
parent: S05
milestone: M002
provides:
  - "`ath memory stats` subcommand showing entry count, index size, observation count, disk usage"
  - "`ath memory gc --days N` subcommand deleting old observation .jsonl files"
  - "format_bytes and dir_disk_usage helpers"
key_files:
  - crates/ath-cli/src/memory.rs
  - crates/ath-cli/Cargo.toml
  - Cargo.toml
key_decisions:
  - "Used filetime crate for backdating file modification times in gc tests — std has no stable set_modified API on all platforms"
  - "gc only targets .jsonl files in observations/ — non-jsonl files are explicitly ignored for safety"
  - "Stats command uses direct filesystem walks rather than adding methods to VikingStore — keeps the store API clean"
patterns_established:
  - "cmd_gc pattern: list dir → filter by extension → check mtime against threshold → delete + accumulate stats"
  - "format_bytes helper uses B/KB/MB tiers with 1-decimal formatting"
observability_surfaces:
  - "`ath memory stats` — primary health-check command for memory store"
  - "`ath memory gc` — reports files deleted and bytes freed"
  - "Missing observations dir produces clear message rather than error"
duration: ~15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Stats, GC subcommands and full verification

**Added `ath memory stats` (health-check) and `ath memory gc` (maintenance) subcommands, completing all 6 CLI subcommands with 21 passing tests and zero workspace regressions.**

## What Happened

Added `Stats` and `Gc { days: u32 }` variants to `MemoryCommands`. Implemented `cmd_stats` which counts store entries via `VikingStore::list()`, keyword index entries via `KeywordIndex::load().len()`, observation `.jsonl` files by scanning the observations directory, and total disk usage via recursive walk of `.ath/memory/`. All counts gracefully default to zero when directories or files don't exist.

Implemented `cmd_gc` which scans `.ath/memory/observations/` for `.jsonl` files, compares each file's modification time against a `now - days * 86400` threshold, and deletes files that exceed the retention period. Reports count deleted and bytes freed. Missing observations directory produces "No observations to clean" rather than an error.

Added supporting helpers: `count_jsonl_files`, `dir_disk_usage`, `format_stats`, `format_bytes`.

Wrote 8 new tests covering stats (populated store, empty store, nonexistent dir), gc (deletes old files, keeps recent, handles missing dir, ignores non-jsonl), and format_bytes units.

Added `filetime` as a workspace dependency for backdating file modification times in gc tests.

## Verification

- `cargo test -p ath-cli -- memory` — 21 tests pass (13 from T01 + 8 new)
- `cargo test -p ath-memory -- config` — 7 tests pass
- `cargo test --workspace` — all tests pass, zero failures
- `cargo check --workspace` — clean (only pre-existing dead_code warnings in ath-memory observe buffer)

### Slice-level verification status (all pass — final task):
- ✅ `cargo test -p ath-memory -- config` — 7 pass
- ✅ `cargo test -p ath-cli -- memory` — 21 pass (all 6 subcommands tested)
- ✅ `cargo test --workspace` — no regressions
- ✅ `cargo check --workspace` — clean
- ✅ Malformed config.toml returns defaults with warn (verified by T01 test)
- ✅ MemoryError hints surface via display_error (verified by T01 test)

## Diagnostics

- Run `ath memory stats` to see store health: entry count, index size, observation count, disk usage
- Run `ath memory gc --days 30` to clean old observations; output reports deletions
- Missing observations directory prints "No observations to clean" — not an error
- Stats handles missing store/index/observations directories gracefully with zero counts

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-cli/src/memory.rs` — Added Stats + Gc variants, cmd_stats + cmd_gc implementations, helper functions, and 8 new tests
- `crates/ath-cli/Cargo.toml` — Added filetime dev-dependency
- `Cargo.toml` — Added filetime workspace dependency
- `.gsd/milestones/M002/slices/S05/tasks/T02-PLAN.md` — Added Observability Impact section
