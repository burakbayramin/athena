//! Domain-specific error types for git operations.
//!
//! Each variant includes a fix hint to help users resolve the issue.

use std::path::PathBuf;

/// Git operation errors with fix hints for all failure modes.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// Failed to open or initialize a git repository.
    #[error("failed to open repository at {path}: {source}")]
    RepositoryOpen {
        /// The path that was attempted.
        path: PathBuf,
        /// The underlying git2 error.
        source: git2::Error,
    },

    /// Working tree has uncommitted changes.
    #[error("dirty working tree: {hint}")]
    DirtyWorkingTree {
        /// Fix hint for the user.
        hint: String,
    },

    /// Index has unresolved merge conflicts.
    #[error("merge conflict detected: {hint}")]
    MergeConflict {
        /// Fix hint for the user.
        hint: String,
    },

    /// A declared file does not exist on disk and was not previously tracked.
    #[error("file missing: {path}: {hint}")]
    FileMissing {
        /// The path of the missing file.
        path: PathBuf,
        /// Fix hint for the user.
        hint: String,
    },

    /// git2 index operation (add/remove) failed.
    #[error("staging failed for {path}: {source}")]
    StagingFailed {
        /// The path that failed to stage.
        path: PathBuf,
        /// The underlying git2 error.
        source: git2::Error,
    },

    /// git2 commit call failed.
    #[error("commit failed: {source}")]
    CommitFailed {
        /// The underlying git2 error.
        source: git2::Error,
    },

    /// Mutex was poisoned (a thread panicked while holding the lock).
    #[error("repository lock poisoned -- a previous operation panicked")]
    LockPoisoned,

    /// spawn_blocking join error.
    #[error("task join error: {message}")]
    TaskJoin {
        /// The error message from the join failure.
        message: String,
    },
}
