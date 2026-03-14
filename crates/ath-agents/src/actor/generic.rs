//! Generic OpenAI-compatible actor implementation.
//!
//! `GenericHandle` works with any OpenAI-compatible API endpoint — Ollama, Groq,
//! Together, Mistral, or any service that implements the OpenAI chat completions API.
//! It uses genai's `ServiceTargetResolver` to route to a custom base URL.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use ath_types::agent::{AgentRequest, AgentResponse};
use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{ModelIden, ServiceTarget};
use tokio::sync::{mpsc, oneshot};

use crate::backend::AgentBackend;
use crate::circuit_breaker::CircuitBreaker;
use crate::error::AgentError;

use super::{run_with_retry_and_breaker_streaming, ActorMessage};

/// Configuration for constructing a GenericHandle.
#[derive(Debug, Clone)]
pub struct GenericHandleConfig {
    /// Provider name for error messages and identification.
    pub provider: String,
    /// Model identifier passed to the API.
    pub model: String,
    /// API key (already resolved from env var).
    pub api_key: Option<String>,
    /// Custom base URL (e.g., "http://localhost:11434" for Ollama).
    pub base_url: Option<String>,
}

/// The actor task for a generic OpenAI-compatible provider.
struct GenericActor {
    receiver: mpsc::Receiver<ActorMessage>,
    client: genai::Client,
    circuit_breaker: CircuitBreaker,
    model: String,
    provider: String,
    circuit_breaker_open: Arc<AtomicBool>,
}

impl GenericActor {
    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            let result = run_with_retry_and_breaker_streaming(
                &self.client,
                &self.model,
                &msg.request,
                &mut self.circuit_breaker,
                &self.provider,
                msg.on_chunk.as_ref(),
            )
            .await;

            self.circuit_breaker_open
                .store(self.circuit_breaker.is_open(), Ordering::Relaxed);

            let _ = msg.respond_to.send(result);
        }
    }
}

/// Cloneable handle for sending requests to a generic OpenAI-compatible actor.
///
/// Implements `AgentBackend` for uniform provider access.
#[derive(Clone, Debug)]
pub struct GenericHandle {
    sender: mpsc::Sender<ActorMessage>,
    circuit_breaker_open: Arc<AtomicBool>,
    provider: String,
}

impl GenericHandle {
    /// Create a new generic actor and return a handle to it.
    ///
    /// Builds a genai client with custom auth and optional service target resolver
    /// for the given base URL.
    pub fn new(config: GenericHandleConfig) -> Result<Self, AgentError> {
        let client = build_generic_client(&config)?;
        let provider = config.provider.clone();
        let model = config.model.clone();
        let circuit_breaker_open = Arc::new(AtomicBool::new(false));

        let (sender, receiver) = mpsc::channel(32);

        let actor = GenericActor {
            receiver,
            client,
            circuit_breaker: CircuitBreaker::default(),
            model,
            provider: provider.clone(),
            circuit_breaker_open: circuit_breaker_open.clone(),
        };

        tokio::spawn(actor.run());

        Ok(Self {
            sender,
            circuit_breaker_open,
            provider,
        })
    }
}

#[async_trait]
impl AgentBackend for GenericHandle {
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
        &self.provider
    }
}

/// Build a genai Client configured for a generic OpenAI-compatible endpoint.
fn build_generic_client(config: &GenericHandleConfig) -> Result<genai::Client, AgentError> {
    let mut builder = genai::Client::builder();

    // Auth resolver: use the provided API key for all adapter kinds
    if let Some(ref api_key) = config.api_key {
        let key = api_key.clone();
        let auth_resolver = AuthResolver::from_resolver_fn(
            move |_model_iden: ModelIden| -> genai::resolver::Result<Option<AuthData>> {
                Ok(Some(AuthData::from_single(key.clone())))
            },
        );
        builder = builder.with_auth_resolver(auth_resolver);
    }

    // Service target resolver: override endpoint and adapter kind for custom base URLs
    if let Some(ref base_url) = config.base_url {
        let url = base_url.clone();
        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                let ServiceTarget { model, auth, .. } = service_target;
                let endpoint = Endpoint::from_owned(url.clone());
                // Use OpenAI adapter for any custom endpoint
                let model = ModelIden::new(AdapterKind::OpenAI, model.model_name);
                Ok(ServiceTarget {
                    endpoint,
                    auth,
                    model,
                })
            },
        );
        builder = builder.with_service_target_resolver(target_resolver);
    }

    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::AgentId;

    #[test]
    fn generic_handle_config_debug() {
        let config = GenericHandleConfig {
            provider: "ollama".into(),
            model: "llama3.3".into(),
            api_key: None,
            base_url: Some("http://localhost:11434".into()),
        };
        let debug = format!("{config:?}");
        assert!(debug.contains("ollama"));
        assert!(debug.contains("llama3.3"));
    }

    #[tokio::test]
    async fn generic_handle_creates_successfully() {
        let config = GenericHandleConfig {
            provider: "test-provider".into(),
            model: "test-model".into(),
            api_key: Some("test-key".into()),
            base_url: Some("http://localhost:8080".into()),
        };
        let handle = GenericHandle::new(config).expect("should create handle");
        assert_eq!(handle.provider_name(), "test-provider");
        assert!(handle.is_available().await);
    }

    #[tokio::test]
    async fn generic_handle_without_api_key() {
        // Ollama typically doesn't need an API key
        let config = GenericHandleConfig {
            provider: "ollama".into(),
            model: "llama3.3".into(),
            api_key: None,
            base_url: Some("http://localhost:11434".into()),
        };
        let handle = GenericHandle::new(config).expect("should create handle");
        assert_eq!(handle.provider_name(), "ollama");
    }

    #[tokio::test]
    async fn generic_handle_without_base_url() {
        // Provider like Groq that uses standard genai routing
        let config = GenericHandleConfig {
            provider: "groq".into(),
            model: "llama-3.1-8b-instant".into(),
            api_key: Some("test-key".into()),
            base_url: None,
        };
        let handle = GenericHandle::new(config).expect("should create handle");
        assert_eq!(handle.provider_name(), "groq");
    }

    #[tokio::test]
    async fn send_to_stopped_actor_returns_error() {
        let (tx, _rx) = mpsc::channel::<ActorMessage>(1);
        drop(_rx);
        let dead_handle = GenericHandle {
            sender: tx,
            circuit_breaker_open: Arc::new(AtomicBool::new(false)),
            provider: "dead".into(),
        };

        let request = AgentRequest {
            id: uuid::Uuid::new_v4(),
            agent: AgentId::new("dead", "test"),
            prompt: "test".into(),
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
