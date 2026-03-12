//! Async wrapper for GitLayer using `spawn_blocking`.
//!
//! Since git2 is a synchronous library, `AsyncGitLayer` bridges it to async
//! callers by running operations inside `tokio::task::spawn_blocking`.

use std::path::PathBuf;

use crate::commit::CommitMetadata;
use crate::error::GitError;
use crate::layer::{CommitResult, GitLayer};

/// Async facade over [`GitLayer`] for use in tokio runtimes.
///
/// Internally clones the `GitLayer` (which uses `Arc<Mutex<Repository>>`)
/// into `spawn_blocking` closures so that synchronous git2 calls do not
/// block the async runtime.
#[derive(Clone)]
pub struct AsyncGitLayer {
    inner: GitLayer,
}

impl AsyncGitLayer {
    /// Create a new `AsyncGitLayer` by constructing a `GitLayer` at the given path.
    pub fn new(path: PathBuf) -> Result<Self, GitError> {
        let inner = GitLayer::new(path)?;
        Ok(Self { inner })
    }

    /// Async version of [`GitLayer::stage_and_commit`].
    ///
    /// Runs the synchronous commit operation inside `spawn_blocking`.
    pub async fn commit_phase_async(
        &self,
        files: Vec<PathBuf>,
        metadata: CommitMetadata,
    ) -> Result<CommitResult, GitError> {
        let layer = self.inner.clone();
        tokio::task::spawn_blocking(move || layer.stage_and_commit(&files, &metadata))
            .await
            .map_err(|e| GitError::TaskJoin {
                message: e.to_string(),
            })?
    }

    /// Async version of [`GitLayer::is_dirty`].
    ///
    /// Runs the synchronous check inside `spawn_blocking`.
    pub async fn is_dirty_async(&self) -> Result<bool, GitError> {
        let layer = self.inner.clone();
        tokio::task::spawn_blocking(move || layer.is_dirty())
            .await
            .map_err(|e| GitError::TaskJoin {
                message: e.to_string(),
            })?
    }

    /// Get a reference to the inner [`GitLayer`].
    pub fn inner(&self) -> &GitLayer {
        &self.inner
    }
}
