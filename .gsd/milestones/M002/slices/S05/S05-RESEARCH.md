# S05: CLI & Config — Research

**Date:** 2026-03-14

## Summary

S05 adds `ath memory` CLI subcommands (`tree`, `search`, `read`, `add`, `stats`, `gc`) and a `config.toml` schema for memory settings. The foundation is solid — S01–S04 provide all the backing APIs: `VikingStore::list/read/write/delete`, `KeywordIndex::search/add/len`, `ObservationReader`, `InjectionConfig`, and `ExtractionConfig`. The CLI crate already uses clap derive with `Subcommand` enum pattern, and adding a new `Memory(memory::MemoryArgs)` variant is trivial.

The main design question is where to put the CLI logic — the spec suggests `ath-memory/src/cli/` but the actual crate structure puts all CLI code in `ath-cli/src/`. Following established patterns means adding a `memory.rs` module in `ath-cli` with a `MemoryArgs` struct containing nested subcommands. The config schema lives in `ath-memory` since it owns `InjectionConfig` and `ExtractionConfig`, and `ath-cli` passes parsed config values through.

This is a low-risk slice. All underlying APIs exist and are tested. The work is straightforward wiring: parse args → instantiate `VikingStore`/`KeywordIndex` → call existing methods → format output. No async needed for any subcommand except possibly extraction stats.

## Recommendation

1. **Config**: Add a `MemoryConfig` struct in `ath-memory` with serde `Deserialize` for TOML loading from `.ath/memory/config.toml`. It wraps `InjectionConfig`, `ExtractionConfig`, and a new `GcConfig` (retention days, max size). Provide `MemoryConfig::load(root)` and `MemoryConfig::default()`. Uses the `toml` workspace dependency.

2. **CLI**: Add `memory.rs` in `ath-cli/src/` with a `MemoryCommands` enum (clap `Subcommand`). Each subcommand is a simple function that constructs the necessary memory types from the `.ath/memory/` directory and calls existing APIs:
   - `tree` → `VikingStore::list()` + tree-format the URIs
   - `search <query>` → `KeywordIndex::load()` + `search(query, top_k)`  → join with `VikingStore::read()` for display
   - `read <uri>` → `VikingStore::read(uri)` → formatted output
   - `add <uri> <content>` → construct `LayeredContent` + `VikingStore::write()` + `KeywordIndex::add()` + save
   - `stats` → count files, dirs, index entries, observation files, disk size
   - `gc` → delete observation JSONL files older than retention period, optionally compact store

3. **Integration**: Wire `Memory(MemoryArgs)` into `Commands` enum in `main.rs`. Add `ath-memory` dependency to `ath-cli/Cargo.toml`.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| URI parsing | `VikingUri::from_str` | Already validates scheme, segments, traversal |
| Store CRUD | `VikingStore` | Atomic writes, markdown persistence, listing |
| Keyword search | `KeywordIndex::search()` | TF scoring, stopwords, persistence |
| Config parsing | `toml` crate (workspace dep) | Already used by `ath-config` for global config |
| CLI arg parsing | clap derive macros | Already the pattern in `ath-cli` |
| Tree formatting | Hand-roll with `├── / └──` characters | Simple enough; no need for a dep |

## Existing Code and Patterns

- `crates/ath-cli/src/main.rs` — `Commands` enum with `Subcommand` derive, `dispatch()` async fn, `display_error()` with `actionable_hint()` from `MemoryError::hint()`. Follow this pattern: add `Memory(memory::MemoryArgs)` variant.
- `crates/ath-cli/src/report.rs` — Good reference for a subcommand that loads data from `.ath/` dir, formats it with colored output, and has tests. The `ReportTarget` pattern (enum for defaulting vs explicit) applies to `gc --days` vs default retention.
- `crates/ath-cli/src/run.rs` — Shows how `ConfigStore::load()` is called and how the project dir is resolved (`std::env::current_dir()`).
- `crates/ath-config/src/file.rs` — `RawFileConfig` with `Deserialize` and `load_config_file()`. Config.toml in `.ath/memory/` follows the same TOML loading pattern but is local to the memory crate.
- `crates/ath-memory/src/store.rs` — `VikingStore::new(root)` creates dirs, `list()` returns sorted `Vec<VikingUri>`, `read()` returns `Option<LayeredContent>`.
- `crates/ath-memory/src/keyword.rs` — `KeywordIndex::load(path)/save(path)`, `search(query, top_k) -> Vec<KeywordHit>` where `KeywordHit { uri, score }`.
- `crates/ath-memory/src/inject/injector.rs` — `InjectionConfig` with default values. Config.toml's `[injection]` section maps directly to these fields.
- `crates/ath-memory/src/extract/types.rs` — `ExtractionConfig` with `max_observation_count` and `max_serialization_bytes`. Maps to `[extraction]` section.
- `crates/ath-memory/src/error.rs` — `MemoryError` already has `hint()` for all variants. CLI `display_error` already handles it via `find_cause::<MemoryError>` — just need to add the import.

## Constraints

- **ath-cli currently doesn't depend on ath-memory** — need to add `ath-memory = { path = "../ath-memory" }` to `ath-cli/Cargo.toml`
- **Memory root is `.ath/memory/`** relative to project dir — resolved via `std::env::current_dir()` + `.ath/memory/`
- **`KeywordIndex` and `MemoryIndex` require `&mut self`** for mutation ops — CLI commands that modify (add, gc) need owned or `Mutex`-wrapped instances
- **No async required** — all VikingStore/KeywordIndex operations are sync. CLI subcommands can be plain `fn` (not async), matching `init` and `report` patterns.
- **`MemoryError` must be wired into `actionable_hint()`** in `main.rs` — currently only `ConfigError`, `AgentError`, `PhaseRunnerError`, and `ValidationError` are handled. Need to add `MemoryError`.
- **Config.toml is optional** — must default gracefully when `.ath/memory/config.toml` doesn't exist, same as global config does for `~/.config/ath/config.toml`
- **Store markdown format** uses known section headings (`## Abstract`, `## Overview`, `## Detail`). The `add` command creates content with this format via `LayeredContent::new()`.

## Common Pitfalls

- **Tree building from flat URI list** — `VikingStore::list()` returns flat sorted `Vec<VikingUri>`. Building a tree view requires grouping by shared prefix segments. Need to handle single-entry directories (don't show as branches) and deep nesting gracefully. The sort order from `list()` guarantees lexicographic ordering which makes tree construction straightforward.
- **GC deleting active observation files** — GC removes old `.jsonl` files from observations dir. Must not delete the file for a currently active run. Since GC is a manual CLI command (not called during a run), this is unlikely but should be guarded by checking if the observation writer is active. Simplest: only GC files older than a configurable threshold (default 30 days).
- **Config.toml field names vs Rust struct names** — The spec uses `total_budget`, `project_identity`, etc. in TOML. The Rust `InjectionConfig` uses the same field names. Use `#[serde(default)]` on all fields so partial configs work. The TOML section names (`[injection]`, `[extraction]`, `[gc]`) map to struct fields via nested serde.
- **`add` command must update both store AND keyword index** — Writing to the store without indexing means the entry won't appear in `search` results. Save the keyword index after adding.

## Open Risks

- **Disk size calculation for `stats`** may be slow on large memory stores — `walkdir` or recursive `fs::read_dir` with file size accumulation. At Athena's scale (~1000 entries), this is fine. Not a real risk.
- **GC for store entries is underspecified** — The spec mentions `ath memory compact` for re-summarizing old entries, which requires LLM calls. For S05, limit GC to observation file cleanup and store entry deletion by age. Compaction is out of scope (requires LLM, belongs in a future slice).

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust + clap CLI | `bobmatnyc/claude-mpm-skills@clap` (65 installs) | available — low install count, not essential |
| Rust + clap CLI | `pproenca/dot-skills@rust-clap` (58 installs) | available — low install count, not essential |

No skills warrant installation — the work is standard clap + serde patterns already well-established in this codebase.

## Sources

- `docs/specs/memory-layer.md` §7 (CLI Interface) and §10 (Token Budget Defaults) — spec for subcommand signatures and config.toml schema
- S01–S04 summaries — API surface and integration patterns
- `crates/ath-cli/src/main.rs` — existing CLI architecture
- `crates/ath-config/src/` — config loading patterns
