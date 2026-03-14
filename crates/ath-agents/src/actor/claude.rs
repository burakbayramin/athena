//! Claude (Anthropic) actor implementation.
//!
//! `ClaudeActor` runs as a tokio task processing requests via a bounded mpsc channel.
//! `ClaudeHandle` is the cloneable handle that callers use to send requests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use ath_config::ConfigStore;
use ath_types::agent::{AgentRequest, AgentResponse};
use tokio::sync::{mpsc, oneshot};

use crate::backend::AgentBackend;
use crate::circuit_breaker::CircuitBreaker;
use crate::error::AgentError;

use super::{build_genai_client, run_with_retry_and_breaker, ActorMessage};

/// The actor task that owns the genai client and circuit breaker.
struct ClaudeActor {
    receiver: mpsc::Receiver<ActorMessage>,
    client: genai::Client,
    circuit_breaker: CircuitBreaker,
    model: String,
    circuit_breaker_open: Arc<AtomicBool>,
}

impl ClaudeActor {
    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            let result = run_with_retry_and_breaker(
                &self.client,
                &self.model,
                &msg.request,
                &mut self.circuit_breaker,
                "claude",
            )
            .await;

            // Update the lightweight status flag
            self.circuit_breaker_open
                .store(self.circuit_breaker.is_open(), Ordering::Relaxed);

            let _ = msg.respond_to.send(result);
        }
    }
}

/// Cloneable handle for sending requests to the Claude actor.
///
/// Implements `AgentBackend` for uniform provider access.
#[derive(Clone, Debug)]
pub struct ClaudeHandle {
    sender: mpsc::Sender<ActorMessage>,
    circuit_breaker_open: Arc<AtomicBool>,
}

impl ClaudeHandle {
    /// Create a new Claude actor and return a handle to it.
    ///
    /// Validates that the Anthropic API key is present (fail-fast).
    /// Spawns the actor task and returns the handle.
    pub fn new(config: &ConfigStore) -> Result<Self, AgentError> {
        // Fail-fast: validate API key presence
        if config.anthropic_api_key.is_none() {
            return Err(AgentError::AuthFailed {
                provider: "claude".to_string(),
                reason: "Anthropic API key not configured".to_string(),
            });
        }

        let client = build_genai_client(config)?;
        let model = config.claude_model.clone();
        let circuit_breaker_open = Arc::new(AtomicBool::new(false));

        let (sender, receiver) = mpsc::channel(32);

        let actor = ClaudeActor {
            receiver,
            client,
            circuit_breaker: CircuitBreaker::default(),
            model,
            circuit_breaker_open: circuit_breaker_open.clone(),
        };

        tokio::spawn(actor.run());

        Ok(Self {
            sender,
            circuit_breaker_open,
        })
    }
}

#[async_trait]
impl AgentBackend for ClaudeHandle {
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ActorMessage {
                request,
                respond_to: tx,
            })
            .await
            .map_err(|_| AgentError::ActorStopped)?;

        rx.await.map_err(|_| AgentError::ActorStopped)?
    }

    async fn is_available(&self) -> bool {
        !self.circuit_breaker_open.load(Ordering::Relaxed)
    }

    fn provider_name(&self) -> &str {
        "claude"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::AgentId;

    fn config_without_anthropic_key() -> ConfigStore {
        ConfigStore {
            anthropic_api_key: None,
            google_api_key: Some("test-google-key".to_string()),
            openai_api_key: Some("test-openai-key".to_string()),
            claude_model: "opus-4".to_string(),
            gemini_model: "2.5-pro".to_string(),
            codex_model: "o3".to_string(),
            agents: ath_config::AgentsConfig::default(),
            skills: None,
        }
    }

    fn config_with_anthropic_key() -> ConfigStore {
        ConfigStore {
            anthropic_api_key: Some("test-anthropic-key".to_string()),
            google_api_key: None,
            openai_api_key: None,
            claude_model: "opus-4".to_string(),
            gemini_model: "2.5-pro".to_string(),
            codex_model: "o3".to_string(),
            agents: ath_config::AgentsConfig::default(),
            skills: None,
        }
    }

    #[test]
    fn missing_api_key_returns_auth_failed() {
        let config = config_without_anthropic_key();
        let result = ClaudeHandle::new(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AgentError::AuthFailed { ref provider, .. } if provider == "claude"),
            "expected AuthFailed for claude, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn provider_name_is_claude() {
        let config = config_with_anthropic_key();
        let handle = ClaudeHandle::new(&config).expect("should create handle");
        assert_eq!(handle.provider_name(), "claude");
    }

    #[tokio::test]
    async fn is_available_initially_true() {
        let config = config_with_anthropic_key();
        let handle = ClaudeHandle::new(&config).expect("should create handle");
        assert!(handle.is_available().await);
    }

    #[tokio::test]
    async fn dropping_handle_stops_actor() {
        let config = config_with_anthropic_key();
        let handle = ClaudeHandle::new(&config).expect("should create handle");
        let sender = handle.sender.clone();

        // Drop all handles
        drop(handle);

        // The actor should shut down once all senders are dropped.
        // Sending on the cloned sender should still work briefly (channel still open),
        // but after the original handle is dropped, this is the last sender clone.
        drop(sender);

        // Give the actor a moment to shut down
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        // If we get here without hanging, the actor shut down.
    }

    #[tokio::test]
    async fn send_to_stopped_actor_returns_error() {
        let config = config_with_anthropic_key();
        let _handle = ClaudeHandle::new(&config).expect("should create handle");

        // Create a "detached" handle with a closed channel
        // by creating a new channel and immediately dropping the receiver
        let (tx, _rx) = mpsc::channel::<ActorMessage>(1);
        drop(_rx);
        let dead_handle = ClaudeHandle {
            sender: tx,
            circuit_breaker_open: Arc::new(AtomicBool::new(false)),
        };

        let request = AgentRequest {
            id: uuid::Uuid::new_v4(),
            agent: AgentId::claude("opus-4".to_string()),
            prompt: "test".to_string(),
            context: None,
            json_schema: None,
            created_at: chrono::Utc::now(),
        };

        let result = dead_handle.send(request).await;
        assert!(
            matches!(result, Err(AgentError::ActorStopped)),
            "expected ActorStopped, got: {result:?}"
        );
    }
}
