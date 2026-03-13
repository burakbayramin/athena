//! Extraction LLM trait, response types, and configuration.
//!
//! The `ExtractionLlm` trait is defined locally in `ath-memory` (not imported
//! from `ath-agents`) to avoid coupling to provider infrastructure. The
//! orchestrator bridges `AgentBackend` to this trait in S04.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::MemoryError;

/// Minimal async trait for LLM calls during extraction.
///
/// Takes a prompt string and optional JSON schema hint, returns the raw
/// LLM response as a string. Implementors handle provider-specific details
/// (auth, retries, model selection).
#[async_trait]
pub trait ExtractionLlm: Send + Sync {
    /// Send a prompt to the LLM and get the response text.
    ///
    /// `json_schema` is an optional schema hint that some providers use
    /// to constrain output format (e.g., OpenAI's structured output mode).
    async fn complete(
        &self,
        prompt: &str,
        json_schema: Option<&serde_json::Value>,
    ) -> Result<String, MemoryError>;
}

/// Configuration for the extraction pipeline.
#[derive(Debug, Clone)]
pub struct ExtractionConfig {
    /// Maximum number of observations to include in a prompt.
    /// Excess observations are truncated (newest kept).
    pub max_observation_count: usize,

    /// Maximum byte budget for serialized observations in a prompt.
    /// Observations are dropped from the oldest end to stay within budget.
    pub max_serialization_bytes: usize,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            max_observation_count: 100,
            max_serialization_bytes: 50_000,
        }
    }
}

// ── Response types ──────────────────────────────────────────────────────────

/// LLM response for the run summary extraction stage.
#[derive(Debug, Clone, Deserialize)]
pub struct RunSummary {
    /// One-sentence abstract (max ~20 words).
    #[serde(default)]
    pub abstract_text: String,

    /// Structured overview of the run (phases, agents, decisions, issues).
    #[serde(default)]
    pub overview: String,

    /// Conventions detected during the run, if any.
    #[serde(default)]
    pub conventions_detected: Vec<String>,

    /// Decisions extracted during the run, if any.
    #[serde(default)]
    pub decisions: Vec<DecisionSnippet>,

    /// Issues encountered during the run, if any.
    #[serde(default)]
    pub issues: Vec<IssueSnippet>,
}

/// A decision snippet extracted as part of run summary.
#[derive(Debug, Clone, Deserialize)]
pub struct DecisionSnippet {
    /// The decision that was made.
    #[serde(default)]
    pub decision: String,
    /// Why it was made.
    #[serde(default)]
    pub rationale: String,
}

/// An issue snippet extracted as part of run summary.
#[derive(Debug, Clone, Deserialize)]
pub struct IssueSnippet {
    /// What the issue was.
    #[serde(default)]
    pub issue: String,
    /// How it was resolved, if at all.
    #[serde(default)]
    pub resolution: String,
}

/// Result of running the full extraction pipeline via `extract_all`.
///
/// Reports which stages succeeded and which failed, allowing callers
/// to inspect partial results.
#[derive(Debug)]
pub struct ExtractionResult {
    /// Stages that completed successfully (e.g., "run_summary", "conventions").
    pub succeeded: Vec<String>,
    /// Stages that failed, with their error details.
    pub failed: Vec<(String, MemoryError)>,
}

impl ExtractionResult {
    /// Create a new empty result.
    pub fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Returns true if all stages succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.failed.is_empty()
    }

    /// Returns the number of stages that succeeded.
    pub fn success_count(&self) -> usize {
        self.succeeded.len()
    }

    /// Returns the number of stages that failed.
    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }
}

impl Default for ExtractionResult {
    fn default() -> Self {
        Self::new()
    }
}

/// LLM response for the convention detection stage.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Convention {
    /// Short identifier for the convention (e.g., "error-handling-pattern").
    #[serde(default)]
    pub id: String,

    /// Human-readable description of the convention.
    #[serde(default)]
    pub description: String,

    /// How confident the LLM is (0.0–1.0).
    #[serde(default)]
    pub confidence: f32,

    /// Examples from the observations supporting this convention.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// LLM response for the decision extraction stage.
#[derive(Debug, Clone, Deserialize)]
pub struct Decision {
    /// What was decided.
    #[serde(default)]
    pub decision: String,

    /// Why it was decided.
    #[serde(default)]
    pub rationale: String,

    /// Where in the run this decision was made (phase, task, etc.).
    #[serde(default)]
    pub context: String,

    /// Impact assessment (e.g., "affects all API routes").
    #[serde(default)]
    pub impact: String,
}

/// LLM response for the agent profile update stage.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentProfileUpdate {
    /// The agent identifier (e.g., "claude/opus-4").
    #[serde(default)]
    pub agent_id: String,

    /// Tasks this agent handled in this run.
    #[serde(default)]
    pub tasks_handled: Vec<String>,

    /// Observed strengths.
    #[serde(default)]
    pub strengths: Vec<String>,

    /// Observed weaknesses or areas for improvement.
    #[serde(default)]
    pub weaknesses: Vec<String>,

    /// Review pass rate for this run (0.0–1.0), if applicable.
    #[serde(default)]
    pub review_pass_rate: Option<f32>,

    /// Common feedback themes from reviews.
    #[serde(default)]
    pub feedback_themes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_summary_deserialize_full() {
        let json = r#"{
            "abstract_text": "Built auth module with JWT.",
            "overview": "Phase 1 did planning, phase 2 implemented.",
            "conventions_detected": ["hexagonal architecture"],
            "decisions": [{"decision": "use JWT", "rationale": "stateless"}],
            "issues": [{"issue": "timeout", "resolution": "increased limit"}]
        }"#;
        let summary: RunSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.abstract_text, "Built auth module with JWT.");
        assert_eq!(summary.conventions_detected.len(), 1);
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.issues.len(), 1);
    }

    #[test]
    fn run_summary_deserialize_partial() {
        // Only abstract_text — everything else should default
        let json = r#"{"abstract_text": "Did stuff."}"#;
        let summary: RunSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.abstract_text, "Did stuff.");
        assert!(summary.overview.is_empty());
        assert!(summary.conventions_detected.is_empty());
        assert!(summary.decisions.is_empty());
        assert!(summary.issues.is_empty());
    }

    #[test]
    fn run_summary_deserialize_empty_object() {
        let json = "{}";
        let summary: RunSummary = serde_json::from_str(json).unwrap();
        assert!(summary.abstract_text.is_empty());
        assert!(summary.overview.is_empty());
    }

    #[test]
    fn convention_deserialize_with_defaults() {
        let json = r#"{"id": "hex-arch", "description": "Uses hexagonal architecture"}"#;
        let conv: Convention = serde_json::from_str(json).unwrap();
        assert_eq!(conv.id, "hex-arch");
        assert_eq!(conv.confidence, 0.0); // default
        assert!(conv.evidence.is_empty()); // default
    }

    #[test]
    fn decision_deserialize() {
        let json = r#"{
            "decision": "Use PostgreSQL",
            "rationale": "ACID compliance needed",
            "context": "Phase 1, task database-setup",
            "impact": "All data access patterns affected"
        }"#;
        let d: Decision = serde_json::from_str(json).unwrap();
        assert_eq!(d.decision, "Use PostgreSQL");
        assert!(!d.rationale.is_empty());
    }

    #[test]
    fn agent_profile_update_deserialize() {
        let json = r#"{
            "agent_id": "claude/opus-4",
            "tasks_handled": ["auth", "api"],
            "strengths": ["clean code"],
            "weaknesses": [],
            "review_pass_rate": 0.85,
            "feedback_themes": ["missing error handling"]
        }"#;
        let profile: AgentProfileUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(profile.agent_id, "claude/opus-4");
        assert_eq!(profile.review_pass_rate, Some(0.85));
        assert_eq!(profile.tasks_handled.len(), 2);
    }

    #[test]
    fn agent_profile_update_optional_pass_rate() {
        let json = r#"{"agent_id": "gemini/2.5-pro"}"#;
        let profile: AgentProfileUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(profile.review_pass_rate, None);
        assert!(profile.tasks_handled.is_empty());
    }
}
