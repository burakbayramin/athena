//! Prompt construction for extraction stages.
//!
//! Handles observation preprocessing (truncation, serialization budget) and
//! prompt template formatting for each extraction stage.

use crate::extract::types::ExtractionConfig;
use crate::observe::types::Observation;

/// Preprocess observations for inclusion in a prompt.
///
/// 1. Truncates to `config.max_observation_count` (keeps the *last* N — most recent).
/// 2. Serializes each observation to JSON.
/// 3. Drops oldest observations until total byte size fits within
///    `config.max_serialization_bytes`.
///
/// Returns the serialized observations as a single JSONL string.
#[tracing::instrument(skip(observations, config), fields(
    input_count = observations.len(),
    max_count = config.max_observation_count,
    max_bytes = config.max_serialization_bytes
))]
pub fn preprocess_observations(
    observations: &[Observation],
    config: &ExtractionConfig,
) -> String {
    // Take the last N observations (most recent).
    let start = observations.len().saturating_sub(config.max_observation_count);
    let truncated = &observations[start..];

    // Serialize each observation to a JSON line.
    let mut lines: Vec<String> = Vec::with_capacity(truncated.len());
    for obs in truncated {
        match serde_json::to_string(obs) {
            Ok(json) => lines.push(json),
            Err(e) => {
                tracing::warn!(
                    observation_id = %obs.id,
                    error = %e,
                    "Failed to serialize observation, skipping"
                );
            }
        }
    }

    // Drop from the front (oldest) until within byte budget.
    let mut total_bytes: usize = lines.iter().map(|l| l.len() + 1).sum(); // +1 for newline
    while total_bytes > config.max_serialization_bytes && !lines.is_empty() {
        let removed = lines.remove(0);
        total_bytes -= removed.len() + 1;
    }

    tracing::debug!(
        output_count = lines.len(),
        total_bytes,
        "Preprocessed observations for prompt"
    );

    lines.join("\n")
}

/// Build the prompt for the convention detection stage.
///
/// Asks the LLM to identify recurring project conventions from observations.
pub fn build_conventions_prompt(observations_jsonl: &str) -> String {
    format!(
        r#"You are analyzing an Athena run. Given the following observations, identify recurring project conventions and patterns. For each convention, produce a JSON array element with:

1. "id" (kebab-case slug): Short identifier (e.g., "error-handling-pattern", "hexagonal-architecture").
2. "description" (1-3 sentences): What the convention is and how it manifests.
3. "confidence" (0.0–1.0): How confident you are this is an established convention, not a one-off.
4. "evidence" (array of strings): Specific observations that support this convention.

Observations:
{observations_jsonl}

Respond with a JSON array of convention objects. No markdown fencing, no explanation — just the JSON array."#
    )
}

/// Build the prompt for the decision extraction stage.
///
/// Asks the LLM to extract architectural and design decisions from observations.
pub fn build_decisions_prompt(observations_jsonl: &str) -> String {
    format!(
        r#"You are analyzing an Athena run. Given the following observations, extract significant architectural and design decisions that were made. For each decision, produce a JSON array element with:

1. "decision" (1-2 sentences): What was decided.
2. "rationale" (1-3 sentences): Why it was decided.
3. "context" (string): Where in the run this decision was made (phase, task, agent).
4. "impact" (string): What parts of the system this decision affects.

Observations:
{observations_jsonl}

Respond with a JSON array of decision objects. No markdown fencing, no explanation — just the JSON array."#
    )
}

/// Build the prompt for the agent profile update stage.
///
/// Asks the LLM to assess agent performance from observations.
pub fn build_agent_profiles_prompt(observations_jsonl: &str) -> String {
    format!(
        r#"You are analyzing an Athena run. Given the following observations, assess each agent's performance. For each agent that participated, produce a JSON array element with:

1. "agent_id" (string): The agent identifier (e.g., "claude/opus-4", "gemini/2.5-pro").
2. "tasks_handled" (array of strings): What tasks this agent worked on.
3. "strengths" (array of strings): Observed strengths in this run.
4. "weaknesses" (array of strings): Observed weaknesses or areas for improvement.
5. "review_pass_rate" (float 0.0–1.0 or null): If the agent was reviewed, what fraction passed.
6. "feedback_themes" (array of strings): Common themes from review feedback.

Observations:
{observations_jsonl}

Respond with a JSON array of agent profile objects. No markdown fencing, no explanation — just the JSON array."#
    )
}

/// Build the prompt for the run summary extraction stage.
///
/// Follows the prompt format from memory-layer.md §4.4.
pub fn build_run_summary_prompt(observations_jsonl: &str) -> String {
    format!(
        r#"You are analyzing an Athena run. Given the following observations from a project build run, produce a JSON response with these fields:

1. "abstract_text" (1 sentence, max 20 words): What was accomplished?
2. "overview" (max 300 words):
   - What phases were executed?
   - Which agents handled what?
   - What key decisions were made?
   - Were there any issues or retries?
   - What files were created/modified?
3. "conventions_detected" (array of strings): Any project conventions or patterns observed.
4. "decisions" (array of objects with "decision" and "rationale" fields): Key decisions made.
5. "issues" (array of objects with "issue" and "resolution" fields): Problems encountered and how they were resolved.

Observations:
{observations_jsonl}

Respond with a single JSON object. No markdown fencing, no explanation — just the JSON."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::types::{ObservationType, Observation};
    use ath_types::AgentId;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_observation(run_id: Uuid, summary: &str) -> Observation {
        Observation {
            id: Uuid::new_v4(),
            run_id,
            timestamp: Utc::now(),
            phase_id: Some(1),
            event: ObservationType::AgentRequest {
                agent: AgentId::claude("opus-4"),
                prompt_summary: summary.to_string(),
                phase_id: Some(1),
                task_name: Some("test".into()),
            },
        }
    }

    #[test]
    fn preprocess_truncates_at_max_count() {
        let run_id = Uuid::new_v4();
        let observations: Vec<Observation> = (0..10)
            .map(|i| make_observation(run_id, &format!("obs_{i}")))
            .collect();

        let config = ExtractionConfig {
            max_observation_count: 5,
            max_serialization_bytes: 100_000,
        };

        let result = preprocess_observations(&observations, &config);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 5);

        // Should contain the last 5 observations (obs_5 through obs_9)
        assert!(result.contains("obs_5"));
        assert!(result.contains("obs_9"));
        assert!(!result.contains("obs_4"));
    }

    #[test]
    fn preprocess_respects_byte_budget() {
        let run_id = Uuid::new_v4();
        let observations: Vec<Observation> = (0..10)
            .map(|i| make_observation(run_id, &format!("obs_{i}")))
            .collect();

        // Each serialized observation is ~250+ bytes. Set budget to allow ~2.
        let config = ExtractionConfig {
            max_observation_count: 100,
            max_serialization_bytes: 600,
        };

        let result = preprocess_observations(&observations, &config);
        let lines: Vec<&str> = result.lines().collect();
        // Should have dropped oldest to fit budget
        assert!(lines.len() < 10);
        let total_bytes: usize = result.len();
        assert!(total_bytes <= 600, "total_bytes={total_bytes} should be <= 600");
    }

    #[test]
    fn preprocess_empty_observations() {
        let config = ExtractionConfig::default();
        let result = preprocess_observations(&[], &config);
        assert!(result.is_empty());
    }

    #[test]
    fn preprocess_default_max_count_is_100() {
        let config = ExtractionConfig::default();
        assert_eq!(config.max_observation_count, 100);
    }

    #[test]
    fn build_run_summary_prompt_includes_observations() {
        let observations = r#"{"type":"AgentRequest","agent":"claude"}"#;
        let prompt = build_run_summary_prompt(observations);
        assert!(prompt.contains(observations));
        assert!(prompt.contains("abstract_text"));
        assert!(prompt.contains("overview"));
        assert!(prompt.contains("conventions_detected"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn build_conventions_prompt_includes_observations() {
        let observations = r#"{"type":"FileOperation","op":"create","path":"src/auth.rs"}"#;
        let prompt = build_conventions_prompt(observations);
        assert!(prompt.contains(observations));
        assert!(prompt.contains("convention"));
        assert!(prompt.contains("confidence"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn build_decisions_prompt_includes_observations() {
        let observations = r#"{"type":"RoutingDecision","agent":"claude"}"#;
        let prompt = build_decisions_prompt(observations);
        assert!(prompt.contains(observations));
        assert!(prompt.contains("decision"));
        assert!(prompt.contains("rationale"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn build_agent_profiles_prompt_includes_observations() {
        let observations = r#"{"type":"AgentResponse","agent":"gemini"}"#;
        let prompt = build_agent_profiles_prompt(observations);
        assert!(prompt.contains(observations));
        assert!(prompt.contains("agent_id"));
        assert!(prompt.contains("strengths"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn preprocess_keeps_newest_when_truncated() {
        let run_id = Uuid::new_v4();
        let observations: Vec<Observation> = (0..20)
            .map(|i| make_observation(run_id, &format!("item_{i}")))
            .collect();

        let config = ExtractionConfig {
            max_observation_count: 3,
            max_serialization_bytes: 100_000,
        };

        let result = preprocess_observations(&observations, &config);
        // Should have items 17, 18, 19
        assert!(result.contains("item_17"));
        assert!(result.contains("item_18"));
        assert!(result.contains("item_19"));
        assert!(!result.contains("item_16"));
    }
}
