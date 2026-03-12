//! Mock backend for testing agent orchestration without real API calls.
//!
//! `MockBackend` implements `AgentBackend` and supports three modes:
//! - Sequenced: returns pre-configured responses in order
//! - AlwaysOk: always returns a successful response with given content
//! - AlwaysFail: always returns an error produced by a factory function

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ath_types::agent::{AgentKind, AgentRequest, AgentResponse};
use chrono::Utc;

use crate::backend::AgentBackend;
use crate::error::AgentError;

/// Internal mode for the mock backend.
enum MockMode {
    /// Returns responses from a queue, panics when exhausted.
    Sequenced(Mutex<VecDeque<Result<AgentResponse, AgentError>>>),
    /// Always returns a success response with the given content.
    AlwaysOk { content: String },
    /// Always returns an error produced by the factory function.
    AlwaysFail {
        make_error: Arc<dyn Fn() -> AgentError + Send + Sync>,
    },
}

/// A mock implementation of `AgentBackend` for testing.
///
/// Supports three construction modes:
/// - [`MockBackend::new`]: sequenced responses
/// - [`MockBackend::always_ok`]: infinite success
/// - [`MockBackend::failing`]: infinite failure
pub struct MockBackend {
    mode: MockMode,
    name: String,
}

impl MockBackend {
    /// Create a mock that returns responses in order.
    ///
    /// Panics with a descriptive message when all responses are exhausted.
    pub fn new(responses: Vec<Result<AgentResponse, AgentError>>) -> Self {
        Self {
            mode: MockMode::Sequenced(Mutex::new(VecDeque::from(responses))),
            name: "mock".into(),
        }
    }

    /// Create a mock that always succeeds with the given content.
    ///
    /// Each call generates a fresh `AgentResponse` with a new UUID,
    /// current timestamp, and zero token counts.
    pub fn always_ok(content: &str) -> Self {
        Self {
            mode: MockMode::AlwaysOk {
                content: content.to_owned(),
            },
            name: "mock".into(),
        }
    }

    /// Create a mock that always fails with errors from the given factory.
    ///
    /// Takes a function (not a value) since `AgentError` is not `Clone`.
    pub fn failing(make_error: impl Fn() -> AgentError + Send + Sync + 'static) -> Self {
        Self {
            mode: MockMode::AlwaysFail {
                make_error: Arc::new(make_error),
            },
            name: "mock".into(),
        }
    }

    /// Set a custom provider name for this mock.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }
}

#[async_trait]
impl AgentBackend for MockBackend {
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match &self.mode {
            MockMode::Sequenced(queue) => {
                let mut q = queue.lock().expect("MockBackend lock poisoned");
                q.pop_front().unwrap_or_else(|| {
                    panic!(
                        "MockBackend: no more responses queued (request id: {})",
                        request.id
                    )
                })
            }
            MockMode::AlwaysOk { content } => Ok(AgentResponse {
                request_id: request.id,
                agent: AgentKind::Claude("mock".into()),
                content: content.clone(),
                input_tokens: 0,
                output_tokens: 0,
                created_at: Utc::now(),
            }),
            MockMode::AlwaysFail { make_error } => Err(make_error()),
        }
    }

    async fn is_available(&self) -> bool {
        true
    }

    fn provider_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uuid::Uuid;

    fn make_request() -> AgentRequest {
        AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentKind::Claude("opus-4".into()),
            prompt: "test prompt".into(),
            context: None,
            created_at: Utc::now(),
        }
    }

    fn make_response(request_id: Uuid, content: &str) -> AgentResponse {
        AgentResponse {
            request_id,
            agent: AgentKind::Claude("opus-4".into()),
            content: content.into(),
            input_tokens: 100,
            output_tokens: 50,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn sequenced_returns_responses_in_order() {
        let req1 = make_request();
        let req2 = make_request();
        let resp1 = make_response(req1.id, "first");
        let resp2 = make_response(req2.id, "second");

        let mock = MockBackend::new(vec![Ok(resp1.clone()), Ok(resp2.clone())]);

        let r1 = mock.send(req1).await.unwrap();
        assert_eq!(r1.content, "first");

        let r2 = mock.send(req2).await.unwrap();
        assert_eq!(r2.content, "second");
    }

    #[tokio::test]
    async fn sequenced_returns_errors() {
        let mock = MockBackend::new(vec![Err(AgentError::ServerError {
            provider: "mock".into(),
            status: 500,
        })]);

        let result = mock.send(make_request()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[should_panic(expected = "no more responses queued")]
    async fn sequenced_panics_when_exhausted() {
        let mock = MockBackend::new(vec![]);
        mock.send(make_request()).await.ok();
    }

    #[tokio::test]
    async fn always_ok_returns_content() {
        let mock = MockBackend::always_ok("hello world");
        let req = make_request();
        let resp = mock.send(req.clone()).await.unwrap();
        assert_eq!(resp.content, "hello world");
        assert_eq!(resp.request_id, req.id);
    }

    #[tokio::test]
    async fn always_ok_returns_fresh_responses() {
        let mock = MockBackend::always_ok("content");
        let req1 = make_request();
        let req2 = make_request();

        let r1 = mock.send(req1.clone()).await.unwrap();
        let r2 = mock.send(req2.clone()).await.unwrap();

        // Each response has the correct request_id
        assert_eq!(r1.request_id, req1.id);
        assert_eq!(r2.request_id, req2.id);
        assert_ne!(req1.id, req2.id);
    }

    #[tokio::test]
    async fn failing_always_returns_error() {
        let mock = MockBackend::failing(|| AgentError::Timeout {
            provider: "mock".into(),
            duration: Duration::from_secs(300),
        });

        for _ in 0..3 {
            let result = mock.send(make_request()).await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn is_available_returns_true() {
        let mock = MockBackend::always_ok("ok");
        assert!(mock.is_available().await);
    }

    #[tokio::test]
    async fn provider_name_default() {
        let mock = MockBackend::always_ok("ok");
        assert_eq!(mock.provider_name(), "mock");
    }

    #[tokio::test]
    async fn provider_name_custom() {
        let mock = MockBackend::always_ok("ok").with_name("test-claude");
        assert_eq!(mock.provider_name(), "test-claude");
    }

    #[tokio::test]
    async fn mock_is_send_sync() {
        // Verify MockBackend can be used across async boundaries
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockBackend>();
    }

    #[tokio::test]
    async fn mock_as_dyn_backend() {
        // Verify MockBackend can be used as dyn AgentBackend
        let mock: Box<dyn AgentBackend> = Box::new(MockBackend::always_ok("dynamic"));
        let resp = mock.send(make_request()).await.unwrap();
        assert_eq!(resp.content, "dynamic");
    }
}
