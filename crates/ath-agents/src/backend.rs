//! The `AgentBackend` trait defining the uniform async interface for all providers.
//!
//! All LLM provider implementations (Claude, Gemini, Codex) and test doubles
//! implement this trait. It provides send, availability checking, and provider
//! identification methods.

use std::sync::Arc;

use async_trait::async_trait;
use ath_types::agent::{AgentRequest, AgentResponse};

use crate::error::AgentError;

/// Callback type for streaming content chunks.
pub type ChunkCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// A uniform async interface for LLM provider backends.
///
/// Implementors must be `Send + Sync` to allow use across async task boundaries.
/// Real implementations use internal actors; test doubles implement directly.
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Send a request to the agent and wait for a response.
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;

    /// Send a request with streaming chunk callback.
    ///
    /// The callback receives each content chunk as it arrives from the provider.
    /// Default implementation ignores the callback and delegates to `send`.
    async fn send_streaming(
        &self,
        request: AgentRequest,
        on_chunk: Option<ChunkCallback>,
    ) -> Result<AgentResponse, AgentError> {
        let _ = on_chunk;
        self.send(request).await
    }

    /// Check if the backend is currently available (e.g., circuit breaker not tripped).
    async fn is_available(&self) -> bool;

    /// Provider name for logging and error reporting.
    fn provider_name(&self) -> &str;
}
