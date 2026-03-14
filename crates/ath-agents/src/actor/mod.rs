//! Actor infrastructure for LLM provider backends.
//!
//! Each provider (Claude, Gemini, Codex) runs as a tokio actor with its own
//! bounded mpsc channel and circuit breaker. This module provides shared
//! infrastructure: message types, error classification, client construction,
//! and retry-with-circuit-breaker logic.

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod generic;

use std::sync::Arc;
use std::time::Duration;

use ath_config::ConfigStore;
use ath_types::agent::{AgentRequest, AgentResponse};
use backon::BackoffBuilder;
use chrono::Utc;
use genai::adapter::AdapterKind;
use genai::chat::ChatRequest;
use genai::resolver::{AuthData, AuthResolver};
use tokio::sync::oneshot;

use crate::circuit_breaker::CircuitBreaker;
use crate::error::AgentError;

pub use claude::ClaudeHandle;
pub use codex::CodexHandle;
pub use gemini::GeminiHandle;
pub use generic::{GenericHandle, GenericHandleConfig};

/// Callback type for streaming content chunks.
pub type ChunkCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// Message sent from a handle to its actor via the mpsc channel.
pub struct ActorMessage {
    /// The agent request to process.
    pub request: AgentRequest,
    /// Channel to send the result back to the caller.
    pub respond_to: oneshot::Sender<Result<AgentResponse, AgentError>>,
    /// Optional callback for streaming content chunks.
    pub on_chunk: Option<ChunkCallback>,
}

/// Classify a genai error into our normalized AgentError.
///
/// Maps HTTP status codes, auth errors, parse errors, and other genai
/// error variants into the appropriate AgentError variant.
pub fn classify_error(err: genai::Error, provider: &str) -> AgentError {
    match &err {
        // HTTP errors with status codes
        genai::Error::WebModelCall { webc_error, .. }
        | genai::Error::WebAdapterCall { webc_error, .. } => {
            classify_webc_error(webc_error, provider)
        }

        // Auth-related errors
        genai::Error::RequiresApiKey { .. }
        | genai::Error::NoAuthResolver { .. }
        | genai::Error::NoAuthData { .. } => AgentError::AuthFailed {
            provider: provider.to_string(),
            reason: err.to_string(),
        },

        genai::Error::Resolver { resolver_error, .. } => match resolver_error {
            genai::resolver::Error::ApiKeyEnvNotFound { .. } => AgentError::AuthFailed {
                provider: provider.to_string(),
                reason: err.to_string(),
            },
            _ => AgentError::AuthFailed {
                provider: provider.to_string(),
                reason: err.to_string(),
            },
        },

        // Response parsing errors
        genai::Error::NoChatResponse { .. }
        | genai::Error::ChatResponseGeneration { .. }
        | genai::Error::InvalidJsonResponseElement { .. }
        | genai::Error::StreamParse { .. }
        | genai::Error::SerdeJson(_) => AgentError::InvalidResponse {
            provider: provider.to_string(),
            reason: err.to_string(),
        },

        // Everything else
        _ => AgentError::Unknown {
            provider: provider.to_string(),
            message: err.to_string(),
        },
    }
}

/// Classify a webc (HTTP) error into our AgentError.
fn classify_webc_error(err: &genai::webc::Error, provider: &str) -> AgentError {
    match err {
        genai::webc::Error::ResponseFailedStatus { status, body, .. } => {
            let code = status.as_u16();
            eprintln!("[{provider}] HTTP {code}: {body}");
            match code {
                429 => AgentError::RateLimit {
                    provider: provider.to_string(),
                    // genai does not expose Retry-After headers directly;
                    // fall back to exponential backoff.
                    retry_after: None,
                },
                401 | 403 => AgentError::AuthFailed {
                    provider: provider.to_string(),
                    reason: format!("HTTP {code}"),
                },
                408 => AgentError::Timeout {
                    provider: provider.to_string(),
                    duration: Duration::from_secs(300),
                },
                500..=599 => AgentError::ServerError {
                    provider: provider.to_string(),
                    status: code,
                },
                _ => AgentError::Unknown {
                    provider: provider.to_string(),
                    message: if body.is_empty() {
                        format!("HTTP {code}")
                    } else {
                        // Truncate body to avoid spamming logs
                        let truncated = if body.len() > 500 { &body[..500] } else { body };
                        format!("HTTP {code}: {truncated}")
                    },
                },
            }
        }
        genai::webc::Error::Reqwest(reqwest_err) => {
            if reqwest_err.is_timeout() {
                AgentError::Timeout {
                    provider: provider.to_string(),
                    duration: Duration::from_secs(300),
                }
            } else if reqwest_err.is_connect() {
                AgentError::ServerError {
                    provider: provider.to_string(),
                    status: 503,
                }
            } else {
                AgentError::Unknown {
                    provider: provider.to_string(),
                    message: reqwest_err.to_string(),
                }
            }
        }
        _ => AgentError::InvalidResponse {
            provider: provider.to_string(),
            reason: err.to_string(),
        },
    }
}

/// Build a genai Client with API keys from ConfigStore injected via AuthResolver.
pub fn build_genai_client(config: &ConfigStore) -> Result<genai::Client, AgentError> {
    let anthropic_key = config.anthropic_api_key.clone();
    let google_key = config.google_api_key.clone();
    let openai_key = config.openai_api_key.clone();

    let auth_resolver = AuthResolver::from_resolver_fn(
        move |model_iden: genai::ModelIden| -> genai::resolver::Result<Option<AuthData>> {
            let key = match model_iden.adapter_kind {
                AdapterKind::Anthropic => anthropic_key.clone(),
                AdapterKind::Gemini => google_key.clone(),
                AdapterKind::OpenAI | AdapterKind::OpenAIResp => openai_key.clone(),
                // For any other adapter kind, try openai key as fallback
                _ => openai_key.clone(),
            };

            match key {
                Some(k) => Ok(Some(AuthData::from_single(k))),
                None => Err(genai::resolver::Error::Custom(format!(
                    "No API key configured for adapter {:?}",
                    model_iden.adapter_kind
                ))),
            }
        },
    );

    let client = genai::Client::builder()
        .with_auth_resolver(auth_resolver)
        .build();

    Ok(client)
}

/// Call a provider via genai streaming, returning content and token counts.
///
/// Builds a ChatRequest from prompt + optional context and sends it through
/// the genai client using `exec_chat_stream`. Content and usage are captured
/// via `ChatOptions` and extracted from the `StreamEnd` event.
///
/// When `json_schema` is provided, falls back to non-streaming `exec_chat`
/// since some providers don't support structured output with streaming.
///
/// An optional `on_chunk` callback receives each content chunk as it arrives,
/// enabling real-time output display.
pub async fn call_provider(
    client: &genai::Client,
    model: &str,
    prompt: &str,
    context: Option<&str>,
    json_schema: Option<&serde_json::Value>,
    provider: &str,
) -> Result<(String, u64, u64), AgentError> {
    call_provider_streaming(
        client,
        model,
        prompt,
        context,
        json_schema,
        provider,
        None,
        &[],
    )
    .await
}

/// Call a provider with optional streaming chunk callback and conversation history.
#[allow(clippy::too_many_arguments)]
pub async fn call_provider_streaming(
    client: &genai::Client,
    model: &str,
    prompt: &str,
    context: Option<&str>,
    json_schema: Option<&serde_json::Value>,
    provider: &str,
    on_chunk: Option<&(dyn Fn(&str) + Send + Sync)>,
    messages: &[ath_types::agent::ChatMessage],
) -> Result<(String, u64, u64), AgentError> {
    use ath_types::agent::ChatRole;
    use futures::StreamExt;
    use genai::chat::{ChatMessage, ChatOptions, ChatResponseFormat, ChatStreamEvent, JsonSpec};

    // Build ChatRequest from conversation history + current prompt
    let chat_req = if messages.is_empty() {
        let mut req = ChatRequest::from_user(prompt);
        if let Some(ctx) = context {
            req = req.with_system(ctx);
        }
        req
    } else {
        let mut msgs: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 2);

        // System message first (if provided)
        if let Some(ctx) = context {
            msgs.push(ChatMessage::system(ctx));
        }

        // Conversation history
        for msg in messages {
            match msg.role {
                ChatRole::User => msgs.push(ChatMessage::user(&msg.content)),
                ChatRole::Assistant => msgs.push(ChatMessage::assistant(&msg.content)),
                ChatRole::System => msgs.push(ChatMessage::system(&msg.content)),
            }
        }

        // Current prompt as final user message
        msgs.push(ChatMessage::user(prompt));

        ChatRequest::from_messages(msgs)
    };

    // JSON schema mode: fall back to non-streaming (some providers don't support both)
    if let Some(schema) = json_schema {
        let spec = JsonSpec::new("structured_output", schema.clone());
        let options =
            ChatOptions::default().with_response_format(ChatResponseFormat::JsonSpec(spec));

        let response = client
            .exec_chat(model, chat_req, Some(&options))
            .await
            .map_err(|e| classify_error(e, provider))?;

        let input_tokens = response.usage.prompt_tokens.unwrap_or(0) as u64;
        let output_tokens = response.usage.completion_tokens.unwrap_or(0) as u64;
        let content = response.into_first_text().unwrap_or_default();

        return Ok((content, input_tokens, output_tokens));
    }

    // Streaming mode: capture content and usage from stream
    let options = ChatOptions::default()
        .with_capture_content(true)
        .with_capture_usage(true);

    let chat_stream_res = client
        .exec_chat_stream(model, chat_req, Some(&options))
        .await
        .map_err(|e| classify_error(e, provider))?;

    let mut stream = chat_stream_res.stream;
    let mut stream_end = None;

    while let Some(event_result) = stream.next().await {
        let event = event_result.map_err(|e| classify_error(e, provider))?;
        match event {
            ChatStreamEvent::Chunk(chunk) => {
                if let Some(cb) = on_chunk {
                    cb(&chunk.content);
                }
            }
            ChatStreamEvent::End(end) => {
                stream_end = Some(end);
            }
            // Start, ReasoningChunk, ThoughtSignatureChunk, ToolCallChunk — ignore
            _ => {}
        }
    }

    let end = stream_end.ok_or_else(|| AgentError::InvalidResponse {
        provider: provider.to_string(),
        reason: "stream ended without End event".to_string(),
    })?;

    let input_tokens = end
        .captured_usage
        .as_ref()
        .and_then(|u| u.prompt_tokens)
        .unwrap_or(0) as u64;
    let output_tokens = end
        .captured_usage
        .as_ref()
        .and_then(|u| u.completion_tokens)
        .unwrap_or(0) as u64;

    let content = end.captured_into_first_text().unwrap_or_default();

    Ok((content, input_tokens, output_tokens))
}

/// Run a provider call with retry logic and circuit breaker integration.
///
/// - Checks circuit breaker before each attempt.
/// - Retries up to 3 times for retryable errors.
/// - Honors Retry-After duration from RateLimit errors when present.
/// - Falls back to exponential backoff (1s base, 2x, 60s cap, jitter) otherwise.
/// - Non-retryable errors abort immediately.
/// - Wraps the entire sequence in a 5-minute timeout.
pub async fn run_with_retry_and_breaker(
    client: &genai::Client,
    model: &str,
    request: &AgentRequest,
    circuit_breaker: &mut CircuitBreaker,
    provider: &str,
) -> Result<AgentResponse, AgentError> {
    run_with_retry_and_breaker_streaming(client, model, request, circuit_breaker, provider, None)
        .await
}

/// Run a provider call with retry logic, circuit breaker, and optional streaming callback.
pub async fn run_with_retry_and_breaker_streaming(
    client: &genai::Client,
    model: &str,
    request: &AgentRequest,
    circuit_breaker: &mut CircuitBreaker,
    provider: &str,
    on_chunk: Option<&ChunkCallback>,
) -> Result<AgentResponse, AgentError> {
    if !circuit_breaker.can_attempt() {
        return Err(AgentError::CircuitOpen {
            provider: provider.to_string(),
        });
    }

    let timeout_duration = Duration::from_secs(300); // 5 minutes

    let result = tokio::time::timeout(timeout_duration, async {
        // Build the exponential backoff iterator for fallback delays
        let backoff = backon::ExponentialBuilder::default().with_jitter().build();
        let mut backoff_iter = backoff.into_iter();

        let max_attempts = 3;
        let mut last_error: Option<AgentError> = None;

        for attempt in 0..max_attempts {
            let chunk_cb = on_chunk.map(|c| c.as_ref() as &(dyn Fn(&str) + Send + Sync));
            let call_result = call_provider_streaming(
                client,
                model,
                &request.prompt,
                request.context.as_deref(),
                request.json_schema.as_ref(),
                provider,
                chunk_cb,
                &request.messages,
            )
            .await;

            match call_result {
                Ok((content, input_tokens, output_tokens)) => {
                    circuit_breaker.record_success();
                    return Ok(AgentResponse {
                        request_id: request.id,
                        agent: request.agent.clone(),
                        content,
                        input_tokens,
                        output_tokens,
                        created_at: Utc::now(),
                    });
                }
                Err(e) => {
                    if !e.is_retryable() {
                        circuit_breaker.record_failure();
                        return Err(e);
                    }

                    // Determine delay: honor Retry-After from RateLimit if present
                    let delay = if let AgentError::RateLimit {
                        retry_after: Some(duration),
                        ..
                    } = &e
                    {
                        *duration
                    } else {
                        backoff_iter.next().unwrap_or(Duration::from_secs(60))
                    };

                    last_error = Some(e);

                    // Don't sleep after the last attempt
                    if attempt < max_attempts - 1 {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // All attempts exhausted
        circuit_breaker.record_failure();
        Err(last_error.unwrap_or_else(|| AgentError::Unknown {
            provider: provider.to_string(),
            message: "all retry attempts exhausted".to_string(),
        }))
    })
    .await;

    match result {
        Ok(inner_result) => inner_result,
        Err(_elapsed) => {
            circuit_breaker.record_failure();
            Err(AgentError::Timeout {
                provider: provider.to_string(),
                duration: timeout_duration,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_error_requires_api_key() {
        let model_iden = genai::ModelIden::new(AdapterKind::Anthropic, "claude-opus-4");
        let err = genai::Error::RequiresApiKey { model_iden };
        let agent_err = classify_error(err, "claude");
        assert!(
            matches!(agent_err, AgentError::AuthFailed { ref provider, .. } if provider == "claude"),
            "expected AuthFailed, got: {agent_err:?}"
        );
        assert!(!agent_err.is_retryable());
    }

    #[test]
    fn classify_error_no_auth_data() {
        let model_iden = genai::ModelIden::new(AdapterKind::Gemini, "gemini-2.5-pro");
        let err = genai::Error::NoAuthData { model_iden };
        let agent_err = classify_error(err, "gemini");
        assert!(
            matches!(agent_err, AgentError::AuthFailed { .. }),
            "expected AuthFailed, got: {agent_err:?}"
        );
    }

    #[test]
    fn classify_error_no_chat_response() {
        let model_iden = genai::ModelIden::new(AdapterKind::OpenAI, "o3");
        let err = genai::Error::NoChatResponse { model_iden };
        let agent_err = classify_error(err, "codex");
        assert!(
            matches!(agent_err, AgentError::InvalidResponse { ref provider, .. } if provider == "codex"),
            "expected InvalidResponse, got: {agent_err:?}"
        );
        assert!(agent_err.is_retryable());
    }

    #[test]
    fn classify_error_serde_json() {
        let serde_err: serde_json::Error = serde_json::from_str::<String>("not-json").unwrap_err();
        let err = genai::Error::SerdeJson(serde_err);
        let agent_err = classify_error(err, "test");
        assert!(
            matches!(agent_err, AgentError::InvalidResponse { .. }),
            "expected InvalidResponse, got: {agent_err:?}"
        );
    }

    #[test]
    fn classify_error_internal_is_unknown() {
        let err = genai::Error::Internal("something broke".to_string());
        let agent_err = classify_error(err, "test");
        assert!(
            matches!(agent_err, AgentError::Unknown { ref provider, .. } if provider == "test"),
            "expected Unknown, got: {agent_err:?}"
        );
    }

    #[test]
    fn classify_webc_429_is_rate_limit() {
        let webc_err = genai::webc::Error::ResponseFailedStatus {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            body: "rate limited".to_string(),
            headers: Box::new(reqwest::header::HeaderMap::new()),
        };
        let agent_err = classify_webc_error(&webc_err, "test");
        assert!(
            matches!(agent_err, AgentError::RateLimit { ref provider, .. } if provider == "test"),
            "expected RateLimit, got: {agent_err:?}"
        );
        assert!(agent_err.is_retryable());
    }

    #[test]
    fn classify_webc_401_is_auth_failed() {
        let webc_err = genai::webc::Error::ResponseFailedStatus {
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: "unauthorized".to_string(),
            headers: Box::new(reqwest::header::HeaderMap::new()),
        };
        let agent_err = classify_webc_error(&webc_err, "claude");
        assert!(
            matches!(agent_err, AgentError::AuthFailed { .. }),
            "expected AuthFailed, got: {agent_err:?}"
        );
        assert!(!agent_err.is_retryable());
    }

    #[test]
    fn classify_webc_500_is_server_error() {
        let webc_err = genai::webc::Error::ResponseFailedStatus {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            body: "server error".to_string(),
            headers: Box::new(reqwest::header::HeaderMap::new()),
        };
        let agent_err = classify_webc_error(&webc_err, "gemini");
        assert!(
            matches!(agent_err, AgentError::ServerError { ref provider, status: 500, .. } if provider == "gemini"),
            "expected ServerError 500, got: {agent_err:?}"
        );
        assert!(agent_err.is_retryable());
    }

    #[test]
    fn classify_webc_503_is_server_error() {
        let webc_err = genai::webc::Error::ResponseFailedStatus {
            status: reqwest::StatusCode::SERVICE_UNAVAILABLE,
            body: "service unavailable".to_string(),
            headers: Box::new(reqwest::header::HeaderMap::new()),
        };
        let agent_err = classify_webc_error(&webc_err, "codex");
        assert!(
            matches!(agent_err, AgentError::ServerError { status: 503, .. }),
            "expected ServerError 503, got: {agent_err:?}"
        );
    }
}
