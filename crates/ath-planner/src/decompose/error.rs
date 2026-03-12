//! Error types for the decomposition process.

use thiserror::Error;

/// Errors that can occur during project decomposition.
#[derive(Debug, Error)]
pub enum DecomposeError {
    /// Decomposition failed after exhausting retry attempts.
    #[error("Decomposition failed after {attempts} attempts")]
    DecomposeFailed {
        attempts: usize,
        last_error: Option<String>,
    },

    /// The underlying agent returned an error.
    #[error("Agent error: {0}")]
    Agent(#[from] ath_agents::error::AgentError),

    /// The decomposition output failed validation.
    #[error("Validation errors: {0:?}")]
    Validation(Vec<ath_types::ValidationError>),
}
