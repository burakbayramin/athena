# S05: CLI & Config

**Goal:** `ath memory` subcommands work on real memory data and config loads from `.ath/memory/config.toml`
**Demo:** `ath memory tree` shows store structure, `ath memory search "error"` finds entries, `ath memory add` creates entries, `ath memory stats` shows counts, `ath memory gc` cleans old observations — all driven by CLI integration tests

## Must-Haves

- `MemoryConfig` struct in `ath-memory` with TOML deserialization, wrapping `InjectionConfig`, `ExtractionConfig`, and `GcConfig`
- `MemoryConfig::load(root)` with graceful default when config.toml missing
- `ath memory tree` — lists store entries as a tree with `├── / └──` formatting
- `ath memory search <query>` — keyword search with score display, `--top` flag
- `ath memory read <uri>` — reads and displays a specific Viking entry
- `ath memory add <uri> <content>` — writes entry to store AND keyword index
- `ath memory stats` — shows entry count, index size, observation file count, disk usage
- `ath memory gc` — deletes observation JSONL files older than configurable retention (default 30 days), `--days` flag
- `MemoryError` wired into `actionable_hint()` in `main.rs`
- All subcommands have CLI integration tests

## Verification

- `cargo test -p ath-memory -- config` — config loading tests pass (load, missing file defaults, partial config)
- `cargo test -p ath-cli -- memory` — CLI integration tests pass for all 6 subcommands
- `cargo test --workspace` — no regressions
- `cargo check --workspace` — clean
- Malformed `config.toml` produces `tracing::warn` (not panic/error) and returns defaults — verified by unit test
- `MemoryError` variants surface actionable hints via `display_error` — verified by CLI test asserting hint text appears in rendered output

## Observability / Diagnostics

- Runtime signals: `tracing::instrument` on config load, `tracing::warn` on config parse failures
- Inspection surfaces: `.ath/memory/config.toml` is human-readable TOML, `ath memory stats` is the health-check command
- Failure visibility: `MemoryError::hint()` provides actionable fixes surfaced via `display_error`

## Integration Closure

- Upstream surfaces consumed: `VikingStore` (list/read/write/delete), `KeywordIndex` (search/add/save/load), `ObservationReader`, `InjectionConfig`, `ExtractionConfig` from S01–S03
- New wiring introduced: `ath-memory` dependency in `ath-cli`, `Memory(MemoryArgs)` variant in `Commands` enum, `MemoryError` in `actionable_hint()`
- What remains before the milestone is truly usable end-to-end: S06 end-to-end integration test

## Tasks

- [x] **T01: Config schema, tree/search/read/add subcommands, and CLI wiring** `est:45m`
  - Why: The core deliverable — config loading and the four data-path subcommands that let users inspect and manually add memory entries
  - Files: `crates/ath-memory/src/config.rs`, `crates/ath-memory/src/lib.rs`, `crates/ath-memory/Cargo.toml`, `crates/ath-cli/src/memory.rs`, `crates/ath-cli/src/main.rs`, `crates/ath-cli/Cargo.toml`
  - Do: Add `MemoryConfig` with serde Deserialize wrapping injection/extraction/gc sections, all fields `#[serde(default)]`. Add `config` module to `ath-memory`. Create `memory.rs` in `ath-cli` with `MemoryArgs` (clap Args) containing `MemoryCommands` enum (Subcommand). Implement `tree` (store.list → group by prefix → tree-format), `search` (load keyword index → search → join with store.read for display), `read` (parse URI → store.read → format L0/L1/L2), `add` (parse URI → LayeredContent::new → store.write + keyword_index.add + save). Wire `Memory(MemoryArgs)` into Commands enum. Add `MemoryError` to `actionable_hint()`. Add `ath-memory` dep to ath-cli Cargo.toml. Write tests for config loading (missing file, partial, full) and for tree/search/read/add via direct function calls on temp dirs.
  - Verify: `cargo test -p ath-memory -- config` + `cargo test -p ath-cli -- memory` + `cargo check --workspace`
  - Done when: `ath memory tree`, `search`, `read`, and `add` work on test data with passing tests, config loads with defaults

- [x] **T02: Stats, GC subcommands and full verification** `est:30m`
  - Why: Completes the CLI surface with operational commands (stats for health-check, gc for maintenance) and runs final verification
  - Files: `crates/ath-cli/src/memory.rs`
  - Do: Add `stats` subcommand — walk `.ath/memory/store/` for entry count and disk size, count keyword index entries via `KeywordIndex::load().len()`, count observation files in `.ath/memory/observations/`, format as human-readable summary. Add `gc` subcommand with `--days` flag (default 30) — list `.jsonl` files in observations dir, parse file modification time, delete files older than threshold, report count deleted. Both are sync. Write tests for stats (populate temp dir with known entries, verify output counts) and gc (create old + recent files with modified timestamps, verify only old ones deleted). Run full workspace test suite.
  - Verify: `cargo test -p ath-cli -- memory` (all 6 subcommands) + `cargo test --workspace` — zero failures
  - Done when: All 6 subcommands have passing tests, workspace is clean, no regressions

## Files Likely Touched

- `crates/ath-memory/src/config.rs`
- `crates/ath-memory/src/lib.rs`
- `crates/ath-memory/Cargo.toml`
- `crates/ath-cli/src/memory.rs`
- `crates/ath-cli/src/main.rs`
- `crates/ath-cli/Cargo.toml`
