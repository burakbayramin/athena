//! Observation event types for the memory subsystem.
//!
//! `ObservationType` captures structured execution events during a run.
//! Each variant is internally tagged (`"type":"VariantName"`) so JSONL lines
//! are human-readable and `grep`-able.
//!
//! `Observation` wraps an event with identity, run correlation, and a timestamp.

use ath_types::{AgentKind, Severity, TokenUsage};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A file operation kind observed during execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOpKind {
    /// A file was created.
    Create,
    /// A file was modified.
    Modify,
    /// A file was deleted.
    Delete,
    /// A file was read.
    Read,
}

/// Typed observation events emitted during orchestrator execution.
///
/// Internally tagged with `#[serde(tag = "type")]` so each JSONL line
/// contains `"type":"VariantName"` for grep-ability and human inspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ObservationType {
    /// An agent was invoked with a prompt.
    AgentRequest {
        /// The agent that received the request.
        agent: AgentKind,
        /// Summary of the prompt (not the full prompt — no secrets).
        prompt_summary: String,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
        /// The task name, if applicable.
        task_name: Option<String>,
    },

    /// An agent returned a response.
    AgentResponse {
        /// The agent that produced the response.
        agent: AgentKind,
        /// Token usage for this interaction.
        token_usage: TokenUsage,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
        /// The task name, if applicable.
        task_name: Option<String>,
    },

    /// A review verdict was issued.
    ReviewVerdict {
        /// The agent that performed the review.
        reviewer: AgentKind,
        /// Whether the review passed.
        passed: bool,
        /// Severity of the finding.
        severity: Severity,
        /// Summary of the reason (not the full review text).
        reason_summary: String,
        /// The attempt number (1-based).
        attempt_number: u32,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
    },

    /// A retry was started after a failed review.
    RetryStarted {
        /// The phase being retried.
        phase_id: Option<u32>,
        /// The attempt number (1-based).
        attempt_number: u32,
        /// Summary of the feedback driving the retry.
        feedback_summary: String,
    },

    /// A file operation was observed.
    FileOperation {
        /// The kind of file operation.
        op: FileOpKind,
        /// The file path (forward-slash normalized, never PathBuf).
        path: String,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
    },

    /// An error occurred during execution.
    Error {
        /// The error message.
        message: String,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
        /// Severity of the error.
        severity: Severity,
    },

    /// A routing decision was made by the orchestrator.
    RoutingDecision {
        /// The agent selected for the task.
        agent: AgentKind,
        /// The task being routed.
        task_name: String,
        /// Reason for the routing decision.
        reason: String,
        /// The phase this occurred in, if applicable.
        phase_id: Option<u32>,
    },
}

/// A timestamped, identified observation wrapping an event.
///
/// Each observation belongs to a run (identified by `run_id`) and carries
/// its own unique `id` for deduplication and correlation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    /// Unique identifier for this observation.
    pub id: Uuid,
    /// The run this observation belongs to.
    pub run_id: Uuid,
    /// When this observation was recorded.
    pub timestamp: DateTime<Utc>,
    /// The phase this observation relates to, if applicable.
    pub phase_id: Option<u32>,
    /// The event payload.
    pub event: ObservationType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::AgentKind;

    /// Helper: round-trip serialize → deserialize and assert equality.
    fn assert_round_trip(event: ObservationType) {
        let obs = Observation {
            id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            phase_id: Some(1),
            event,
        };
        let json = serde_json::to_string(&obs).expect("serialize");
        let back: Observation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(obs, back);
    }

    #[test]
    fn round_trip_agent_request() {
        assert_round_trip(ObservationType::AgentRequest {
            agent: AgentKind::Claude("opus-4".into()),
            prompt_summary: "Analyze authentication module".into(),
            phase_id: Some(1),
            task_name: Some("auth-analysis".into()),
        });
    }

    #[test]
    fn round_trip_agent_response() {
        assert_round_trip(ObservationType::AgentResponse {
            agent: AgentKind::Gemini("2.5-pro".into()),
            token_usage: TokenUsage {
                input_tokens: 5000,
                output_tokens: 2000,
                estimated_cost_usd: 0.15,
            },
            phase_id: Some(2),
            task_name: Some("code-gen".into()),
        });
    }

    #[test]
    fn round_trip_review_verdict() {
        assert_round_trip(ObservationType::ReviewVerdict {
            reviewer: AgentKind::Codex("o3".into()),
            passed: false,
            severity: Severity::Critical,
            reason_summary: "Missing error handling in auth flow".into(),
            attempt_number: 1,
            phase_id: Some(3),
        });
    }

    #[test]
    fn round_trip_retry_started() {
        assert_round_trip(ObservationType::RetryStarted {
            phase_id: Some(3),
            attempt_number: 2,
            feedback_summary: "Add Result return types to all public functions".into(),
        });
    }

    #[test]
    fn round_trip_file_operation() {
        assert_round_trip(ObservationType::FileOperation {
            op: FileOpKind::Create,
            path: "src/auth/mod.rs".into(),
            phase_id: Some(1),
        });
    }

    #[test]
    fn round_trip_error() {
        assert_round_trip(ObservationType::Error {
            message: "Agent timeout after 30s".into(),
            phase_id: Some(4),
            severity: Severity::Warning,
        });
    }

    #[test]
    fn round_trip_routing_decision() {
        assert_round_trip(ObservationType::RoutingDecision {
            agent: AgentKind::Claude("opus-4".into()),
            task_name: "security-audit".into(),
            reason: "Claude best suited for security analysis".into(),
            phase_id: Some(1),
        });
    }

    #[test]
    fn observation_without_phase_id() {
        let obs = Observation {
            id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            phase_id: None,
            event: ObservationType::Error {
                message: "Startup error".into(),
                phase_id: None,
                severity: Severity::Critical,
            },
        };
        let json = serde_json::to_string(&obs).expect("serialize");
        let back: Observation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(obs, back);
    }

    #[test]
    fn internally_tagged_type_field_present() {
        let event = ObservationType::AgentRequest {
            agent: AgentKind::Claude("opus-4".into()),
            prompt_summary: "test".into(),
            phase_id: None,
            task_name: None,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains(r#""type":"AgentRequest""#),
            "JSON should contain internally tagged type field: {json}"
        );
    }

    #[test]
    fn file_op_kind_round_trip() {
        for kind in [
            FileOpKind::Create,
            FileOpKind::Modify,
            FileOpKind::Delete,
            FileOpKind::Read,
        ] {
            let json = serde_json::to_string(&kind).expect("serialize");
            let back: FileOpKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(kind, back);
        }
    }
}
