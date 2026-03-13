---
id: T02
parent: S01
milestone: M002
provides:
  - VikingStore CRUD implementation with markdown persistence
  - Round-trip serialization of LayeredContent to human-readable markdown with YAML frontmatter
key_files:
  - crates/ath-memory/src/store.rs
  - crates/ath-memory/src/lib.rs
key_decisions:
  - "Store markdown parser splits only on known section headings (Abstract/Overview/Detail) so user content can contain arbitrary ## headings without breaking round-trip"
patterns_established:
  - "VikingStore temp-file + rename pattern for atomic writes, with Windows remove-before-rename handling"
  - "Simple YAML frontmatter parser (hand-rolled, no serde_yaml dep) for uri/created_at/updated_at"
observability_surfaces:
  - "tracing::instrument on all CRUD methods (write, read, delete, list) with URI field"
  - "Store files at .ath/memory/store/<segments>.md are plain markdown, directly inspectable"
  - "MemoryError::IoError includes path and operation context for all I/O failures"
duration: ~20m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Filesystem-backed Viking Store with markdown persistence

**Implemented VikingStore with CRUD operations persisting LayeredContent as human-readable markdown files with YAML frontmatter and atomic temp-file writes.**

## What Happened

Built `VikingStore` in `src/store.rs` with four CRUD methods:
- `write()` — serializes LayeredContent to markdown (frontmatter + `## Abstract`/`## Overview`/`## Detail` sections), writes via temp file + rename for atomicity
- `read()` — parses markdown back into LayeredContent, returns `Ok(None)` for missing entries
- `delete()` — removes backing file, returns whether it existed
- `list()` — walks the store directory tree, reconstructs VikingUri from relative paths

The markdown format uses YAML frontmatter (`uri`, `created_at`, `updated_at`) between `---` fences, followed by `## Abstract`, `## Overview`, and optionally `## Detail` sections. The `## Detail` section is omitted entirely when `detail` is `None`.

Hit one issue during initial testing: the section parser was splitting on all `## ` lines, which broke when overview_text itself contained `## Conventions`. Fixed by restricting the parser to only split on the three known store headings (D005).

## Verification

- `cargo test -p ath-memory -- store` — **14 tests pass** (round-trip, round-trip-no-detail, overwrite, delete, delete-nonexistent, read-nonexistent, list, list-empty, creates-root, creates-parent-dirs, human-readable-markdown, omits-detail-when-none, temp-file, path-traversal-blocked)
- `cargo test -p ath-memory` — **37 tests + 1 doc-test pass** (no regressions from T01)
- `cargo check --workspace` — clean, no regressions

### Slice-level verification (intermediate — T02 of 3):
- ✅ `cargo test -p ath-memory` — all tests pass
- ✅ `cargo check --workspace` — clean
- ✅ URI parsing tests (from T01)
- ✅ Store round-trip, overwrite, missing entry handling
- ⬜ HNSW insert→search→persist→reload→search (T03)
- ⬜ Keyword fallback search (T03)
- ⬜ Dimension mismatch rejection (T03)
- ⬜ Empty index search (T03)

## Diagnostics

- Inspect store behavior: `cargo test -p ath-memory -- store` exercises all CRUD paths
- Inspect written files: store files are plain markdown at `<root>/<uri-segments>.md`
- Inspect errors: MemoryError::IoError includes the path and operation description; SerializationError includes the specific frontmatter field that failed
- Tracing spans on all four CRUD methods log URI and file path at `debug` level

## Deviations

Section parser was narrowed from "split on any `## ` line" to "split only on known headings" to handle user content containing markdown headings. This is a design improvement, not a plan deviation.

## Known Issues

None.

## Files Created/Modified

- `crates/ath-memory/src/store.rs` — VikingStore implementation with CRUD methods, markdown serialization, and 14 tests
- `crates/ath-memory/src/lib.rs` — added `pub mod store` and `pub use store::VikingStore` re-export
- `.gsd/milestones/M002/slices/S01/tasks/T02-PLAN.md` — added Observability Impact section (pre-flight fix)
