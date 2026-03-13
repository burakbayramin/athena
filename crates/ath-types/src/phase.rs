//! Phase record types for audit trails.
//!
//! `PhaseRecord` captures the full audit trail for a phase execution, including
//! token usage per agent, files produced, and all review attempts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::review::ReviewVerdict;

/// Token usage statistics for an agent interaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens produced.
    pub output_tokens: u64,
    /// Estimated cost in USD.
    pub estimated_cost_usd: f64,
}

/// Records a single agent's contribution to a phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentContribution {
    /// The agent that contributed.
    pub agent: AgentKind,
    /// Token usage for this contribution.
    pub tokens: TokenUsage,
    /// Files produced by this agent.
    pub files_produced: Vec<String>,
}

/// A single review attempt within a phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewAttempt {
    /// The attempt number (1-based).
    pub attempt_number: u32,
    /// The verdict from the review.
    pub verdict: ReviewVerdict,
    /// When this review attempt occurred.
    pub timestamp: DateTime<Utc>,
}

/// Full audit trail for a phase execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseRecord {
    /// Unique identifier for this phase execution.
    pub id: Uuid,
    /// Stable phase identifier from the routed execution plan.
    pub phase_id: u32,
    /// Name of the phase.
    pub phase_name: String,
    /// When the phase started.
    pub started_at: DateTime<Utc>,
    /// When the phase completed (None if still running).
    pub completed_at: Option<DateTime<Utc>>,
    /// All agent contributions to this phase.
    pub contributions: Vec<AgentContribution>,
    /// All review attempts for this phase.
    pub review_attempts: Vec<ReviewAttempt>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentKind;
    use crate::review::{CodeSuggestion, ReviewVerdict, Severity};

    #[test]
    fn phase_record_round_trip() {
        let record = PhaseRecord {
            id: Uuid::new_v4(),
            phase_id: 1,
            phase_name: "foundation".into(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            contributions: vec![
                AgentContribution {
                    agent: AgentKind::Claude("opus-4".into()),
                    tokens: TokenUsage {
                        input_tokens: 5000,
                        output_tokens: 2000,
                        estimated_cost_usd: 0.15,
                    },
                    files_produced: vec!["src/lib.rs".into(), "src/main.rs".into()],
                },
                AgentContribution {
                    agent: AgentKind::Gemini("2.5-pro".into()),
                    tokens: TokenUsage {
                        input_tokens: 3000,
                        output_tokens: 1000,
                        estimated_cost_usd: 0.05,
                    },
                    files_produced: vec!["tests/integration.rs".into()],
                },
            ],
            review_attempts: vec![
                ReviewAttempt {
                    attempt_number: 1,
                    verdict: ReviewVerdict {
                        passed: false,
                        reviewer: AgentKind::Codex("o3".into()),
                        severity: Severity::Critical,
                        reason: "Missing error handling".into(),
                        suggestions: vec![CodeSuggestion {
                            file: "src/lib.rs".into(),
                            line: Some(15),
                            suggestion: "Add Result return type".into(),
                        }],
                    },
                    tokens: TokenUsage {
                        input_tokens: 1200,
                        output_tokens: 400,
                        estimated_cost_usd: 0.02,
                    },
                    timestamp: Utc::now(),
                },
                ReviewAttempt {
                    attempt_number: 2,
                    verdict: ReviewVerdict {
                        passed: true,
                        reviewer: AgentKind::Codex("o3".into()),
                        severity: Severity::Info,
                        reason: "All issues addressed".into(),
                        suggestions: vec![],
                    },
                    tokens: TokenUsage {
                        input_tokens: 800,
                        output_tokens: 200,
                        estimated_cost_usd: 0.01,
                    },
                    timestamp: Utc::now(),
                },
            ],
        };

        let json = serde_json::to_string(&record).expect("serialize");
        let deserialized: PhaseRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(record, deserialized);
    }

    #[test]
    fn token_usage_round_trip() {
        let usage = TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            estimated_cost_usd: 0.42,
        };
        let json = serde_json::to_string(&usage).expect("serialize");
        let deserialized: TokenUsage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(usage, deserialized);
    }

    #[test]
    fn phase_record_with_no_completion() {
        let record = PhaseRecord {
            id: Uuid::new_v4(),
            phase_id: 2,
            phase_name: "analysis".into(),
            started_at: Utc::now(),
            completed_at: None,
            contributions: vec![],
            review_attempts: vec![],
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let deserialized: PhaseRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(record, deserialized);
        assert!(deserialized.completed_at.is_none());
    }
}
