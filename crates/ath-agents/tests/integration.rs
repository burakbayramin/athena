//! Integration tests for the ath-agents crate.
//!
//! These tests exercise the public API from an external crate perspective,
//! proving that AgentBackend trait dispatch, MockBackend modes, dynamic dispatch,
//! circuit breaker behavior, and provider handle construction all work correctly.

use std::time::Duration;

use ath_agents::{
    AgentBackend, AgentError, CircuitBreaker, ClaudeHandle, CodexHandle, GeminiHandle, MockBackend,
};
use ath_config::ConfigStore;
use ath_types::agent::{AgentKind, AgentRequest, AgentResponse};
use chrono::Utc;
use uuid::Uuid;

/// Helper: create a valid AgentRequest for a given agent kind.
fn make_request(agent: AgentKind) -> AgentRequest {
    AgentRequest {
        id: Uuid::new_v4(),
        agent,
        prompt: "What is 2 + 2?".into(),
        context: None,
        created_at: Utc::now(),
    }
}

/// Helper: create a successful AgentResponse for a given request ID.
fn make_response(request_id: Uuid, content: &str) -> AgentResponse {
    AgentResponse {
        request_id,
        agent: AgentKind::Claude("mock".into()),
        content: content.into(),
        input_tokens: 10,
        output_tokens: 5,
        created_at: Utc::now(),
    }
}

/// Helper: create a ConfigStore with no API keys configured.
fn config_no_keys() -> ConfigStore {
    ConfigStore {
        anthropic_api_key: None,
        google_api_key: None,
        openai_api_key: None,
        claude_model: "opus-4".into(),
        gemini_model: "2.5-pro".into(),
        codex_model: "o3".into(),
    }
}

// ============================================================
// Group 1: MockBackend through AgentBackend trait (always_ok)
// ============================================================

#[tokio::test]
async fn mock_always_ok_returns_correct_content() {
    let backend = MockBackend::always_ok("hello world");
    let request = make_request(AgentKind::Claude("opus-4".into()));
    let response = backend.send(request).await.unwrap();
    assert_eq!(response.content, "hello world");
}

#[tokio::test]
async fn mock_always_ok_is_available() {
    let backend = MockBackend::always_ok("ok");
    assert!(backend.is_available().await);
}

#[tokio::test]
async fn mock_always_ok_provider_name() {
    let backend = MockBackend::always_ok("ok");
    assert_eq!(backend.provider_name(), "mock");
}

#[tokio::test]
async fn mock_always_ok_request_id_matches() {
    let backend = MockBackend::always_ok("test");
    let request = make_request(AgentKind::Gemini("2.5-pro".into()));
    let req_id = request.id;
    let response = backend.send(request).await.unwrap();
    assert_eq!(response.request_id, req_id);
}

// ============================================================
// Group 2: MockBackend sequenced responses
// ============================================================

#[tokio::test]
async fn mock_sequenced_returns_in_order() {
    let req1 = make_request(AgentKind::Claude("opus-4".into()));
    let req2 = make_request(AgentKind::Claude("opus-4".into()));
    let req3 = make_request(AgentKind::Claude("opus-4".into()));

    let resp1 = make_response(req1.id, "first");
    let resp2_err = AgentError::ServerError {
        provider: "mock".into(),
        status: 500,
    };
    let resp3 = make_response(req3.id, "third");

    let backend = MockBackend::new(vec![Ok(resp1), Err(resp2_err), Ok(resp3)]);

    // First call: success
    let r1 = backend.send(req1).await.unwrap();
    assert_eq!(r1.content, "first");

    // Second call: error
    let r2 = backend.send(req2).await;
    assert!(r2.is_err());

    // Third call: success
    let r3 = backend.send(req3).await.unwrap();
    assert_eq!(r3.content, "third");
}

// ============================================================
// Group 3: MockBackend failing mode
// ============================================================

#[tokio::test]
async fn mock_failing_returns_expected_error() {
    let backend = MockBackend::failing(|| AgentError::Timeout {
        provider: "mock".into(),
        duration: Duration::from_secs(300),
    });

    let result = backend
        .send(make_request(AgentKind::Claude("opus-4".into())))
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AgentError::Timeout { ref provider, .. } if provider == "mock"),
        "expected Timeout error, got: {err:?}"
    );
}

// ============================================================
// Group 4: Dynamic dispatch via Box<dyn AgentBackend>
// ============================================================

#[tokio::test]
async fn dyn_dispatch_works_through_trait_object() {
    let backend: Box<dyn AgentBackend> = Box::new(MockBackend::always_ok("dynamic"));
    let request = make_request(AgentKind::Claude("opus-4".into()));
    let response = backend.send(request).await.unwrap();
    assert_eq!(response.content, "dynamic");
}

#[tokio::test]
async fn dyn_dispatch_is_available() {
    let backend: Box<dyn AgentBackend> = Box::new(MockBackend::always_ok("test"));
    assert!(backend.is_available().await);
}

#[tokio::test]
async fn dyn_dispatch_provider_name() {
    let backend: Box<dyn AgentBackend> =
        Box::new(MockBackend::always_ok("test").with_name("custom-provider"));
    assert_eq!(backend.provider_name(), "custom-provider");
}

#[tokio::test]
async fn multiple_dyn_backends_in_vec() {
    let backends: Vec<Box<dyn AgentBackend>> = vec![
        Box::new(MockBackend::always_ok("claude-response").with_name("claude")),
        Box::new(MockBackend::always_ok("gemini-response").with_name("gemini")),
        Box::new(MockBackend::always_ok("codex-response").with_name("codex")),
    ];

    for backend in &backends {
        let request = make_request(AgentKind::Claude("test".into()));
        let response = backend.send(request).await.unwrap();
        assert!(!response.content.is_empty());
    }

    assert_eq!(backends[0].provider_name(), "claude");
    assert_eq!(backends[1].provider_name(), "gemini");
    assert_eq!(backends[2].provider_name(), "codex");
}

// ============================================================
// Group 5: Provider handle construction with missing keys
// ============================================================

#[test]
fn claude_handle_rejects_missing_api_key() {
    let config = config_no_keys();
    let result = ClaudeHandle::new(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AgentError::AuthFailed { ref provider, .. } if provider == "claude"),
        "expected AuthFailed for claude, got: {err:?}"
    );
}

#[test]
fn gemini_handle_rejects_missing_api_key() {
    let config = config_no_keys();
    let result = GeminiHandle::new(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AgentError::AuthFailed { ref provider, .. } if provider == "gemini"),
        "expected AuthFailed for gemini, got: {err:?}"
    );
}

#[test]
fn codex_handle_rejects_missing_api_key() {
    let config = config_no_keys();
    let result = CodexHandle::new(&config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AgentError::AuthFailed { ref provider, .. } if provider == "codex"),
        "expected AuthFailed for codex, got: {err:?}"
    );
}

// ============================================================
// Group 6: Circuit breaker integration
// ============================================================

#[tokio::test(start_paused = true)]
async fn circuit_breaker_trips_after_three_failures() {
    let mut cb = CircuitBreaker::default();

    // Three consecutive failures should trip the breaker
    cb.record_failure();
    assert!(cb.can_attempt(), "should allow after 1 failure");
    cb.record_failure();
    assert!(cb.can_attempt(), "should allow after 2 failures");
    cb.record_failure();

    // Now the circuit should be open
    assert!(cb.is_open(), "should be open after 3 failures");
    assert!(
        !cb.can_attempt(),
        "should not allow attempts when circuit is open"
    );
}

#[tokio::test(start_paused = true)]
async fn circuit_breaker_recovers_after_cooldown() {
    let mut cb = CircuitBreaker::new(3, Duration::from_secs(30));

    // Trip the breaker
    cb.record_failure();
    cb.record_failure();
    cb.record_failure();
    assert!(cb.is_open());

    // Advance past cooldown
    tokio::time::advance(Duration::from_secs(31)).await;

    // Should allow one probe (half-open)
    assert!(cb.can_attempt(), "should allow probe after cooldown");

    // Probe succeeds -> circuit closes
    cb.record_success();
    assert!(!cb.is_open(), "should close after successful probe");
    assert!(cb.can_attempt(), "should allow normal attempts after closing");
}

// Note: Circuit breaker integration with actual provider actors is verified
// via the retry loop unit tests in actor/mod.rs. Testing full retry+circuit
// breaker behavior end-to-end would require real HTTP calls or a more complex
// mock setup. The CircuitBreaker state machine is thoroughly tested in
// circuit_breaker.rs unit tests.
