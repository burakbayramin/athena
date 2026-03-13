# S03: Git Layer

**Goal:** Set up ath-git crate with dependencies, error types, commit message builder, and GitLayer struct with repository management.
**Demo:** Set up ath-git crate with dependencies, error types, commit message builder, and GitLayer struct with repository management.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Set up ath-git crate with dependencies, error types, commit message builder, and GitLayer struct with repository management.

Purpose: Establishes the foundation that Plan 02 builds commit logic on top of. All types, errors, and the repository handle are created here.
Output: Compiling ath-git crate with GitLayer::new(), CommitMetadata, GitError, and passing unit/integration tests.
- [x] **T02: Plan 02**
  - Implement the core stage-and-commit workflow on GitLayer: file staging with explicit file list, commit creation with trailers, initial commit handling, and empty diff detection.

Purpose: This is the heart of Phase 3 -- the actual git commit capability that OUTP-01 requires.
Output: Working stage_and_commit method with comprehensive integration tests proving all success criteria.
- [x] **T03: Plan 03**
  - Add the async wrapper module for spawn_blocking integration, test file deletion staging, and run final verification of all phase success criteria.

Purpose: Completes the ath-git crate so it is ready for Phase 7 (PhaseRunner) consumption via the async API, and confirms all edge cases are handled.
Output: Fully tested ath-git crate with sync core and async wrapper, all success criteria verified.

## Files Likely Touched

