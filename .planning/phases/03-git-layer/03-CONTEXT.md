# Phase 3: Git Layer - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

In-process git commits after each phase completes, with per-phase metadata in commit messages, using git2 (no system git dependency). Delivers: GitLayer struct with repository management, file staging, commit creation with trailers, and edge case handling (initial commit, empty diff, dirty tree). No orchestration logic, no task routing, no review gates.

</domain>

<decisions>
## Implementation Decisions

### Commit Message Format
- Git trailers for structured metadata (standard git convention, machine-parseable)
- Subject line uses `athena:` prefix for easy filtering (`git log --grep`)
- Trailers included: Phase (name/number), Agent (provider/model), Task-Id, Files-Count, Review-Status, Reviewer
- Commit author set to `Athena <athena@noreply>` — committer stays as system git user
- No token usage in commits (tracked in PhaseRecord, not git history)

### Repository Targeting
- Explicit path: `GitLayer::new(path: PathBuf)` — caller passes target directory, no magic discovery
- Auto-init: if no .git exists, `git2::Repository::init()` creates one (handles greenfield case per success criteria #4)
- Commit to current branch — Athena's commits identifiable by prefix and trailers, user controls branching
- Error on dirty working tree — refuse to operate if uncommitted changes exist, prevents mixing human and Athena changes

### Async Boundary
- GitLayer is sync internally (git2 is sync)
- spawn_blocking wrapper for async callers (Phase 7's PhaseRunner)
- GitLayer owns the git2::Repository handle for its lifetime (opened once in new())
- Arc<Mutex<Repository>> for thread safety across spawn_blocking calls

### File Staging Strategy
- Explicit file list: caller passes `Vec<PathBuf>` from AgentContribution.files_produced
- Error and abort if a declared file doesn't exist on disk — catches agent bugs early
- Support deletions: if a listed file existed before but is now gone, stage the removal
- Empty diff (no changes): skip silently, return Ok with a flag indicating no commit was created

### Claude's Discretion
- Exact GitLayer method signatures and return types
- Internal git2 index/tree manipulation approach
- Error type granularity within ath-git
- spawn_blocking wrapper naming and module structure
- Merge conflict detection implementation details (success criteria don't require resolution, just detection)

</decisions>

<specifics>
## Specific Ideas

- The `athena:` prefix + git trailers pattern mirrors how CI systems and bots mark their commits — familiar to developers
- "Error on dirty tree" is how tools like `cargo release` and `git-absorb` work — clean separation of concerns
- Explicit file list enforces the module isolation principle from Phase 6 downstream — GitLayer is the enforcement point

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AgentKind` (ath-types/src/agent.rs): provider_name() and model() methods for building Agent trailer value
- `AgentContribution` (ath-types/src/phase.rs): files_produced field provides the explicit file list for staging
- `PhaseRecord` (ath-types/src/phase.rs): phase_name for commit subject and Phase trailer
- `ReviewVerdict` (ath-types/src/review.rs): passed field and reviewer field for Review-Status and Reviewer trailers

### Established Patterns
- `thiserror` for domain errors with fix hints (ValidationError pattern in ath-types)
- All types derive Debug, Clone, Serialize, Deserialize, PartialEq
- Workspace-level dependency management in root Cargo.toml
- Sync crates (ath-types, ath-config) don't depend on tokio — ath-git follows this pattern (sync core, async wrapper)

### Integration Points
- ath-git crate stub exists with ath-types dependency already configured
- git2 needs adding as workspace dependency (ath-git is the only consumer)
- tokio dependency needed for spawn_blocking wrapper (or wrapper lives in calling crate)
- Downstream: Phase 7 (PhaseRunner) calls GitLayer after each phase completes
- Downstream: Phase 10 (Parallel Execution) needs serialized commits — Arc<Mutex> design supports this

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-git-layer*
*Context gathered: 2026-03-12*
