//! The `AgentBackend` trait defining the uniform async interface for all providers.
//!
//! All LLM provider implementations (Claude, Gemini, Codex) and test doubles
//! implement this trait. It provides send, availability checking, and provider
//! identification methods.

use async_trait::async_trait;
use ath_types::agent::{AgentRequest, AgentResponse};

use crate::error::AgentError;

/// A uniform async interface for LLM provider backends.
///
/// Implementors must be `Send + Sync` to allow use across async task boundaries.
/// Real implementations use internal actors; test doubles implement directly.
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Send a request to the agent and wait for a response.
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;

    /// Check if the backend is currently available (e.g., circuit breaker not tripped).
    async fn is_available(&self) -> bool;

    /// Provider name for logging and error reporting.
    fn provider_name(&self) -> &str;
}
