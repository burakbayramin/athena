//! Input parsing error types with fix hints.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during input mode resolution and parsing.
#[derive(Debug, Error)]
pub enum InputError {
    /// No input was provided (no description, --spec, or --codebase).
    #[error("No input provided")]
    NoInput { hint: String },

    /// The specified spec file was not found.
    #[error("Spec file not found: {path}")]
    SpecFileNotFound { path: PathBuf, hint: String },

    /// The spec file exceeds the size limit.
    #[error("Spec file too large: {size} bytes (max {max} bytes)")]
    SpecFileTooLarge { size: u64, max: u64, hint: String },

    /// The specified codebase directory was not found.
    #[error("Codebase path not found: {path}")]
    CodebaseNotFound { path: PathBuf, hint: String },

    /// --codebase was provided without a description.
    #[error("Codebase mode requires a description")]
    CodebaseWithoutIntent { hint: String },

    /// --spec cannot be combined with other input modes.
    #[error("--spec cannot be combined with other input modes")]
    SpecExclusive { hint: String },

    /// Failed to parse ProjectSpec after multiple attempts.
    #[error("Failed to parse ProjectSpec after {attempts} attempts")]
    ParseFailed {
        attempts: usize,
        last_error: Option<String>,
    },

    /// An error from the agent layer.
    #[error("Agent error: {0}")]
    Agent(#[from] ath_agents::error::AgentError),

    /// An IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl InputError {
    /// Returns a user-facing hint for how to fix the error.
    pub fn hint(&self) -> &str {
        match self {
            InputError::NoInput { hint } => hint,
            InputError::SpecFileNotFound { hint, .. } => hint,
            InputError::SpecFileTooLarge { hint, .. } => hint,
            InputError::CodebaseNotFound { hint, .. } => hint,
            InputError::CodebaseWithoutIntent { hint } => hint,
            InputError::SpecExclusive { hint } => hint,
            InputError::ParseFailed { .. } => "Check the project description and try again",
            InputError::Agent(_) => "Check your API key configuration and try again",
            InputError::Io(_) => "Check file permissions and try again",
        }
    }
}
