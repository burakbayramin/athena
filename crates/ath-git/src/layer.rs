//! GitLayer -- sync repository management.
//!
//! Wraps a `git2::Repository` in `Arc<Mutex<_>>` for thread-safe access.
//! Provides methods for opening/initializing repos, checking dirty state,
//! and detecting merge conflicts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use git2::Repository;

use crate::error::GitError;

/// Thread-safe wrapper around a git2 Repository.
///
/// Opened once via `new()` and reused for the lifetime of the struct.
/// The `Arc<Mutex<Repository>>` enables safe sharing across `spawn_blocking` calls.
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Init with git2 directly
        git2::Repository::init(dir.path()).expect("git init");
        // Now open with GitLayer
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

        // Create an initial commit so the repo is non-empty
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

        // Now write a file -- should be dirty (untracked)
        std::fs::write(dir.path().join("new_file.txt"), "hello").expect("write file");
        assert!(layer.is_dirty().expect("is_dirty"));
    }

    #[test]
    fn clean_repo_not_dirty() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");

        // Create initial commit with empty tree
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

        // No modifications -- should not be dirty
        assert!(!layer.is_dirty().expect("is_dirty"));
    }

    #[test]
    fn check_conflicts_ok_when_no_conflicts() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let layer = GitLayer::new(dir.path().to_path_buf()).expect("init");
        assert!(layer.check_conflicts().is_ok());
    }
}
