//! Review verdict types with severity levels and code suggestions.
//!
//! `ReviewVerdict` captures the result of a code review, including whether it passed,
//! the severity level, and any suggestions for improvement.

use serde::{Deserialize, Serialize};

use crate::agent::AgentId;
use crate::error::ValidationError;

/// Severity level of a review finding. Only `Critical` blocks progress.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocks progress — must be fixed before continuing.
    Critical,
    /// Should be addressed but does not block.
    Warning,
    /// Informational only.
    Info,
}

/// A code suggestion attached to a review verdict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeSuggestion {
    /// The file this suggestion applies to.
    pub file: String,
    /// The line number (if applicable).
    pub line: Option<u32>,
    /// The suggested change.
    pub suggestion: String,
}

/// The result of a code review by an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewVerdict {
    /// Whether the review passed.
    pub passed: bool,
    /// The agent that performed the review.
    pub reviewer: AgentId,
    /// The severity of the finding.
    pub severity: Severity,
    /// The reason for the verdict.
    pub reason: String,
    /// Code suggestions for improvement.
    pub suggestions: Vec<CodeSuggestion>,
}

impl ReviewVerdict {
    /// Returns true if this verdict blocks progress (critical failure).
    pub fn blocks_progress(&self) -> bool {
        !self.passed && self.severity == Severity::Critical
    }

    /// Validates the review verdict.
    /// Critical failures must have a non-empty reason.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.severity == Severity::Critical && !self.passed && self.reason.trim().is_empty() {
            return Err(ValidationError::EmptyField {
                field: "reason".into(),
                hint: "Critical review failures must include a reason".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn critical_failing_verdict() -> ReviewVerdict {
        ReviewVerdict {
            passed: false,
            reviewer: AgentId::claude("opus-4"),
            severity: Severity::Critical,
            reason: "Security vulnerability in auth module".into(),
            suggestions: vec![CodeSuggestion {
                file: "src/auth.rs".into(),
                line: Some(42),
                suggestion: "Use bcrypt instead of md5 for password hashing".into(),
            }],
        }
    }

    #[test]
    fn critical_failure_blocks_progress() {
        let verdict = critical_failing_verdict();
        assert!(verdict.blocks_progress());
    }

    #[test]
    fn warning_failure_does_not_block_progress() {
        let verdict = ReviewVerdict {
            passed: false,
            reviewer: AgentId::gemini("2.5-pro"),
            severity: Severity::Warning,
            reason: "Consider using more descriptive variable names".into(),
            suggestions: vec![],
        };
        assert!(!verdict.blocks_progress());
    }

    #[test]
    fn passing_critical_does_not_block() {
        let verdict = ReviewVerdict {
            passed: true,
            reviewer: AgentId::claude("opus-4"),
            severity: Severity::Critical,
            reason: "All checks passed".into(),
            suggestions: vec![],
        };
        assert!(!verdict.blocks_progress());
    }

    #[test]
    fn review_verdict_round_trip() {
        let verdict = critical_failing_verdict();
        let json = serde_json::to_string(&verdict).expect("serialize");
        let deserialized: ReviewVerdict = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(verdict, deserialized);
    }

    #[test]
    fn review_verdict_preserves_suggestions_list() {
        let verdict = ReviewVerdict {
            passed: false,
            reviewer: AgentId::codex("o3"),
            severity: Severity::Warning,
            reason: "Multiple style issues".into(),
            suggestions: vec![
                CodeSuggestion {
                    file: "src/main.rs".into(),
                    line: Some(10),
                    suggestion: "Use snake_case".into(),
                },
                CodeSuggestion {
                    file: "src/lib.rs".into(),
                    line: None,
                    suggestion: "Add module documentation".into(),
                },
            ],
        };
        let json = serde_json::to_string(&verdict).expect("serialize");
        let deserialized: ReviewVerdict = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(verdict.suggestions.len(), deserialized.suggestions.len());
        assert_eq!(verdict, deserialized);
    }

    #[test]
    fn severity_serializes_lowercase() {
        let json = serde_json::to_string(&Severity::Critical).expect("serialize");
        assert_eq!(json, "\"critical\"");
        let json = serde_json::to_string(&Severity::Warning).expect("serialize");
        assert_eq!(json, "\"warning\"");
        let json = serde_json::to_string(&Severity::Info).expect("serialize");
        assert_eq!(json, "\"info\"");
    }
}
