//! # ath-git
//!
//! Git operations for the Athena orchestrator.
//!
//! Provides `GitLayer` for in-process git operations (init, status, staging, commit)
//! using git2, with structured commit messages via git trailers.

pub mod commit;
pub mod error;
pub mod layer;

pub use commit::{build_commit_message, CommitMetadata};
pub use error::GitError;
pub use layer::GitLayer;
