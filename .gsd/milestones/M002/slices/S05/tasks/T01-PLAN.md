---
estimated_steps: 8
estimated_files: 6
---

# T01: Config schema, tree/search/read/add subcommands, and CLI wiring

**Slice:** S05 — CLI & Config
**Milestone:** M002

## Description

Build `MemoryConfig` in `ath-memory` for TOML-based config loading, then create the `memory.rs` CLI module in `ath-cli` with the four data-path subcommands (tree, search, read, add). Wire the new `Memory` variant into the existing `Commands` enum and add `MemoryError` to `actionable_hint()`.

## Steps

1. Add `toml` workspace dep to `ath-memory/Cargo.toml`. Create `crates/ath-memory/src/config.rs` with `MemoryConfig` struct containing `injection: InjectionConfig`, `extraction: ExtractionConfig`, `gc: GcConfig` (new struct with `retention_days: u32` default 30, `max_observation_files: Option<usize>`). All fields `#[serde(default)]`. Add `MemoryConfig::load(root: &Path) -> Self` that reads `.ath/memory/config.toml`, returns `Default` on missing file, logs warning on parse error and returns `Default`. Implement Deserialize bridges for `InjectionConfig` and `ExtractionConfig` (they don't derive Deserialize yet — add serde Deserialize derive or use intermediate structs). Add `pub mod config` to `lib.rs` and re-export `MemoryConfig`.

2. Write unit tests for config: deserialize full TOML, deserialize partial TOML (only `[gc]` section), missing file returns defaults, malformed file returns defaults with no panic.

3. Add `ath-memory = { path = "../ath-memory" }` to `ath-cli/Cargo.toml`. Create `crates/ath-cli/src/memory.rs` with `MemoryArgs` (clap `Args` with `#[command(subcommand)]`) and `MemoryCommands` enum (clap `Subcommand`) containing variants: `Tree`, `Search { query: String, #[arg(long, default_value_t = 10)] top: usize }`, `Read { uri: String }`, `Add { uri: String, content: String }`.

4. Implement `memory_command(args, global) -> Result<()>` that resolves project dir, constructs `.ath/memory/` paths, and dispatches to per-subcommand functions. Implement `cmd_tree` — call `VikingStore::list()`, group URIs by prefix segments to build a tree, format with `├── / └──` box-drawing characters showing entry counts per top-level directory.

5. Implement `cmd_search` — load `KeywordIndex` from `.ath/memory/index/keyword.json` (return empty results if missing), call `search(query, top)`, for each hit load the entry from store to show abstract text, format as `[{score:.2}] {uri} — {abstract}`.

6. Implement `cmd_read` — parse URI string via `VikingUri::from_str`, call `store.read(&uri)`, display all three layers (Abstract, Overview, Detail) with section headers if non-empty.

7. Implement `cmd_add` — parse URI, construct `LayeredContent::new(uri, content, "")` (abstract = user content, overview = empty), call `store.write()`, load/create keyword index, call `index.add(uri_str, content)`, save index. Print confirmation.

8. Wire into `main.rs`: add `mod memory;` and `Memory(memory::MemoryArgs)` variant to `Commands`. Add dispatch arm in `dispatch()` — call `memory::memory_command(args, global)` (sync, not async). Add `use ath_memory::MemoryError;` and the `MemoryError` check to `actionable_hint()`. Write CLI tests for tree/search/read/add using temp dirs with pre-populated store data.

## Must-Haves

- [ ] `MemoryConfig` deserializes from TOML with all fields defaulting
- [ ] `MemoryConfig::load()` returns defaults on missing/malformed file
- [ ] `GcConfig` struct with `retention_days` (default 30)
- [ ] `ath memory tree` formats store listing as indented tree
- [ ] `ath memory search` returns keyword hits with scores
- [ ] `ath memory read` displays layered content for a URI
- [ ] `ath memory add` writes to store AND keyword index
- [ ] `MemoryError` integrated into `actionable_hint()`
- [ ] Config and CLI tests pass

## Verification

- `cargo test -p ath-memory -- config` — config deserialization and loading tests pass
- `cargo test -p ath-cli -- memory` — tree, search, read, add tests pass
- `cargo check --workspace` — no errors or new warnings

## Observability Impact

- **Config parse failures**: `MemoryConfig::load()` emits `tracing::warn!` with the parse error message when `config.toml` is malformed, then returns defaults. A future agent can grep tracing output for "config" + "warn" to detect misconfiguration.
- **MemoryError hints**: `MemoryError` is wired into `actionable_hint()` in `main.rs`, so any memory subsystem error displayed via `display_error` includes a `Fix:` line with resolution guidance.
- **CLI subcommand output**: `cmd_tree`, `cmd_search`, `cmd_read`, `cmd_add` print structured output to stdout — empty store/index states produce informative "no entries" / "no results" messages rather than silent empty output.

## Inputs

- `crates/ath-memory/src/store.rs` — `VikingStore::new/list/read/write/delete`
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::new/load/save/add/search`
- `crates/ath-memory/src/inject/injector.rs` — `InjectionConfig` struct (needs Deserialize)
- `crates/ath-memory/src/extract/types.rs` — `ExtractionConfig` struct (needs Deserialize)
- `crates/ath-cli/src/main.rs` — `Commands` enum pattern, `actionable_hint()`, `dispatch()`
- `crates/ath-cli/src/report.rs` — reference pattern for subcommand structure and tests

## Expected Output

- `crates/ath-memory/src/config.rs` — `MemoryConfig`, `GcConfig` with serde + TOML loading + tests
- `crates/ath-memory/src/lib.rs` — `pub mod config` + re-export
- `crates/ath-memory/Cargo.toml` — `toml` dependency added
- `crates/ath-cli/src/memory.rs` — `MemoryArgs`, `MemoryCommands`, 4 subcommand implementations + tests
- `crates/ath-cli/src/main.rs` — `Memory` variant, `MemoryError` in hint chain, `mod memory`
- `crates/ath-cli/Cargo.toml` — `ath-memory` dependency added
