---
phase: 03-git-layer
plan: 03
subsystem: git
tags: [git2, async, spawn_blocking, tokio, edge-cases, deletion, conflict-detection]

# Dependency graph
requires:
  - phase: 03-git-layer
    plan: 02
    provides: "stage_and_commit method, CommitResult, empty diff detection, initial commit support"
provides:
  - "AsyncGitLayer wrapping GitLayer with spawn_blocking for async callers"
  - "Clone derive on GitLayer for Arc-based sharing across spawn_blocking"
  - "File deletion staging verified via integration test"
  - "Merge conflict detection blocking commits"
  - "Full end-to-end workflow test covering all 4 roadmap success criteria"
affects: [07-review-engine, 10-parallel-execution]

# Tech tracking
tech-stack:
  added: [tokio (spawn_blocking wrapper)]
  patterns: [AsyncGitLayer facade over sync GitLayer, Clone+Arc for spawn_blocking sharing]

key-files:
  created:
    - crates/ath-git/src/async_ops.rs
  modified:
    - crates/ath-git/src/layer.rs
    - crates/ath-git/src/lib.rs
    - crates/ath-git/Cargo.toml
    - crates/ath-git/tests/integration.rs

key-decisions:
  - "AsyncGitLayer owns GitLayer directly (not Arc-wrapped) since GitLayer is already Clone via Arc<Mutex<Repository>>"
  - "Used treebuilder API for conflict test to avoid working-tree mutations that block git2 merge"

patterns-established:
  - "spawn_blocking wrapper pattern: clone layer into closure, map JoinError to GitError::TaskJoin"
  - "End-to-end workflow test pattern: create files, build metadata with from_phase_data, commit, verify tree and trailers"

requirements-completed: [OUTP-01]

# Metrics
duration: 4min
completed: 2026-03-12
---

# Phase 3 Plan 03: Async Wrapper and Edge Cases Summary

**AsyncGitLayer with spawn_blocking for tokio callers, plus deletion staging, conflict detection, and full e2e success criteria verification**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T13:23:45Z
- **Completed:** 2026-03-12T13:28:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- AsyncGitLayer module providing commit_phase_async and is_dirty_async via spawn_blocking
- 6 new integration tests: async commit, async initial commit, deletion staging, dirty tree rejection, conflict blocking, full e2e workflow
- All 4 roadmap success criteria verified by named tests (commit scoping, trailer metadata, one-commit-per-phase, empty repo)
- 156 workspace tests passing, cargo clippy clean with -D warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Async wrapper module and edge case tests** - `9a7ee49` (feat)
2. **Task 2: Final workspace verification and success criteria audit** - `7e30bcc` (test)

## Files Created/Modified
- `crates/ath-git/src/async_ops.rs` - AsyncGitLayer struct with spawn_blocking wrappers for commit and is_dirty
- `crates/ath-git/src/layer.rs` - Added Clone derive to GitLayer, cfg_attr for repo_handle dead_code warning
- `crates/ath-git/src/lib.rs` - Added async_ops module and AsyncGitLayer re-export
- `crates/ath-git/Cargo.toml` - Added tokio dependency (workspace) and tokio dev-dependency with rt-multi-thread+macros
- `crates/ath-git/tests/integration.rs` - Added 6 new tests covering async ops, deletion, dirty tree, conflicts, full workflow

## Decisions Made
- AsyncGitLayer owns GitLayer directly rather than wrapping in another Arc -- GitLayer already has Arc<Mutex<Repository>> internally so Clone is cheap
- Used git2 treebuilder API to create the feature branch commit in conflict_detection test, avoiding working-tree modifications that cause "uncommitted changes would be overwritten by merge" errors
- dirty_tree_with_untracked_file_rejects test verifies is_dirty() detects untracked files, since stage_and_commit itself does not check dirty state (callers use is_dirty before calling)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed conflict test working tree interference**
- **Found during:** Task 1 (conflict_detection_blocks_commit test)
- **Issue:** Writing "feature change" to conflict.txt on disk before merge caused git2 to reject the merge with "uncommitted changes would be overwritten"
- **Fix:** Used repo.treebuilder() to build the feature branch commit tree directly without modifying the working directory
- **Files modified:** crates/ath-git/tests/integration.rs
- **Verification:** conflict_detection_blocks_commit test passes
- **Committed in:** 9a7ee49 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed clippy dead_code warning on repo_handle**
- **Found during:** Task 2 (clippy -D warnings audit)
- **Issue:** repo_handle() is pub(crate) and only used in unit tests; clippy flags it as dead_code
- **Fix:** Added `#[cfg_attr(not(test), allow(dead_code))]` attribute
- **Files modified:** crates/ath-git/src/layer.rs
- **Verification:** cargo clippy -p ath-git -- -D warnings passes clean
- **Committed in:** 7e30bcc (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for test correctness and CI compliance. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ath-git crate is feature-complete for Phase 3 scope
- AsyncGitLayer ready for Phase 7 (PhaseRunner) consumption
- All 4 roadmap success criteria covered by integration tests
- 156 workspace tests passing with zero regressions

---
*Phase: 03-git-layer*
*Completed: 2026-03-12*
