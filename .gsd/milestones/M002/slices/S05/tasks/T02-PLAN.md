---
estimated_steps: 4
estimated_files: 1
---

# T02: Stats, GC subcommands and full verification

**Slice:** S05 — CLI & Config
**Milestone:** M002

## Description

Add the two operational subcommands (`stats` and `gc`) to `ath memory` and run full workspace verification. These commands provide health-check and maintenance capabilities for the memory store.

## Steps

1. Add `Stats` and `Gc { #[arg(long, default_value_t = 30)] days: u32 }` variants to `MemoryCommands` in `memory.rs`. Implement `cmd_stats` — count entries from `VikingStore::list().len()`, count keyword index entries from `KeywordIndex::load().map(|i| i.len()).unwrap_or(0)`, count `.jsonl` files in `.ath/memory/observations/`, calculate disk usage by walking `.ath/memory/` recursively summing file sizes. Format as a compact summary with labels.

2. Implement `cmd_gc` — list `.jsonl` files in `.ath/memory/observations/`, read each file's modification time via `fs::metadata`, compare against `now - days * 86400`, delete files older than threshold. Report count deleted and bytes freed. If observations directory doesn't exist, print "No observations to clean" and return Ok.

3. Write tests for `stats`: create temp dir with known store entries (via VikingStore::write), a keyword index (via KeywordIndex::save), and observation files (create dummy .jsonl files), call `cmd_stats` and verify output contains expected counts. Write tests for `gc`: create observation files with backdated modification times (use `filetime` crate or just check the logic handles the threshold correctly — test the filtering logic with known dates), verify only old files are deleted and recent ones survive.

4. Run `cargo test -p ath-cli -- memory` (all subcommands), `cargo test -p ath-memory -- config`, and `cargo test --workspace`. Fix any regressions.

## Must-Haves

- [ ] `ath memory stats` shows entry count, index size, observation count, disk usage
- [ ] `ath memory gc --days N` deletes observation files older than N days
- [ ] `gc` handles missing observations directory gracefully
- [ ] Tests for stats verify correct counts against known test data
- [ ] Tests for gc verify only old files are deleted
- [ ] Full workspace test suite passes with zero regressions

## Verification

- `cargo test -p ath-cli -- memory` — all 6 subcommands tested, all pass
- `cargo test --workspace` — zero failures
- `cargo check --workspace` — clean

## Inputs

- `crates/ath-cli/src/memory.rs` — existing module from T01 with tree/search/read/add
- `crates/ath-memory/src/store.rs` — `VikingStore::list()` for entry counting
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::load/len` for index size
- `.ath/memory/observations/` directory layout from S02

## Observability Impact

- **`ath memory stats`**: New health-check surface — shows entry count, keyword index size, observation count, and disk usage. A future agent can run this to assess store health before/after operations.
- **`ath memory gc`**: Reports files deleted and bytes freed. A future agent inspects gc results by running the command and reading the summary line.
- **Failure visibility**: Missing observations directory produces a clear message ("No observations to clean") rather than an error. Stats handles missing index/observations gracefully with zero counts.

## Expected Output

- `crates/ath-cli/src/memory.rs` — updated with `Stats` + `Gc` variants, `cmd_stats` + `cmd_gc` implementations, and tests for all 6 subcommands
