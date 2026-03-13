# S05: CLI & Config — UAT

**Milestone:** M002
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All 6 subcommands are tested via unit/integration tests against temp directories with known data. No live server or external service needed — pure filesystem operations.

## Preconditions

- Rust toolchain installed with `cargo` available
- Working directory is the Athena project root
- `cargo build` succeeds (workspace compiles)

## Smoke Test

Run `cargo test -p ath-cli -- memory` — all 21 tests pass, confirming every subcommand works on test data.

## Test Cases

### 1. Config loads from valid TOML file

1. Create a temp directory with `.ath/memory/config.toml` containing:
   ```toml
   [injection]
   max_context_tokens = 2000
   [gc]
   retention_days = 7
   ```
2. Call `MemoryConfig::load(&temp_dir)`
3. **Expected:** `injection.max_context_tokens == 2000`, `gc.retention_days == 7`, `extraction` fields have defaults

### 2. Config returns defaults when file is missing

1. Create a temp directory with no `.ath/memory/config.toml`
2. Call `MemoryConfig::load(&temp_dir)`
3. **Expected:** Returns `MemoryConfig::default()` — no error, no panic

### 3. Malformed config warns and returns defaults

1. Create `.ath/memory/config.toml` with invalid TOML (`not valid toml!!!`)
2. Call `MemoryConfig::load(&temp_dir)`
3. **Expected:** `tracing::warn` emitted (grep for "failed to parse config file"), returns `MemoryConfig::default()`

### 4. `ath memory tree` shows store entries

1. Create a temp `.ath/memory/store/` with entries at `project/overview.md` and `conventions/naming.md`
2. Run `cmd_tree(&root)`
3. **Expected:** Output contains tree-formatted listing with `├──` / `└──` characters, entries grouped under `conventions/` and `project/` prefixes

### 5. `ath memory tree` on empty store

1. Create temp `.ath/memory/store/` with no entries
2. Run `cmd_tree(&root)`
3. **Expected:** Output contains `(empty store)`

### 6. `ath memory search` finds matching entries

1. Populate temp store with entries containing "error handling" in abstract text
2. Build and save a keyword index covering those entries
3. Run `cmd_search(&root, "error", 5)`
4. **Expected:** Output lists matching URIs with relevance scores, abstract text preview

### 7. `ath memory search` with no keyword index

1. Create temp store dir with no `keyword_index.json`
2. Run `cmd_search(&root, "anything", 5)`
3. **Expected:** Output contains `(no keyword index found)` — no error/panic

### 8. `ath memory search` with no results

1. Build a keyword index with entries about "rust compilation"
2. Run `cmd_search(&root, "javascript", 5)`
3. **Expected:** Output contains `No results for 'javascript'`

### 9. `ath memory read` displays all layers

1. Write a Viking entry with abstract, overview, and detail text
2. Run `cmd_read(&root, "viking://project/overview")`
3. **Expected:** Output shows `Abstract:`, `Overview:`, and `Detail:` sections with the correct content

### 10. `ath memory read` for nonexistent URI

1. Run `cmd_read(&root, "viking://does/not/exist")`
2. **Expected:** Output contains "not found" or similar — no panic

### 11. `ath memory add` writes to store and index

1. Run `cmd_add(&root, "viking://project/new-entry", "New entry content")`
2. Verify entry exists via `VikingStore::read()`
3. Verify keyword index contains the new entry via `KeywordIndex::load().search("content")`
4. **Expected:** Entry readable from store with abstract_text matching input, keyword index finds it

### 12. `ath memory add` updates existing index

1. Build and save a keyword index with existing entries
2. Run `cmd_add(&root, "viking://project/another", "Another entry")`
3. **Expected:** New entry added to existing index — old entries still present, new entry searchable

### 13. `ath memory stats` shows correct counts

1. Create temp store with 3 entries, a keyword index with 5 entries, 2 observation .jsonl files
2. Run `cmd_stats(&root)`
3. **Expected:** Output shows `Store entries: 3`, `Index entries: 5`, `Observation files: 2`, and a non-zero disk usage in B/KB/MB format

### 14. `ath memory stats` on nonexistent directory

1. Run `cmd_stats` on a path with no `.ath/memory/` subdirectory
2. **Expected:** All counts show 0, disk usage shows "0 B" — no error/panic

### 15. `ath memory gc` deletes old observation files

1. Create `.ath/memory/observations/` with:
   - `old-run.jsonl` — mtime set to 60 days ago via `filetime`
   - `recent-run.jsonl` — mtime set to 5 days ago
2. Run `cmd_gc(&root, 30)`
3. **Expected:** `old-run.jsonl` deleted, `recent-run.jsonl` preserved, output reports 1 file deleted with bytes freed

### 16. `ath memory gc` keeps all files when none are old

1. Create `.ath/memory/observations/` with only recent .jsonl files
2. Run `cmd_gc(&root, 30)`
3. **Expected:** No files deleted, output reports 0 files deleted

### 17. `ath memory gc` ignores non-.jsonl files

1. Create `.ath/memory/observations/` with:
   - `old-run.jsonl` — mtime 60 days ago
   - `old-notes.txt` — mtime 60 days ago
2. Run `cmd_gc(&root, 30)`
3. **Expected:** Only `old-run.jsonl` deleted; `old-notes.txt` preserved regardless of age

### 18. `ath memory gc` with missing observations directory

1. Run `cmd_gc` on a root with no `.ath/memory/observations/`
2. **Expected:** Output contains "No observations to clean" — no error/panic

### 19. MemoryError shows actionable hint

1. Create a `MemoryError::Store("cannot read".into())` error
2. Pass through `actionable_hint()` / `display_error()`
3. **Expected:** Output contains `Fix:` with actionable guidance text

### 20. MemoryError IO variant shows fix hint

1. Create a `MemoryError::Io(io::Error)` variant
2. Pass through `actionable_hint()` / `display_error()`
3. **Expected:** Output contains `Fix:` with relevant filesystem guidance

## Edge Cases

### Partial config.toml (only gc section)

1. Create config with only `[gc]\nretention_days = 14`
2. Load config
3. **Expected:** `gc.retention_days == 14`, injection and extraction fields use defaults

### format_bytes output tiers

1. Call `format_bytes(500)` → `"500 B"`
2. Call `format_bytes(1536)` → `"1.5 KB"`
3. Call `format_bytes(2_500_000)` → `"2.4 MB"`
4. **Expected:** Correct tier selection and 1-decimal formatting

### Tree with single entry

1. Create store with exactly one entry
2. Run `cmd_tree`
3. **Expected:** Uses `└──` (last-item connector) for the single entry — no `├──`

## Failure Signals

- Any test in `cargo test -p ath-cli -- memory` failing
- `cargo test -p ath-memory -- config` tests failing
- `MemoryConfig::load` panicking on malformed input instead of returning defaults
- `cmd_gc` deleting non-.jsonl files
- `cmd_stats` erroring on missing directories instead of showing zero counts
- Any `cargo check --workspace` errors introduced by S05

## Not Proven By This UAT

- CLI binary invocation (`ath memory tree` as a subprocess) — tests call functions directly
- Real-world `.ath/memory/` with production data volumes
- Interaction between CLI commands and concurrent extraction/injection (S04 systems)
- End-to-end flow where run → extract → CLI inspect works as a pipeline (covered by S06)

## Notes for Tester

- All tests use temp directories with synthetic data — no real `.ath/` directory needed
- The `filetime` crate is only a dev-dependency; it's used solely for backdating mtime in gc tests
- Pre-existing `dead_code` warnings in `ath-memory` observe buffer are known and unrelated to S05
- Config tests live in `ath-memory` crate; all other tests live in `ath-cli` crate
