//! Codex (OpenAI) actor implementation.
//!
//! `CodexActor` runs as a tokio task processing requests via a bounded mpsc channel.
//! `CodexHandle` is the cloneable handle that callers use to send requests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use ath_config::ConfigStore;
use ath_types::agent::{AgentRequest, AgentResponse};
use tokio::sync::{mpsc, oneshot};

use crate::backend::AgentBackend;
use crate::circuit_breaker::CircuitBreaker;
use crate::error::AgentError;

use super::{build_genai_client, run_with_retry_and_breaker_streaming, ActorMessage};

/// The actor task that owns the genai client and circuit breaker.
struct CodexActor {
    receiver: mpsc::Receiver<ActorMessage>,
    client: genai::Client,
    circuit_breaker: CircuitBreaker,
    model: String,
    circuit_breaker_open: Arc<AtomicBool>,
}

impl CodexActor {
    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            let result = run_with_retry_and_breaker_streaming(
                &self.client,
                &self.model,
                &msg.request,
                &mut self.circuit_breaker,
                "codex",
                msg.on_chunk.as_ref(),
            )
            .await;

            self.circuit_breaker_open
                .store(self.circuit_breaker.is_open(), Ordering::Relaxed);

            let _ = msg.respond_to.send(result);
        }
    }
}

/// Cloneable handle for sending requests to the Codex actor.
///
/// Implements `AgentBackend` for uniform provider access.
#[derive(Clone, Debug)]
pub struct CodexHandle {
    sender: mpsc::Sender<ActorMessage>,
    circuit_breaker_open: Arc<AtomicBool>,
}

impl CodexHandle {
    /// Create a new Codex actor and return a handle to it.
    ///
    /// Validates that the OpenAI API key is present (fail-fast).
    /// Spawns the actor task and returns the handle.
    pub fn new(config: &ConfigStore) -> Result<Self, AgentError> {
        if config.openai_api_key.is_none() {
            return Err(AgentError::AuthFailed {
                provider: "codex".to_string(),
                reason: "OpenAI API key not configured".to_string(),
            });
        }

        let client = build_genai_client(config)?;
        let model = config.codex_model.clone();
        let circuit_breaker_open = Arc::new(AtomicBool::new(false));

        let (sender, receiver) = mpsc::channel(32);

        let actor = CodexActor {
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
impl AgentBackend for CodexHandle {
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        self.send_streaming(request, None).await
    }

    async fn send_streaming(
        &self,
        request: AgentRequest,
        on_chunk: Option<crate::backend::ChunkCallback>,
    ) -> Result<AgentResponse, AgentError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ActorMessage {
                request,
                respond_to: tx,
                on_chunk,
            })
            .await
            .map_err(|_| AgentError::ActorStopped)?;

        rx.await.map_err(|_| AgentError::ActorStopped)?
    }

    async fn is_available(&self) -> bool {
        !self.circuit_breaker_open.load(Ordering::Relaxed)
    }

    fn provider_name(&self) -> &str {
        "codex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::AgentId;

    fn config_without_openai_key() -> ConfigStore {
        ConfigStore {
            anthropic_api_key: Some("test-anthropic-key".to_string()),
            google_api_key: Some("test-google-key".to_string()),
            openai_api_key: None,
            claude_model: "opus-4".to_string(),
            gemini_model: "2.5-pro".to_string(),
            codex_model: "o3".to_string(),
            agents: ath_config::AgentsConfig::default(),
            skills: None,
        }
    }

    fn config_with_openai_key() -> ConfigStore {
        ConfigStore {
            anthropic_api_key: None,
            google_api_key: None,
            openai_api_key: Some("test-openai-key".to_string()),
            claude_model: "opus-4".to_string(),
            gemini_model: "2.5-pro".to_string(),
            codex_model: "o3".to_string(),
            agents: ath_config::AgentsConfig::default(),
            skills: None,
        }
    }

    #[test]
    fn missing_api_key_returns_auth_failed() {
        let config = config_without_openai_key();
        let result = CodexHandle::new(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AgentError::AuthFailed { ref provider, .. } if provider == "codex"),
            "expected AuthFailed for codex, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn provider_name_is_codex() {
        let config = config_with_openai_key();
        let handle = CodexHandle::new(&config).expect("should create handle");
        assert_eq!(handle.provider_name(), "codex");
    }

    #[tokio::test]
    async fn is_available_initially_true() {
        let config = config_with_openai_key();
        let handle = CodexHandle::new(&config).expect("should create handle");
        assert!(handle.is_available().await);
    }

    #[tokio::test]
    async fn send_to_stopped_actor_returns_error() {
        let (tx, _rx) = mpsc::channel::<ActorMessage>(1);
        drop(_rx);
        let dead_handle = CodexHandle {
            sender: tx,
            circuit_breaker_open: Arc::new(AtomicBool::new(false)),
        };

        let request = AgentRequest {
            id: uuid::Uuid::new_v4(),
            agent: AgentId::codex("o3".to_string()),
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
