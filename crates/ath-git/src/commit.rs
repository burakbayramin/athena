//! Commit message builder with git trailers.
//!
//! Builds well-formed commit messages with the `athena:` prefix and structured
//! metadata trailers (Phase, Agent, Task-Id, Files-Count, optional Review-Status/Reviewer).

use ath_types::agent::AgentKind;
use ath_types::review::ReviewVerdict;

/// Metadata for building a commit message with trailers.
#[derive(Debug, Clone)]
pub struct CommitMetadata {
    /// The name of the phase being committed.
    pub phase_name: String,
    /// The provider name (e.g., "Anthropic").
    pub agent_provider: String,
    /// The model name (e.g., "opus-4").
    pub agent_model: String,
    /// Unique task identifier.
    pub task_id: String,
    /// Number of files included in this commit.
    pub files_count: usize,
    /// Optional review status ("passed" or "failed").
    pub review_status: Option<String>,
    /// Optional reviewer identity ("Provider/Model").
    pub reviewer: Option<String>,
}

impl CommitMetadata {
    /// Construct metadata from phase data, agent kind, and optional review verdict.
    pub fn from_phase_data(
        phase_name: impl Into<String>,
        agent: &AgentKind,
        task_id: impl Into<String>,
        files_count: usize,
        review: Option<&ReviewVerdict>,
    ) -> Self {
        let (review_status, reviewer) = match review {
            Some(v) => (
                Some(if v.passed {
                    "passed".to_string()
                } else {
                    "failed".to_string()
                }),
                Some(format!(
                    "{}/{}",
                    v.reviewer.provider_name(),
                    v.reviewer.model()
                )),
            ),
            None => (None, None),
        };

        Self {
            phase_name: phase_name.into(),
            agent_provider: agent.provider_name().to_string(),
            agent_model: agent.model().to_string(),
            task_id: task_id.into(),
            files_count,
            review_status,
            reviewer,
        }
    }
}

/// Build a commit message with trailers from the given metadata.
///
/// Format:
/// ```text
/// athena: {phase_name}
///
/// Phase: {phase_name}
/// Agent: {provider}/{model}
/// Task-Id: {task_id}
/// Files-Count: {files_count}
/// Review-Status: {status}     (optional)
/// Reviewer: {provider/model}  (optional)
/// ```
pub fn build_commit_message(meta: &CommitMetadata) -> String {
    let mut msg = format!("athena: {}\n\n", meta.phase_name);
    msg.push_str(&format!("Phase: {}\n", meta.phase_name));
    msg.push_str(&format!(
        "Agent: {}/{}\n",
        meta.agent_provider, meta.agent_model
    ));
    msg.push_str(&format!("Task-Id: {}\n", meta.task_id));
    msg.push_str(&format!("Files-Count: {}\n", meta.files_count));
    if let Some(ref status) = meta.review_status {
        msg.push_str(&format!("Review-Status: {}\n", status));
    }
    if let Some(ref reviewer) = meta.reviewer {
        msg.push_str(&format!("Reviewer: {}\n", reviewer));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> CommitMetadata {
        CommitMetadata {
            phase_name: "foundation".to_string(),
            agent_provider: "Anthropic".to_string(),
            agent_model: "opus-4".to_string(),
            task_id: "task-001".to_string(),
            files_count: 3,
            review_status: None,
            reviewer: None,
        }
    }

    #[test]
    fn build_commit_message_has_subject_and_trailers() {
        let meta = sample_meta();
        let msg = build_commit_message(&meta);

        // Subject line
        assert!(msg.starts_with("athena: foundation\n"));

        // Blank line separates subject from trailers
        assert!(msg.contains("\n\n"));

        // Required trailers
        assert!(msg.contains("Phase: foundation\n"));
        assert!(msg.contains("Agent: Anthropic/opus-4\n"));
        assert!(msg.contains("Task-Id: task-001\n"));
        assert!(msg.contains("Files-Count: 3\n"));
    }

    #[test]
    fn build_commit_message_includes_review_trailers_when_present() {
        let meta = CommitMetadata {
            review_status: Some("passed".to_string()),
            reviewer: Some("Google/2.5-pro".to_string()),
            ..sample_meta()
        };
        let msg = build_commit_message(&meta);

        assert!(msg.contains("Review-Status: passed\n"));
        assert!(msg.contains("Reviewer: Google/2.5-pro\n"));
    }

    #[test]
    fn build_commit_message_omits_review_trailers_when_none() {
        let meta = sample_meta();
        let msg = build_commit_message(&meta);

        assert!(!msg.contains("Review-Status:"));
        assert!(!msg.contains("Reviewer:"));
    }

    #[test]
    fn from_phase_data_extracts_agent_info() {
        let agent = AgentKind::Claude("opus-4".to_string());
        let meta = CommitMetadata::from_phase_data("test-phase", &agent, "task-42", 5, None);

        assert_eq!(meta.phase_name, "test-phase");
        assert_eq!(meta.agent_provider, "Anthropic");
        assert_eq!(meta.agent_model, "opus-4");
        assert_eq!(meta.task_id, "task-42");
        assert_eq!(meta.files_count, 5);
        assert!(meta.review_status.is_none());
        assert!(meta.reviewer.is_none());
    }

    #[test]
    fn from_phase_data_extracts_review_verdict() {
        use ath_types::review::{ReviewVerdict, Severity};

        let agent = AgentKind::Claude("opus-4".to_string());
        let verdict = ReviewVerdict {
            passed: true,
            reviewer: AgentKind::Gemini("2.5-pro".to_string()),
            severity: Severity::Info,
            reason: "All good".to_string(),
            suggestions: vec![],
        };
        let meta = CommitMetadata::from_phase_data("phase-1", &agent, "t-1", 2, Some(&verdict));

        assert_eq!(meta.review_status.as_deref(), Some("passed"));
        assert_eq!(meta.reviewer.as_deref(), Some("Google/2.5-pro"));
    }
}
