# Phase 3: Git Layer - Research

**Researched:** 2026-03-12
**Domain:** In-process git operations via git2-rs (libgit2 bindings for Rust)
**Confidence:** HIGH

## Summary

Phase 3 implements a `GitLayer` struct in the `ath-git` crate that commits generated code to a local git repository after each phase completes, embedding structured metadata (phase name, agent, task ID) in commit messages via git trailers. The crate uses git2 0.20.4, which bundles libgit2 statically -- no system git dependency required.

The core workflow is: open/init repo, check for dirty working tree, stage explicit file list, build commit message with trailers, create commit (handling initial commit as special case with no parents). The `GitLayer` is sync internally (git2 is entirely synchronous) with an `Arc<Mutex<Repository>>` for thread safety, and async callers use `tokio::task::spawn_blocking` to avoid blocking the runtime.

**Primary recommendation:** Use git2 0.20.4 with its bundled libgit2. Keep GitLayer sync, expose a thin async wrapper module. Use `Index::has_conflicts()` for merge conflict detection, `Repository::is_empty()` for initial commit detection, and `Repository::statuses()` for dirty tree checks.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Git trailers for structured metadata (standard git convention, machine-parseable)
- Subject line uses `athena:` prefix for easy filtering (`git log --grep`)
- Trailers included: Phase (name/number), Agent (provider/model), Task-Id, Files-Count, Review-Status, Reviewer
- Commit author set to `Athena <athena@noreply>` -- committer stays as system git user
- No token usage in commits (tracked in PhaseRecord, not git history)
- Explicit path: `GitLayer::new(path: PathBuf)` -- caller passes target directory, no magic discovery
- Auto-init: if no .git exists, `git2::Repository::init()` creates one
- Commit to current branch -- Athena's commits identifiable by prefix and trailers
- Error on dirty working tree -- refuse to operate if uncommitted changes exist
- GitLayer is sync internally (git2 is sync)
- spawn_blocking wrapper for async callers (Phase 7's PhaseRunner)
- GitLayer owns the git2::Repository handle for its lifetime (opened once in new())
- Arc<Mutex<Repository>> for thread safety across spawn_blocking calls
- Explicit file list: caller passes `Vec<PathBuf>` from AgentContribution.files_produced
- Error and abort if a declared file doesn't exist on disk
- Support deletions: if a listed file existed before but is now gone, stage the removal
- Empty diff (no changes): skip silently, return Ok with a flag indicating no commit was created

### Claude's Discretion
- Exact GitLayer method signatures and return types
- Internal git2 index/tree manipulation approach
- Error type granularity within ath-git
- spawn_blocking wrapper naming and module structure
- Merge conflict detection implementation details (detection only, not resolution)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| OUTP-01 | Athena commits generated code to local git repo after each phase with phase/agent metadata | git2 0.20.4 provides Repository::commit() with Signature for author/committer, Index for staging, and free-form message body for trailers |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| git2 | 0.20.4 | In-process git operations (init, stage, commit, status) | Official Rust bindings for libgit2; bundles libgit2 statically so no system dependency; used by cargo itself |
| thiserror | 2.0 | Domain error types with fix hints | Project standard (established in ath-types) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1.x (workspace) | spawn_blocking for async wrapper | Only in the async wrapper module; core GitLayer is sync |
| tempfile | 3.x | Temporary directories for test repositories | Test-only dependency for creating isolated git repos |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| git2 | gitoxide (gix) | gitoxide is pure Rust but API is less stable, more complex for simple operations; git2 is battle-tested and matches the decision in CONTEXT.md |
| git2 | std::process::Command("git") | Requires system git installed; parsing stdout is fragile; CONTEXT.md explicitly chose in-process |

**Installation (add to workspace Cargo.toml):**
```toml
[workspace.dependencies]
git2 = "0.20"
tempfile = "3"
```

**ath-git/Cargo.toml additions:**
```toml
[dependencies]
ath-types = { path = "../ath-types" }
git2 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

## Architecture Patterns

### Recommended Project Structure
```
crates/ath-git/
  src/
    lib.rs          # Public API re-exports
    layer.rs        # GitLayer struct (sync core)
    commit.rs       # Commit message builder with trailers
    error.rs        # GitError enum (thiserror)
    async_ops.rs    # spawn_blocking async wrappers
  tests/
    integration.rs  # Tests using tempfile repos
```

### Pattern 1: GitLayer Sync Core
**What:** GitLayer holds `Arc<Mutex<git2::Repository>>` and exposes sync methods for staging and committing.
**When to use:** All git operations go through GitLayer.
**Example:**
```rust
// Source: git2 0.20.4 docs (docs.rs/git2)
use git2::{Repository, Signature, IndexAddOption};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct GitLayer {
    repo: Arc<Mutex<Repository>>,
    repo_path: PathBuf,
}

impl GitLayer {
    pub fn new(path: PathBuf) -> Result<Self, GitError> {
        let repo = if path.join(".git").exists() {
            Repository::open(&path)?
        } else {
            Repository::init(&path)?
        };
        Ok(Self {
            repo: Arc::new(Mutex::new(repo)),
            repo_path: path,
        })
    }
}
```

### Pattern 2: Commit with Trailers
**What:** Build commit messages with git trailers appended after a blank line.
**When to use:** Every commit created by GitLayer.
**Example:**
```rust
// Git trailer format (standard convention)
fn build_commit_message(meta: &CommitMetadata) -> String {
    let mut msg = format!("athena: {}", meta.phase_name);
    msg.push_str("\n\n");  // blank line before trailers
    msg.push_str(&format!("Phase: {}\n", meta.phase_name));
    msg.push_str(&format!("Agent: {}/{}\n", meta.agent_provider, meta.agent_model));
    msg.push_str(&format!("Task-Id: {}\n", meta.task_id));
    msg.push_str(&format!("Files-Count: {}\n", meta.files_count));
    if let Some(review) = &meta.review_status {
        msg.push_str(&format!("Review-Status: {}\n", review));
    }
    if let Some(reviewer) = &meta.reviewer {
        msg.push_str(&format!("Reviewer: {}\n", reviewer));
    }
    msg
}
```

### Pattern 3: Initial Commit Detection
**What:** Use `Repository::is_empty()` to detect whether this is the first commit; pass empty parents slice.
**When to use:** Every commit -- determines parent list.
**Example:**
```rust
// Source: git2-rs examples/init.rs pattern
let parents = if repo.is_empty()? {
    vec![]  // initial commit: no parents
} else {
    let head = repo.head()?.peel_to_commit()?;
    vec![head]
};
let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
repo.commit(
    Some("HEAD"),
    &author,
    &committer,
    &message,
    &tree,
    &parent_refs,
)?;
```

### Pattern 4: Staging Explicit Files
**What:** Stage only the files declared by the agent, verify each exists, handle deletions.
**When to use:** Before every commit.
**Example:**
```rust
// Source: git2 docs - Index::add_path, Index::remove_path
let mut index = repo.index()?;
for file_path in &files {
    let abs_path = repo_path.join(file_path);
    if abs_path.exists() {
        index.add_path(file_path)?;  // must be relative to repo root
    } else {
        // File was declared but doesn't exist -- check if it's a deletion
        // Try to remove from index (was tracked before)
        match index.remove_path(file_path) {
            Ok(()) => {},  // staged deletion
            Err(_) => return Err(GitError::FileMissing {
                path: file_path.to_path_buf(),
                hint: "Declared file does not exist on disk and was not previously tracked".into(),
            }),
        }
    }
}
index.write()?;
let tree_oid = index.write_tree()?;
let tree = repo.find_tree(tree_oid)?;
```

### Pattern 5: Dirty Working Tree Check
**What:** Before operating, verify no uncommitted changes exist from the user.
**When to use:** At the start of any commit operation.
**Example:**
```rust
// Source: git2 docs - Repository::statuses
fn check_clean(repo: &Repository) -> Result<(), GitError> {
    let statuses = repo.statuses(None)?;
    if !statuses.is_empty() {
        return Err(GitError::DirtyWorkingTree {
            hint: "Commit or stash your changes before running Athena".into(),
        });
    }
    Ok(())
}
```
**Note:** For an empty (just-initialized) repo, `statuses()` returns empty since there are no tracked files yet. This is the correct behavior -- initial commit case should not trigger dirty tree error.

### Pattern 6: Async Wrapper with spawn_blocking
**What:** Thin async functions that clone the Arc and move into spawn_blocking.
**When to use:** Called by Phase 7's PhaseRunner.
**Example:**
```rust
// Source: tokio docs - spawn_blocking pattern
use tokio::task;

impl GitLayer {
    pub async fn commit_phase_async(
        &self,
        files: Vec<PathBuf>,
        metadata: CommitMetadata,
    ) -> Result<CommitResult, GitError> {
        let repo = Arc::clone(&self.repo);
        let repo_path = self.repo_path.clone();
        task::spawn_blocking(move || {
            let repo = repo.lock().map_err(|_| GitError::LockPoisoned)?;
            // ... sync commit logic using repo ...
            Ok(CommitResult { /* ... */ })
        })
        .await
        .map_err(|e| GitError::TaskJoin { source: e.to_string() })?
    }
}
```

### Anti-Patterns to Avoid
- **Opening repo per operation:** Open once in `new()`, reuse via Arc<Mutex>. Opening is expensive.
- **Using `add_all` or glob patterns:** Breaks the explicit-file-list contract. Always stage individual files.
- **Forgetting `index.write()` before `write_tree()`:** The tree is built from the on-disk index, not the in-memory one. Must write first.
- **Using `Signature::now()` for both author and committer:** Author should be Athena, committer should be system default. Use `repo.signature()` for committer (reads from git config) with `Signature::now()` as fallback.
- **Holding Mutex lock across await points:** Never. The lock is held only inside the spawn_blocking closure.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Git trailer parsing/formatting | Custom parser | Standard `Key: Value` format after blank line | Git trailers are a well-defined convention; `git interpret-trailers` and `git log --format=%(trailers)` parse them natively |
| Repository initialization | Custom .git dir creation | `Repository::init()` | Creates proper .git structure with all required files |
| File staging | Direct blob/tree construction | `Index::add_path()` / `Index::remove_path()` | Index handles file mode detection, blob creation, and tree building |
| Conflict detection | Manual index scanning | `Index::has_conflicts()` | Returns bool directly; `conflicts()` iterator for details if needed |
| Empty repo detection | HEAD ref parsing | `Repository::is_empty()` | Handles all edge cases (unborn branch, detached HEAD on empty) |

**Key insight:** git2 provides high-level methods for every operation GitLayer needs. The libgit2 C API has been stable for years; the Rust bindings faithfully expose it. There is no need to drop to low-level blob/tree/ref manipulation.

## Common Pitfalls

### Pitfall 1: Path Must Be Relative to Repo Root
**What goes wrong:** `Index::add_path()` takes paths relative to the repository working directory. Passing absolute paths causes "file not found" errors.
**Why it happens:** The caller may have absolute paths from file production.
**How to avoid:** Strip the repo root prefix from all file paths before staging. Use `path.strip_prefix(&repo_path)`.
**Warning signs:** `git2::Error` with message containing "could not find" during add_path.

### Pitfall 2: Index Write Before Write Tree
**What goes wrong:** `index.write_tree()` reads from the on-disk index. If you modify the in-memory index but skip `index.write()`, `write_tree()` returns a tree that doesn't reflect your staged changes.
**Why it happens:** git2 mirrors libgit2's two-phase design: in-memory modifications must be flushed to disk.
**How to avoid:** Always call `index.write()` immediately before `index.write_tree()`.
**Warning signs:** Commits that appear empty despite staging files.

### Pitfall 3: Signature Fallback for Missing Git Config
**What goes wrong:** `Repository::signature()` reads user.name/user.email from git config. If not configured (common in CI, containers, fresh installs), it returns an error.
**Why it happens:** Many environments don't have global git config set.
**How to avoid:** For the committer signature, try `repo.signature()` first, fall back to `Signature::now("Athena", "athena@noreply")`. The author is always `Signature::now("Athena", "athena@noreply")` per the locked decision.
**Warning signs:** `git2::Error` with "config value 'user.name' was not found".

### Pitfall 4: Mutex Poisoning on Panic
**What goes wrong:** If a thread panics while holding the Mutex<Repository>, subsequent lock attempts return PoisonError.
**Why it happens:** Rust's Mutex is poisoned after a panic to prevent using potentially inconsistent state.
**How to avoid:** Map PoisonError to GitError::LockPoisoned. In practice, git2 operations don't panic (they return Result), so this is a safety net.
**Warning signs:** "poisoned lock" error messages.

### Pitfall 5: Empty Diff After Staging
**What goes wrong:** Files are staged but the diff between the new tree and HEAD tree is empty (files haven't actually changed).
**Why it happens:** Agent declares files_produced but the content is identical to what's already committed.
**How to avoid:** After building the tree, compare it to the parent commit's tree OID. If equal, skip the commit and return `CommitResult { committed: false }`.
**Warning signs:** Git log shows commits with no actual changes.

### Pitfall 6: libgit2-sys Build on Windows
**What goes wrong:** git2 depends on libgit2-sys which compiles C code. On Windows, this requires a C compiler (MSVC or MinGW).
**Why it happens:** libgit2 is a C library compiled from source by default.
**How to avoid:** Ensure the build environment has MSVC build tools (standard with Rust on Windows). The project is already building Rust code so this should be present.
**Warning signs:** Build errors mentioning "cc" or "link.exe" not found.

## Code Examples

### Complete Staging and Commit Flow
```rust
// Source: git2 0.20.4 docs (docs.rs/git2)
fn commit_phase(
    repo: &Repository,
    repo_path: &Path,
    files: &[PathBuf],
    metadata: &CommitMetadata,
) -> Result<CommitResult, GitError> {
    // 1. Check for dirty working tree (skip for empty repos)
    if !repo.is_empty()? {
        let statuses = repo.statuses(None)?;
        // Filter out statuses that we're about to modify
        let dirty = statuses.iter().any(|s| {
            let dominated = s.status().intersects(
                git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
            );
            // Only WT (working tree) changes from the user are "dirty"
            s.status().intersects(
                git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_NEW
            )
        });
        if dirty {
            return Err(GitError::DirtyWorkingTree { /* ... */ });
        }
    }

    // 2. Stage files
    let mut index = repo.index()?;
    for file in files {
        let rel = file.strip_prefix(repo_path).unwrap_or(file);
        let abs = repo_path.join(rel);
        if abs.exists() {
            index.add_path(rel)?;
        } else {
            index.remove_path(rel).map_err(|_| GitError::FileMissing {
                path: rel.to_path_buf(),
                hint: "File declared in files_produced but not found on disk".into(),
            })?;
        }
    }
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // 3. Check for empty diff
    let is_initial = repo.is_empty()?;
    if !is_initial {
        let head_commit = repo.head()?.peel_to_commit()?;
        if head_commit.tree_id() == tree_oid {
            return Ok(CommitResult { committed: false, oid: None });
        }
    }

    // 4. Build signatures
    let author = Signature::now("Athena", "athena@noreply")?;
    let committer = repo.signature()
        .unwrap_or_else(|_| Signature::now("Athena", "athena@noreply").unwrap());

    // 5. Build message with trailers
    let message = build_commit_message(metadata);

    // 6. Create commit
    let parents: Vec<git2::Commit> = if is_initial {
        vec![]
    } else {
        vec![repo.head()?.peel_to_commit()?]
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = repo.commit(Some("HEAD"), &author, &committer, &message, &tree, &parent_refs)?;

    Ok(CommitResult { committed: true, oid: Some(oid) })
}
```

### Dirty Tree Check (Detailed)
```rust
// Source: git2 StatusOptions docs
fn is_working_tree_dirty(repo: &Repository) -> Result<bool, git2::Error> {
    if repo.is_empty()? {
        return Ok(false);  // Empty repo can't be dirty
    }
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(!statuses.is_empty())
}
```

### Conflict Detection
```rust
// Source: git2 Index::has_conflicts docs
fn check_conflicts(repo: &Repository) -> Result<(), GitError> {
    let index = repo.index()?;
    if index.has_conflicts() {
        return Err(GitError::MergeConflict {
            hint: "Resolve merge conflicts before running Athena".into(),
        });
    }
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| git2 0.19.x | git2 0.20.4 | Jan 2025 | Requires libgit2 1.9.0; no API breaking changes for our use case |
| libgit2-sys manual build | Bundled build (default) | Long-standing | No system dependency needed; `vendored-libgit2` feature available for extra isolation |
| gitoxide (gix) as replacement | Still maturing | Ongoing | Pure Rust but API stability not yet at git2 level; not recommended for this use case |

**Deprecated/outdated:**
- git2 `vendored-openssl` feature: not needed unless doing remote operations (push/fetch). GitLayer is local-only.
- `Repository::discover()`: walks parent directories to find .git. Explicitly rejected in CONTEXT.md in favor of explicit path.

## Open Questions

1. **Committer signature when git config is absent**
   - What we know: `repo.signature()` fails without user.name/user.email in git config
   - What's unclear: Whether using Athena as both author AND committer (as fallback) is acceptable UX
   - Recommendation: Fall back to Athena signature for committer too. Document this behavior. Users who care can set git config.

2. **Dirty tree check granularity**
   - What we know: CONTEXT.md says "error on dirty working tree"
   - What's unclear: Should untracked files count as dirty? (e.g., user has new files not yet committed)
   - Recommendation: Yes, include untracked files. This matches how `cargo release` and similar tools work. Better safe than sorry.

3. **File deletion detection edge case**
   - What we know: If a file in files_produced doesn't exist, we should stage deletion if it was previously tracked
   - What's unclear: Whether `Index::remove_path()` fails gracefully if the file was never tracked
   - Recommendation: `remove_path` returns Err if the path isn't in the index. Use this error to distinguish "deletion" from "file never existed" (the latter is a bug).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + cargo test |
| Config file | None needed -- cargo test works out of the box |
| Quick run command | `cargo test -p ath-git` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OUTP-01a | After phase completes, git commit appears with only phase files | integration | `cargo test -p ath-git -- commit_contains_only_staged_files` | No -- Wave 0 |
| OUTP-01b | Commit message includes phase name, agent, task identifier (trailers) | unit | `cargo test -p ath-git -- commit_message_has_trailers` | No -- Wave 0 |
| OUTP-01c | git log shows one commit per executed phase | integration | `cargo test -p ath-git -- one_commit_per_phase` | No -- Wave 0 |
| OUTP-01d | Works in repo with no prior commits (initial commit) | integration | `cargo test -p ath-git -- initial_commit_empty_repo` | No -- Wave 0 |
| EDGE-01 | Empty diff skips commit silently | unit | `cargo test -p ath-git -- empty_diff_skips_commit` | No -- Wave 0 |
| EDGE-02 | Missing declared file returns error | unit | `cargo test -p ath-git -- missing_file_errors` | No -- Wave 0 |
| EDGE-03 | Dirty working tree returns error | integration | `cargo test -p ath-git -- dirty_tree_errors` | No -- Wave 0 |
| EDGE-04 | Merge conflict detected and reported | unit | `cargo test -p ath-git -- conflict_detection` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-git`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/ath-git/tests/integration.rs` -- integration tests using tempfile for isolated repos
- [ ] `crates/ath-git/src/error.rs` -- GitError type needed before tests can compile
- [ ] Add `tempfile = { workspace = true }` to workspace dependencies and ath-git dev-dependencies
- [ ] Add `git2 = { workspace = true }` to workspace dependencies

## Sources

### Primary (HIGH confidence)
- [git2 0.20.4 docs - Repository](https://docs.rs/git2/0.20.4/git2/struct.Repository.html) - init, open, commit, is_empty, statuses, head, signature methods
- [git2 0.20.4 docs - Index](https://docs.rs/git2/0.20.4/git2/struct.Index.html) - add_path, remove_path, write, write_tree, has_conflicts, conflicts
- [git2 0.20.4 docs - Signature](https://docs.rs/git2/0.20.4/git2/struct.Signature.html) - now, new constructors
- [git2-rs GitHub](https://github.com/rust-lang/git2-rs) - examples/init.rs for initial commit pattern
- [git2 crates.io](https://crates.io/crates/git2) - version 0.20.4, released 2026-02-02

### Secondary (MEDIUM confidence)
- [Tokio bridging with sync code](https://tokio.rs/tokio/topics/bridging) - spawn_blocking patterns for wrapping sync libraries
- [Rust forum: Arc with spawn_blocking](https://users.rust-lang.org/t/when-some-functions-require-arc-self-due-to-tokio-spawn-blocking-whats-the-best-practice/92755) - Arc<Mutex> ownership pattern

### Tertiary (LOW confidence)
- None -- all findings verified with official documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - git2 0.20.4 is current, docs verified, API stable
- Architecture: HIGH - patterns derived from official git2 examples and docs; sync core + async wrapper is established Rust pattern
- Pitfalls: HIGH - path relativity, index write ordering, and signature fallback are well-documented gotchas in git2 issue tracker

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (git2 API is very stable; 30 days conservative)