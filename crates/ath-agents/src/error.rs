//! Normalized error types for the agent layer.
//!
//! `AgentError` classifies provider errors into retryable vs non-retryable
//! categories. All provider-specific error details are normalized into a
//! common set of variants that callers can handle uniformly.

use std::time::Duration;
use thiserror::Error;

/// Normalized error enum for agent operations.
///
/// All provider-specific errors are mapped into these variants.
/// Use [`is_retryable`](AgentError::is_retryable) to determine retry behavior.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Provider rate-limited the request (HTTP 429).
    #[error("Rate limited by {provider}")]
    RateLimit {
        provider: String,
        retry_after: Option<Duration>,
    },

    /// Authentication or authorization failed (HTTP 401/403).
    #[error("Authentication failed for {provider}: {reason}")]
    AuthFailed { provider: String, reason: String },

    /// Provider returned a server error (HTTP 5xx).
    #[error("Server error from {provider}: {status}")]
    ServerError { provider: String, status: u16 },

    /// Request timed out waiting for provider response.
    #[error("Request to {provider} timed out after {duration:?}")]
    Timeout { provider: String, duration: Duration },

    /// Provider returned an invalid or unparseable response.
    #[error("Invalid response from {provider}: {reason}")]
    InvalidResponse { provider: String, reason: String },

    /// Circuit breaker is open for the provider.
    #[error("Circuit breaker open for {provider}")]
    CircuitOpen { provider: String },

    /// The agent actor has stopped and cannot process requests.
    #[error("Agent actor has stopped")]
    ActorStopped,

    /// An unclassified error from the provider.
    #[error("Unknown error from {provider}: {message}")]
    Unknown { provider: String, message: String },
}

impl AgentError {
    /// Whether this error should trigger a retry.
    ///
    /// Retryable: RateLimit, ServerError, Timeout, InvalidResponse.
    /// Non-retryable: AuthFailed, CircuitOpen, ActorStopped, Unknown.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AgentError::RateLimit { .. }
                | AgentError::ServerError { .. }
                | AgentError::Timeout { .. }
                | AgentError::InvalidResponse { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_is_retryable() {
        let err = AgentError::RateLimit {
            provider: "Anthropic".into(),
            retry_after: Some(Duration::from_secs(5)),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn rate_limit_without_retry_after_is_retryable() {
        let err = AgentError::RateLimit {
            provider: "Google".into(),
            retry_after: None,
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn server_error_is_retryable() {
        let err = AgentError::ServerError {
            provider: "OpenAI".into(),
            status: 503,
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn timeout_is_retryable() {
        let err = AgentError::Timeout {
            provider: "Anthropic".into(),
            duration: Duration::from_secs(300),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn invalid_response_is_retryable() {
        let err = AgentError::InvalidResponse {
            provider: "Google".into(),
            reason: "not valid JSON".into(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn auth_failed_is_not_retryable() {
        let err = AgentError::AuthFailed {
            provider: "Anthropic".into(),
            reason: "invalid API key".into(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn circuit_open_is_not_retryable() {
        let err = AgentError::CircuitOpen {
            provider: "Google".into(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn actor_stopped_is_not_retryable() {
        let err = AgentError::ActorStopped;
        assert!(!err.is_retryable());
    }

    #[test]
    fn unknown_is_not_retryable() {
        let err = AgentError::Unknown {
            provider: "OpenAI".into(),
            message: "something unexpected".into(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn error_display_rate_limit() {
        let err = AgentError::RateLimit {
            provider: "Anthropic".into(),
            retry_after: None,
        };
        assert_eq!(format!("{err}"), "Rate limited by Anthropic");
    }

    #[test]
    fn error_display_auth_failed() {
        let err = AgentError::AuthFailed {
            provider: "Google".into(),
            reason: "bad key".into(),
        };
        assert_eq!(
            format!("{err}"),
            "Authentication failed for Google: bad key"
        );
    }

    #[test]
    fn auth_failed_hint_names_provider_specific_config_keys() {
        let err = AgentError::AuthFailed {
            provider: "Anthropic".into(),
            reason: "invalid API key".into(),
        };

        assert!(
            err.hint().contains("ANTHROPIC_API_KEY"),
            "auth hint should name the env var"
        );
        assert!(
            err.hint().contains("anthropic_api_key"),
            "auth hint should name the config key"
        );
    }

    #[test]
    fn error_display_actor_stopped() {
        let err = AgentError::ActorStopped;
        assert_eq!(format!("{err}"), "Agent actor has stopped");
    }

    #[test]
    fn all_variants_are_debug() {
        // Verify Debug is derived for all variants
        let errors: Vec<AgentError> = vec![
            AgentError::RateLimit {
                provider: "p".into(),
                retry_after: None,
            },
            AgentError::AuthFailed {
                provider: "p".into(),
                reason: "r".into(),
            },
            AgentError::ServerError {
                provider: "p".into(),
                status: 500,
            },
            AgentError::Timeout {
                provider: "p".into(),
                duration: Duration::from_secs(1),
            },
            AgentError::InvalidResponse {
                provider: "p".into(),
                reason: "r".into(),
            },
            AgentError::CircuitOpen {
                provider: "p".into(),
            },
            AgentError::ActorStopped,
            AgentError::Unknown {
                provider: "p".into(),
                message: "s".into(),
            },
        ];
        for err in &errors {
            let _ = format!("{:?}", err);
        }
        assert_eq!(errors.len(), 8, "should have exactly 8 variants");
    }
}
