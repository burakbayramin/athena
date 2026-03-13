---
estimated_steps: 5
estimated_files: 6
---

# T01: Scaffold ath-memory crate with VikingUri and LayeredContent types

**Slice:** S01 — Viking Store & Vector Index
**Milestone:** M002

## Description

Create the `ath-memory` crate following workspace conventions. Define `VikingUri` (parser for `viking://` URIs with path traversal protection), `LayeredContent` (L0/L1/L2 struct), and `MemoryError` (error enum with `hint()` method). Wire the crate into the workspace. This task establishes the type foundation that T02 and T03 build on.

## Steps

1. Create `crates/ath-memory/Cargo.toml` with workspace version/edition, workspace deps (`serde`, `serde_json`, `thiserror`, `uuid`, `chrono`, `tracing`), and `tempfile` as dev-dependency. Add `ath-memory` to workspace root `Cargo.toml` (members already covered by glob, just add `[workspace.dependencies]` entry).
2. Implement `VikingUri` in `src/uri.rs`: parse `viking://` scheme, extract path segments (e.g., `viking://project/conventions` → segments `["project", "conventions"]`), validate no `..` traversal or empty segments, provide `resolve_path(&self, root: &Path) -> Result<PathBuf>` that resolves to a filesystem path within the given root. Include `Display` and `FromStr` implementations.
3. Define `LayeredContent` in `src/types.rs`: struct with `abstract_text: String` (L0), `overview_text: String` (L1), `detail: Option<String>` (L2), plus `uri: VikingUri`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`. Derive `Serialize`/`Deserialize`/`Debug`/`Clone`. Define `MemoryHit` struct for search results: `uri: VikingUri`, `score: f32`, `content: LayeredContent`.
4. Define `MemoryError` in `src/error.rs` following the `ath-config` pattern: variants for `InvalidUri`, `PathTraversal`, `IoError`, `SerializationError`, `IndexError`, `DimensionMismatch`. Each variant has contextual fields. Add `hint()` method with actionable messages.
5. Wire up `src/lib.rs` with `pub mod` declarations and re-exports. Verify with `cargo check --workspace`.

## Must-Haves

- [ ] `VikingUri` parses valid URIs: `viking://project/conventions`, `viking://agents/planner/profile`
- [ ] `VikingUri` rejects: missing scheme, `..` traversal, empty segments, non-viking schemes
- [ ] `VikingUri::resolve_path()` produces paths within root and rejects traversal
- [ ] `LayeredContent` round-trips through serde_json
- [ ] `MemoryError` variants have actionable `hint()` messages
- [ ] `cargo check --workspace` passes (no regressions)

## Verification

- `cargo test -p ath-memory` — URI parsing tests pass (valid, invalid, traversal cases)
- `cargo check --workspace` — workspace compiles cleanly

## Observability Impact

- **New error surface:** `MemoryError` enum with `hint()` method — all future memory operations surface actionable errors through this type. A future agent can inspect error variants and hints via unit tests in `error.rs`.
- **URI validation signals:** `VikingUri::from_str()` returns structured errors (`InvalidUri`, `PathTraversal`) with the offending input embedded, making parse failures diagnosable without additional logging.
- **Failure state inspection:** `MemoryError` implements `std::fmt::Display` with contextual fields (path, URI, expected/actual dimensions), so tracing captures and log lines carry enough context to diagnose without reproduction.

## Inputs

- `crates/ath-config/Cargo.toml` — pattern for workspace dep declarations
- `crates/ath-config/src/error.rs` — pattern for error enum with `hint()` method
- `crates/ath-config/src/lib.rs` — pattern for module structure and re-exports
- `Cargo.toml` (workspace root) — add `ath-memory` workspace dependency

## Expected Output

- `crates/ath-memory/Cargo.toml` — new crate manifest wired to workspace
- `crates/ath-memory/src/lib.rs` — crate root with module declarations and re-exports
- `crates/ath-memory/src/uri.rs` — VikingUri parser with traversal protection and tests
- `crates/ath-memory/src/types.rs` — LayeredContent and MemoryHit structs
- `crates/ath-memory/src/error.rs` — MemoryError enum with hint() method and tests
