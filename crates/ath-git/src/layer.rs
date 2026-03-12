//! GitLayer -- sync repository management.
//!
//! Wraps a `git2::Repository` in `Arc<Mutex<_>>` for thread-safe access.
//! Provides methods for opening/initializing repos, checking dirty state,
//! detecting merge conflicts, and staging/committing files.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use git2::Repository;

use crate::commit::{build_commit_message, CommitMetadata};
use crate::error::GitError;

/// Result of a `stage_and_commit` operation.
#[derive(Debug, Clone)]
pub struct CommitResult {
    /// Whether a commit was actually created.
    /// `false` when the staged tree is identical to HEAD (empty diff).
    pub committed: bool,
    /// The OID of the created commit, if any.
    pub oid: Option<git2::Oid>,
}

/// Thread-safe wrapper around a git2 Repository.
///
/// Opened once via `new()` and reused for the lifetime of the struct.
/// The `Arc<Mutex<Repository>>` enables safe sharing across `spawn_blocking` calls.
#[derive(Clone)]
pub struct GitLayer {
    repo: Arc<Mutex<Repository>>,
    repo_path: PathBuf,
}

impl GitLayer {
    /// Open an existing repository or auto-initialize a new one at the given path.
    ///
    /// If `.git` exists at `path`, opens it. Otherwise, calls `git2::Repository::init`.
    pub fn new(path: PathBuf) -> Result<Self, GitError> {
        let repo = if path.join(".git").exists() {
            Repository::open(&path).map_err(|e| GitError::RepositoryOpen {
                path: path.clone(),
                source: e,
            })?
        } else {
            Repository::init(&path).map_err(|e| GitError::RepositoryOpen {
                path: path.clone(),
                source: e,
            })?
        };

        Ok(Self {
            repo: Arc::new(Mutex::new(repo)),
            repo_path: path,
        })
    }

    /// Check if the working tree has uncommitted changes (including untracked files).
    ///
    /// Returns `false` for empty (just-initialized) repos since there are no tracked files.
    pub fn is_dirty(&self) -> Result<bool, GitError> {
        let repo = self.repo.lock().map_err(|_| GitError::LockPoisoned)?;

        if repo
            .is_empty()
            .map_err(|e| GitError::RepositoryOpen {
                path: self.repo_path.clone(),
                source: e,
            })?
        {
            return Ok(false);
        }

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true).include_ignored(false);

        let statuses = repo.statuses(Some(&mut opts)).map_err(|e| GitError::RepositoryOpen {
            path: self.repo_path.clone(),
            source: e,
        })?;

        Ok(!statuses.is_empty())
    }

    /// Check for unresolved merge conflicts in the index.
    ///
    /// Returns `Ok(())` if no conflicts, `Err(GitError::MergeConflict)` if conflicts exist.
    pub fn check_conflicts(&self) -> Result<(), GitError> {
        let repo = self.repo.lock().map_err(|_| GitError::LockPoisoned)?;

        let index = repo.index().map_err(|e| GitError::RepositoryOpen {
            path: self.repo_path.clone(),
            source: e,
        })?;

        if index.has_conflicts() {
            return Err(GitError::MergeConflict {
                hint: "Resolve merge conflicts before running Athena".to_string(),
            });
        }

        Ok(())
    }

    /// Get the path to the repository working directory.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Get a clone of the Arc<Mutex<Repository>> for sharing.
    pub(crate) fn repo_handle(&self) -> Arc<Mutex<Repository>> {
        Arc::clone(&self.repo)
    }

    /// Stage the given files and create a commit with trailers.
    ///
    /// - Files must be specified as absolute or repo-relative paths.
    /// - If a file exists on disk, it is added to the index.
    /// - If a file does not exist on disk, it is removed from the index (deletion).
    ///   If it was never tracked, `GitError::FileMissing` is returned.
    /// - If the resulting tree is identical to HEAD's tree, no commit is created
    ///   and `CommitResult { committed: false, oid: None }` is returned.
    /// - For empty repos (no HEAD), this creates the initial commit.
    pub fn stage_and_commit(
        &self,
        files: &[PathBuf],
        metadata: &CommitMetadata,
    ) -> Result<CommitResult, GitError> {
        let repo = self.repo.lock().map_err(|_| GitError::LockPoisoned)?;

        // Check for merge conflicts
        {
            let index = repo.index().map_err(|e| GitError::RepositoryOpen {
                path: self.repo_path.clone(),
                source: e,
            })?;
            if index.has_conflicts() {
                return Err(GitError::MergeConflict {
                    hint: "Resolve merge conflicts before running Athena".to_string(),
                });
            }
        }

        // Stage files
        let mut index = repo.index().map_err(|e| GitError::RepositoryOpen {
            path: self.repo_path.clone(),
            source: e,
        })?;

        for file in files {
            let relative = file
                .strip_prefix(&self.repo_path)
                .unwrap_or(file);

            let abs_path = self.repo_path.join(relative);
            if abs_path.exists() {
                index.add_path(relative).map_err(|e| GitError::StagingFailed {
                    path: file.clone(),
                    source: e,
                })?;
            } else {
                // File doesn't exist on disk -- check if it's tracked in the index.
                // If tracked, remove it (deletion). If not tracked, error.
                let is_tracked = index
                    .get_path(relative, 0)
                    .is_some();
                if is_tracked {
                    index.remove_path(relative).map_err(|e| GitError::StagingFailed {
                        path: file.clone(),
                        source: e,
                    })?;
                } else {
                    return Err(GitError::FileMissing {
                        path: file.clone(),
                        hint: format!(
                            "File '{}' does not exist on disk and is not tracked",
                            relative.display()
                        ),
                    });
                }
            }
        }

        index.write().map_err(|e| GitError::StagingFailed {
            path: self.repo_path.clone(),
            source: e,
        })?;

        let tree_oid = index.write_tree().map_err(|e| GitError::StagingFailed {
            path: self.repo_path.clone(),
            source: e,
        })?;

        // Check if repo is empty (no HEAD)
        let is_empty = repo.is_empty().map_err(|e| GitError::RepositoryOpen {
            path: self.repo_path.clone(),
            source: e,
        })?;

        // Detect empty diff: compare new tree to HEAD's tree
        if !is_empty {
            let head_commit = repo
                .head()
                .and_then(|r| r.peel_to_commit())
                .map_err(|e| GitError::CommitFailed { source: e })?;
            if head_commit.tree_id() == tree_oid {
                return Ok(CommitResult {
                    committed: false,
                    oid: None,
                });
            }
        }

        let tree = repo.find_tree(tree_oid).map_err(|e| GitError::CommitFailed { source: e })?;

        // Build signatures
        let author = git2::Signature::now("Athena", "athena@noreply")
            .map_err(|e| GitError::CommitFailed { source: e })?;
        let committer = repo
            .signature()
            .unwrap_or_else(|_| git2::Signature::now("Athena", "athena@noreply").unwrap());

        // Build message
        let message = build_commit_message(metadata);

        // Determine parents
        let parents = if is_empty {
            vec![]
        } else {
            let head_commit = repo
                .head()
                .and_then(|r| r.peel_to_commit())
                .map_err(|e| GitError::CommitFailed { source: e })?;
            vec![head_commit]
        };

        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo
            .commit(
                Some("HEAD"),
                &author,
                &committer,
                &message,
                &tree,
                &parent_refs,
            )
            .map_err(|e| GitError::CommitFailed { source: e })?;

        Ok(CommitResult {
            committed: true,
            oid: Some(oid),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::CommitMetadata;

    fn sample_meta() -> CommitMetadata {
        CommitMetadata {
            phase_name: "test-phase".to_string(),
            agent_provider: "Anthropic".to_string(),
            agent_model: "opus-4".to_string(),
            task_id: "task-001".to_string(),
            files_count: 1,
            review_status: None,
            reviewer: None,
        }
    }

    #[test]
    fn git_layer_auto_inits_new_repo() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("auto-init");
        assert!(dir.path().join(".git").exists());
        assert_eq!(layer.repo_path(), dir.path());
    }

    #[test]
    fn git_layer_opens_existing_repo() {
        let dir = tempfile::tempdir().expect("create tempdir");
        git2::Repository::init(dir.path()).expect("git init");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("open existing");
        assert_eq!(layer.repo_path(), dir.path());
    }

    #[test]
    fn empty_repo_not_dirty() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");
        assert!(!layer.is_dirty().expect("is_dirty"));
    }

    #[test]
    fn dirty_after_modification() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        {
            let repo = layer.repo_handle();
            let repo = repo.lock().unwrap();
            let sig = git2::Signature::now("Test", "test@test").unwrap();
            let mut index = repo.index().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        std::fs::write(dir.path().join("new_file.txt"), "hello").expect("write file");
        assert!(layer.is_dirty().expect("is_dirty"));
    }

    #[test]
    fn clean_repo_not_dirty() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        {
            let repo = layer.repo_handle();
            let repo = repo.lock().unwrap();
            let sig = git2::Signature::now("Test", "test@test").unwrap();
            let mut index = repo.index().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }

        assert!(!layer.is_dirty().expect("is_dirty"));
    }

    #[test]
    fn check_conflicts_ok_when_no_conflicts() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");
        assert!(layer.check_conflicts().is_ok());
    }

    // --- TDD tests for stage_and_commit ---

    #[test]
    fn commit_contains_only_staged_files() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        // Write 3 files
        std::fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        std::fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        std::fs::write(dir.path().join("c.txt"), "ccc").unwrap();

        // Stage only a.txt and b.txt
        let result = layer
            .stage_and_commit(
                &[dir.path().join("a.txt"), dir.path().join("b.txt")],
                &CommitMetadata {
                    files_count: 2,
                    ..sample_meta()
                },
            )
            .expect("stage_and_commit");

        assert!(result.committed);
        assert!(result.oid.is_some());

        // Verify the commit tree contains only a.txt and b.txt
        let repo = layer.repo_handle();
        let repo = repo.lock().unwrap();
        let commit = repo.find_commit(result.oid.unwrap()).unwrap();
        let tree = commit.tree().unwrap();

        assert!(tree.get_name("a.txt").is_some(), "a.txt should be in tree");
        assert!(tree.get_name("b.txt").is_some(), "b.txt should be in tree");
        assert!(tree.get_name("c.txt").is_none(), "c.txt should NOT be in tree");
    }

    #[test]
    fn commit_message_has_trailers() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        std::fs::write(dir.path().join("file.txt"), "content").unwrap();

        let result = layer
            .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
            .expect("stage_and_commit");

        let repo = layer.repo_handle();
        let repo = repo.lock().unwrap();
        let commit = repo.find_commit(result.oid.unwrap()).unwrap();
        let message = commit.message().unwrap();

        assert!(message.starts_with("athena:"), "subject should start with athena:");
        assert!(message.contains("Phase: test-phase"), "should have Phase trailer");
        assert!(
            message.contains("Agent: Anthropic/opus-4"),
            "should have Agent trailer"
        );
        assert!(message.contains("Task-Id: task-001"), "should have Task-Id trailer");
        assert!(message.contains("Files-Count: 1"), "should have Files-Count trailer");
    }

    #[test]
    fn initial_commit_empty_repo() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        std::fs::write(dir.path().join("init.txt"), "hello").unwrap();

        let result = layer
            .stage_and_commit(&[dir.path().join("init.txt")], &sample_meta())
            .expect("stage_and_commit on empty repo");

        assert!(result.committed);
        assert!(result.oid.is_some());

        // HEAD should now exist
        let repo = layer.repo_handle();
        let repo = repo.lock().unwrap();
        assert!(repo.head().is_ok(), "HEAD should exist after initial commit");
    }

    #[test]
    fn one_commit_per_phase() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        // First commit
        std::fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        layer
            .stage_and_commit(
                &[dir.path().join("a.txt")],
                &CommitMetadata {
                    phase_name: "phase-1".to_string(),
                    ..sample_meta()
                },
            )
            .expect("first commit");

        // Second commit
        std::fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        layer
            .stage_and_commit(
                &[dir.path().join("b.txt")],
                &CommitMetadata {
                    phase_name: "phase-2".to_string(),
                    ..sample_meta()
                },
            )
            .expect("second commit");

        // Count commits via revwalk
        let repo = layer.repo_handle();
        let repo = repo.lock().unwrap();
        let mut revwalk = repo.revwalk().unwrap();
        revwalk.push_head().unwrap();
        let count = revwalk.count();
        assert_eq!(count, 2, "should have exactly 2 commits");
    }

    #[test]
    fn empty_diff_skips_commit() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        // Create initial commit
        std::fs::write(dir.path().join("file.txt"), "content").unwrap();
        let first = layer
            .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
            .expect("first commit");
        assert!(first.committed);

        // Stage same unchanged file again
        let second = layer
            .stage_and_commit(&[dir.path().join("file.txt")], &sample_meta())
            .expect("second stage_and_commit");
        assert!(!second.committed, "should skip commit for empty diff");
        assert!(second.oid.is_none());
    }

    #[test]
    fn missing_file_returns_error() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        let result = layer.stage_and_commit(
            &[dir.path().join("nonexistent.txt")],
            &sample_meta(),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, GitError::FileMissing { .. }),
            "expected FileMissing, got: {:?}",
            err
        );
    }
}
