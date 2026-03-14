//! Extraction pipeline orchestrator.
//!
//! `MemoryExtractor` holds references to the store, keyword index, and LLM,
//! and implements extraction stages that convert observations into structured
//! memory entries.

use crate::error::MemoryError;
use crate::extract::prompts::{
    build_agent_profiles_prompt, build_conventions_prompt, build_decisions_prompt,
    build_run_summary_prompt, preprocess_observations,
};
use crate::extract::types::{
    AgentProfileUpdate, Convention, Decision, ExtractionConfig, ExtractionLlm, ExtractionResult,
    RunSummary,
};
use crate::keyword::KeywordIndex;
use crate::observe::storage::ObservationReader;
use crate::observe::types::Observation;
use crate::store::VikingStore;
use crate::types::LayeredContent;
use crate::uri::VikingUri;

/// Orchestrates extraction stages: observations → LLM → structured memory.
pub struct MemoryExtractor<'a> {
    store: &'a VikingStore,
    keywords: &'a mut KeywordIndex,
    llm: &'a dyn ExtractionLlm,
    config: ExtractionConfig,
}

impl<'a> MemoryExtractor<'a> {
    /// Create a new extractor with the given dependencies.
    pub fn new(
        store: &'a VikingStore,
        keywords: &'a mut KeywordIndex,
        llm: &'a dyn ExtractionLlm,
        config: ExtractionConfig,
    ) -> Self {
        Self {
            store,
            keywords,
            llm,
            config,
        }
    }

    /// Extract a run summary from observations and persist it.
    ///
    /// Pipeline: preprocess observations → build prompt → call LLM →
    /// parse RunSummary → write LayeredContent to `viking://runs/<run_id>/summary`
    /// → update keyword index.
    #[tracing::instrument(skip(self, observations), fields(
        run_id = %run_id,
        observation_count = observations.len()
    ))]
    pub async fn extract_run_summary(
        &mut self,
        run_id: &uuid::Uuid,
        observations: &[Observation],
    ) -> Result<(), MemoryError> {
        // 1. Preprocess observations into serialized form
        let observations_jsonl = preprocess_observations(observations, &self.config);

        if observations_jsonl.is_empty() {
            tracing::warn!(run_id = %run_id, "No observations to extract summary from");
            return Ok(());
        }

        // 2. Build prompt
        let prompt = build_run_summary_prompt(&observations_jsonl);

        // 3. Call LLM
        let response = self.llm.complete(&prompt, None).await?;

        // 4. Parse response
        let summary: RunSummary = serde_json::from_str(&response).map_err(|e| {
            tracing::warn!(
                run_id = %run_id,
                raw_response = %response,
                error = %e,
                "Malformed LLM response for run summary"
            );
            MemoryError::ExtractionError {
                stage: "run_summary".to_string(),
                message: format!("Failed to parse LLM response as RunSummary: {e}"),
            }
        })?;

        // 5. Build LayeredContent and write to store
        let uri_str = format!("viking://runs/{run_id}/summary");
        let uri: VikingUri = uri_str.parse().map_err(|e: MemoryError| {
            MemoryError::ExtractionError {
                stage: "run_summary".to_string(),
                message: format!("Failed to construct URI: {e}"),
            }
        })?;

        let content = LayeredContent::new(
            uri.clone(),
            summary.abstract_text.clone(),
            summary.overview.clone(),
        );

        self.store.write(&content)?;
        tracing::info!(uri = %uri, "Wrote run summary to store");

        // 6. Update keyword index with combined text
        let combined_text = format!(
            "{} {} {}",
            summary.abstract_text,
            summary.overview,
            summary.conventions_detected.join(" ")
        );
        self.keywords.add(&uri.to_string(), &combined_text);
        tracing::debug!(uri = %uri, "Updated keyword index for run summary");

        Ok(())
    }

    /// Extract conventions from observations and persist them.
    ///
    /// Reads existing conventions from the store and merges — new conventions
    /// are added, existing ones are updated if confidence is higher.
    /// Writes each to `viking://project/conventions/<slug>`.
    #[tracing::instrument(skip(self, observations), fields(
        run_id = %run_id,
        observation_count = observations.len()
    ))]
    pub async fn extract_conventions(
        &mut self,
        run_id: &uuid::Uuid,
        observations: &[Observation],
    ) -> Result<(), MemoryError> {
        let observations_jsonl = preprocess_observations(observations, &self.config);
        if observations_jsonl.is_empty() {
            tracing::warn!(run_id = %run_id, "No observations to extract conventions from");
            return Ok(());
        }

        let prompt = build_conventions_prompt(&observations_jsonl);
        let response = self.llm.complete(&prompt, None).await?;

        let conventions: Vec<Convention> = serde_json::from_str(&response).map_err(|e| {
            tracing::warn!(
                run_id = %run_id,
                raw_response = %response,
                error = %e,
                "Malformed LLM response for conventions"
            );
            MemoryError::ExtractionError {
                stage: "conventions".to_string(),
                message: format!("Failed to parse LLM response as Vec<Convention>: {e}"),
            }
        })?;

        for conv in &conventions {
            let slug = if conv.id.is_empty() { "unknown" } else { &conv.id };
            let uri_str = format!("viking://project/conventions/{slug}");
            let uri: VikingUri = uri_str.parse().map_err(|e: MemoryError| {
                MemoryError::ExtractionError {
                    stage: "conventions".to_string(),
                    message: format!("Failed to construct URI for convention '{slug}': {e}"),
                }
            })?;

            // Read-then-merge: if an existing entry exists, append evidence
            let (abstract_text, overview_text) =
                if let Some(existing) = self.store.read(&uri)? {
                    let merged_abstract = format!(
                        "{} (updated: confidence {:.1})",
                        existing.abstract_text, conv.confidence
                    );
                    let merged_overview = format!(
                        "{}\n\n### Run {run_id}\n\nEvidence: {}",
                        existing.overview_text,
                        conv.evidence.join("; ")
                    );
                    (merged_abstract, merged_overview)
                } else {
                    let abstract_text = conv.description.clone();
                    let overview_text = format!(
                        "Convention: {}\nConfidence: {:.1}\nEvidence: {}",
                        conv.description,
                        conv.confidence,
                        conv.evidence.join("; ")
                    );
                    (abstract_text, overview_text)
                };

            let content = LayeredContent::new(uri.clone(), abstract_text, overview_text);
            self.store.write(&content)?;
            tracing::info!(uri = %uri, convention_id = %conv.id, "Wrote convention to store");

            let index_text = format!(
                "{} {} {}",
                conv.id,
                conv.description,
                conv.evidence.join(" ")
            );
            self.keywords.add(&uri.to_string(), &index_text);
        }

        Ok(())
    }

    /// Extract decisions from observations and persist them.
    ///
    /// Write-once per run — each decision is stored under
    /// `viking://runs/<run_id>/decisions/<slug>`.
    #[tracing::instrument(skip(self, observations), fields(
        run_id = %run_id,
        observation_count = observations.len()
    ))]
    pub async fn extract_decisions(
        &mut self,
        run_id: &uuid::Uuid,
        observations: &[Observation],
    ) -> Result<(), MemoryError> {
        let observations_jsonl = preprocess_observations(observations, &self.config);
        if observations_jsonl.is_empty() {
            tracing::warn!(run_id = %run_id, "No observations to extract decisions from");
            return Ok(());
        }

        let prompt = build_decisions_prompt(&observations_jsonl);
        let response = self.llm.complete(&prompt, None).await?;

        let decisions: Vec<Decision> = serde_json::from_str(&response).map_err(|e| {
            tracing::warn!(
                run_id = %run_id,
                raw_response = %response,
                error = %e,
                "Malformed LLM response for decisions"
            );
            MemoryError::ExtractionError {
                stage: "decisions".to_string(),
                message: format!("Failed to parse LLM response as Vec<Decision>: {e}"),
            }
        })?;

        for (i, dec) in decisions.iter().enumerate() {
            // Generate a slug from the decision text, or use index
            let slug = slugify(&dec.decision, i);
            let uri_str = format!("viking://runs/{run_id}/decisions/{slug}");
            let uri: VikingUri = uri_str.parse().map_err(|e: MemoryError| {
                MemoryError::ExtractionError {
                    stage: "decisions".to_string(),
                    message: format!("Failed to construct URI for decision '{slug}': {e}"),
                }
            })?;

            let abstract_text = dec.decision.clone();
            let overview_text = format!(
                "Decision: {}\nRationale: {}\nContext: {}\nImpact: {}",
                dec.decision, dec.rationale, dec.context, dec.impact
            );

            let content = LayeredContent::new(uri.clone(), abstract_text, overview_text);
            self.store.write(&content)?;
            tracing::info!(uri = %uri, "Wrote decision to store");

            let index_text = format!(
                "{} {} {} {}",
                dec.decision, dec.rationale, dec.context, dec.impact
            );
            self.keywords.add(&uri.to_string(), &index_text);
        }

        Ok(())
    }

    /// Update agent profiles from observations.
    ///
    /// Reads existing profiles from the store and merges — new observations
    /// are accumulated with prior data. Writes to `viking://agents/<kind>/profile`.
    #[tracing::instrument(skip(self, observations), fields(
        run_id = %run_id,
        observation_count = observations.len()
    ))]
    pub async fn update_agent_profiles(
        &mut self,
        run_id: &uuid::Uuid,
        observations: &[Observation],
    ) -> Result<(), MemoryError> {
        let observations_jsonl = preprocess_observations(observations, &self.config);
        if observations_jsonl.is_empty() {
            tracing::warn!(run_id = %run_id, "No observations to update agent profiles from");
            return Ok(());
        }

        let prompt = build_agent_profiles_prompt(&observations_jsonl);
        let response = self.llm.complete(&prompt, None).await?;

        let profiles: Vec<AgentProfileUpdate> =
            serde_json::from_str(&response).map_err(|e| {
                tracing::warn!(
                    run_id = %run_id,
                    raw_response = %response,
                    error = %e,
                    "Malformed LLM response for agent profiles"
                );
                MemoryError::ExtractionError {
                    stage: "agent_profiles".to_string(),
                    message: format!(
                        "Failed to parse LLM response as Vec<AgentProfileUpdate>: {e}"
                    ),
                }
            })?;

        for profile in &profiles {
            let kind_slug = slugify(&profile.agent_id, 0);
            let uri_str = format!("viking://agents/{kind_slug}/profile");
            let uri: VikingUri = uri_str.parse().map_err(|e: MemoryError| {
                MemoryError::ExtractionError {
                    stage: "agent_profiles".to_string(),
                    message: format!(
                        "Failed to construct URI for agent profile '{kind_slug}': {e}"
                    ),
                }
            })?;

            // Read-then-merge semantics
            let (abstract_text, overview_text) =
                if let Some(existing) = self.store.read(&uri)? {
                    let merged_abstract = format!(
                        "{} | Run {}: {} tasks",
                        existing.abstract_text,
                        &run_id.to_string()[..8],
                        profile.tasks_handled.len()
                    );
                    let new_section = format!(
                        "\n\n### Run {run_id}\n\nTasks: {}\nStrengths: {}\nWeaknesses: {}\nPass rate: {}\nFeedback: {}",
                        profile.tasks_handled.join(", "),
                        profile.strengths.join(", "),
                        profile.weaknesses.join(", "),
                        profile.review_pass_rate.map_or("N/A".to_string(), |r| format!("{:.0}%", r * 100.0)),
                        profile.feedback_themes.join(", "),
                    );
                    let merged_overview =
                        format!("{}{}", existing.overview_text, new_section);
                    (merged_abstract, merged_overview)
                } else {
                    let abstract_text =
                        format!("Agent profile for {}", profile.agent_id);
                    let overview_text = format!(
                        "Agent: {}\nTasks: {}\nStrengths: {}\nWeaknesses: {}\nPass rate: {}\nFeedback: {}",
                        profile.agent_id,
                        profile.tasks_handled.join(", "),
                        profile.strengths.join(", "),
                        profile.weaknesses.join(", "),
                        profile.review_pass_rate.map_or("N/A".to_string(), |r| format!("{:.0}%", r * 100.0)),
                        profile.feedback_themes.join(", "),
                    );
                    (abstract_text, overview_text)
                };

            let content = LayeredContent::new(uri.clone(), abstract_text, overview_text);
            self.store.write(&content)?;
            tracing::info!(uri = %uri, agent_id = %profile.agent_id, "Wrote agent profile to store");

            let index_text = format!(
                "{} {} {} {}",
                profile.agent_id,
                profile.strengths.join(" "),
                profile.weaknesses.join(" "),
                profile.feedback_themes.join(" ")
            );
            self.keywords.add(&uri.to_string(), &index_text);
        }

        Ok(())
    }

    /// Run all four extraction stages sequentially.
    ///
    /// Catches and logs per-stage errors so that one failed stage doesn't
    /// prevent the others from producing results.
    #[tracing::instrument(skip(self), fields(
        run_id = %run_id,
        observations_root = %observations_root.as_ref().display()
    ))]
    pub async fn extract_all(
        &mut self,
        run_id: &uuid::Uuid,
        observations_root: impl AsRef<std::path::Path>,
    ) -> Result<ExtractionResult, MemoryError> {
        // Load observations
        let observations =
            ObservationReader::read_run(observations_root.as_ref(), run_id)?;

        let preprocessed = preprocess_observations(&observations, &self.config);
        if preprocessed.is_empty() {
            tracing::warn!(run_id = %run_id, "No observations for extraction pipeline");
            return Ok(ExtractionResult::new());
        }

        let mut result = ExtractionResult::new();

        // Stage 1: Run summary
        match self.extract_run_summary(run_id, &observations).await {
            Ok(()) => result.succeeded.push("run_summary".to_string()),
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    stage = "run_summary",
                    error = %e,
                    "Extraction stage failed, continuing with remaining stages"
                );
                result.failed.push(("run_summary".to_string(), e));
            }
        }

        // Stage 2: Conventions
        match self.extract_conventions(run_id, &observations).await {
            Ok(()) => result.succeeded.push("conventions".to_string()),
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    stage = "conventions",
                    error = %e,
                    "Extraction stage failed, continuing with remaining stages"
                );
                result.failed.push(("conventions".to_string(), e));
            }
        }

        // Stage 3: Decisions
        match self.extract_decisions(run_id, &observations).await {
            Ok(()) => result.succeeded.push("decisions".to_string()),
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    stage = "decisions",
                    error = %e,
                    "Extraction stage failed, continuing with remaining stages"
                );
                result.failed.push(("decisions".to_string(), e));
            }
        }

        // Stage 4: Agent profiles
        match self.update_agent_profiles(run_id, &observations).await {
            Ok(()) => result.succeeded.push("agent_profiles".to_string()),
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    stage = "agent_profiles",
                    error = %e,
                    "Extraction stage failed, continuing with remaining stages"
                );
                result.failed.push(("agent_profiles".to_string(), e));
            }
        }

        tracing::info!(
            run_id = %run_id,
            succeeded = result.success_count(),
            failed = result.failure_count(),
            "Extraction pipeline completed"
        );

        Ok(result)
    }
}

/// Generate a URL-safe slug from text.
///
/// Takes the first few words, lowercases, replaces non-alphanumeric with hyphens.
/// Falls back to `"item-{index}"` if the text is empty.
fn slugify(text: &str, index: usize) -> String {
    let slug: String = text
        .to_lowercase()
        .split_whitespace()
        .take(4)
        .collect::<Vec<&str>>()
        .join("-")
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    // Collapse multiple hyphens
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    let trimmed = result.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("item-{index}")
    } else {
        trimmed
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::extract::types::ExtractionLlm;
    use crate::observe::storage::ObservationWriter;
    use crate::observe::types::{ObservationType, Observation};
    use async_trait::async_trait;
    use ath_types::{AgentId, Severity, TokenUsage};
    use chrono::Utc;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Mock LLM that dispatches canned responses based on prompt content.
    pub(crate) struct MockExtractionLlm {
        /// Canned response pairs: (prompt_contains, response)
        responses: Vec<(String, String)>,
    }

    impl MockExtractionLlm {
        /// Create a mock that returns the given response when the prompt
        /// contains the given substring.
        pub(crate) fn new(responses: Vec<(String, String)>) -> Self {
            Self { responses }
        }

        /// Create a mock pre-loaded with a valid run summary response.
        pub(crate) fn with_run_summary() -> Self {
            let response = serde_json::json!({
                "abstract_text": "Built authentication module with JWT token support.",
                "overview": "Phase 1: Claude planned the auth architecture. Phase 2: Gemini implemented JWT middleware and login endpoint. Phase 3: Codex reviewed and found missing error handling. Retry succeeded on attempt 2. Files modified: src/auth/mod.rs, src/auth/jwt.rs, src/middleware/auth.rs.",
                "conventions_detected": ["error types use hint() method", "hexagonal architecture"],
                "decisions": [
                    {"decision": "Use JWT for authentication", "rationale": "Stateless, scales horizontally"}
                ],
                "issues": [
                    {"issue": "Missing error handling in auth middleware", "resolution": "Added Result return types on retry"}
                ]
            });

            Self::new(vec![(
                "abstract_text".to_string(),
                response.to_string(),
            )])
        }

        /// Create a mock pre-loaded with responses for all four extraction stages.
        pub(crate) fn with_all_stages() -> Self {
            let run_summary = serde_json::json!({
                "abstract_text": "Built authentication module with JWT token support.",
                "overview": "Phase 1: Claude planned the auth architecture. Phase 2: Gemini implemented JWT middleware. Phase 3: Codex reviewed and found missing error handling.",
                "conventions_detected": ["error types use hint() method"],
                "decisions": [
                    {"decision": "Use JWT for authentication", "rationale": "Stateless"}
                ],
                "issues": [
                    {"issue": "Missing error handling", "resolution": "Added Result return types"}
                ]
            });

            let conventions = serde_json::json!([
                {
                    "id": "error-handling-pattern",
                    "description": "All error types implement a hint() method returning actionable resolution text.",
                    "confidence": 0.9,
                    "evidence": ["MemoryError uses hint()", "Custom error types follow same pattern"]
                },
                {
                    "id": "hexagonal-architecture",
                    "description": "Project uses hexagonal architecture with ports and adapters.",
                    "confidence": 0.7,
                    "evidence": ["Traits used as ports", "Concrete implementations as adapters"]
                }
            ]);

            let decisions = serde_json::json!([
                {
                    "decision": "Use JWT for authentication",
                    "rationale": "Stateless, scales horizontally without session storage",
                    "context": "Phase 1, auth-planning task by Claude",
                    "impact": "All API routes require JWT validation middleware"
                }
            ]);

            let agent_profiles = serde_json::json!([
                {
                    "agent_id": "claude/opus-4",
                    "tasks_handled": ["auth-planning"],
                    "strengths": ["architectural thinking", "clear documentation"],
                    "weaknesses": [],
                    "review_pass_rate": null,
                    "feedback_themes": []
                },
                {
                    "agent_id": "codex/o3",
                    "tasks_handled": ["code-review"],
                    "strengths": ["thorough review"],
                    "weaknesses": ["sometimes overly strict"],
                    "review_pass_rate": 0.5,
                    "feedback_themes": ["missing error handling"]
                }
            ]);

            Self::new(vec![
                ("abstract_text".to_string(), run_summary.to_string()),
                ("convention".to_string(), conventions.to_string()),
                ("decision".to_string(), decisions.to_string()),
                ("agent_id".to_string(), agent_profiles.to_string()),
            ])
        }

        /// Create a mock for a second run (different conventions for merge testing).
        pub(crate) fn with_all_stages_run2() -> Self {
            let run_summary = serde_json::json!({
                "abstract_text": "Added database migration system.",
                "overview": "Phase 1: Claude designed migration strategy. Phase 2: Gemini implemented migration runner.",
                "conventions_detected": ["migration naming convention"],
                "decisions": [],
                "issues": []
            });

            let conventions = serde_json::json!([
                {
                    "id": "error-handling-pattern",
                    "description": "All error types implement a hint() method.",
                    "confidence": 0.95,
                    "evidence": ["Second run also uses hint() pattern", "Consistent across modules"]
                },
                {
                    "id": "migration-naming",
                    "description": "Database migrations use timestamp-prefixed names.",
                    "confidence": 0.8,
                    "evidence": ["Migration files follow YYYYMMDD_name.sql pattern"]
                }
            ]);

            let decisions = serde_json::json!([
                {
                    "decision": "Use SQLite for migrations",
                    "rationale": "Simple embedded database for dev workflow",
                    "context": "Phase 1, migration-design by Claude",
                    "impact": "Migration runner targets SQLite dialect"
                }
            ]);

            let agent_profiles = serde_json::json!([
                {
                    "agent_id": "claude/opus-4",
                    "tasks_handled": ["migration-design"],
                    "strengths": ["system design"],
                    "weaknesses": [],
                    "review_pass_rate": null,
                    "feedback_themes": []
                }
            ]);

            Self::new(vec![
                ("abstract_text".to_string(), run_summary.to_string()),
                ("convention".to_string(), conventions.to_string()),
                ("decision".to_string(), decisions.to_string()),
                ("agent_id".to_string(), agent_profiles.to_string()),
            ])
        }
    }

    #[async_trait]
    impl ExtractionLlm for MockExtractionLlm {
        async fn complete(
            &self,
            prompt: &str,
            _json_schema: Option<&serde_json::Value>,
        ) -> Result<String, MemoryError> {
            for (contains, response) in &self.responses {
                if prompt.contains(contains) {
                    return Ok(response.clone());
                }
            }
            Err(MemoryError::ExtractionError {
                stage: "mock".to_string(),
                message: "No matching mock response for prompt".to_string(),
            })
        }
    }

    /// Create fixture observations for a run.
    fn fixture_observations(run_id: Uuid) -> Vec<Observation> {
        let now = Utc::now();
        vec![
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(1),
                event: ObservationType::AgentRequest {
                    agent: AgentId::claude("opus-4"),
                    prompt_summary: "Plan authentication module".into(),
                    phase_id: Some(1),
                    task_name: Some("auth-planning".into()),
                },
            },
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(1),
                event: ObservationType::AgentResponse {
                    agent: AgentId::claude("opus-4"),
                    token_usage: TokenUsage {
                        input_tokens: 3000,
                        output_tokens: 1500,
                        estimated_cost_usd: 0.12,
                    },
                    phase_id: Some(1),
                    task_name: Some("auth-planning".into()),
                },
            },
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(2),
                event: ObservationType::FileOperation {
                    op: crate::observe::types::FileOpKind::Create,
                    path: "src/auth/mod.rs".into(),
                    phase_id: Some(2),
                },
            },
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(3),
                event: ObservationType::ReviewVerdict {
                    reviewer: AgentId::codex("o3"),
                    passed: false,
                    severity: Severity::Warning,
                    reason_summary: "Missing error handling in auth flow".into(),
                    attempt_number: 1,
                    phase_id: Some(3),
                },
            },
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(3),
                event: ObservationType::RetryStarted {
                    phase_id: Some(3),
                    attempt_number: 2,
                    feedback_summary: "Add Result return types".into(),
                },
            },
            Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: now,
                phase_id: Some(3),
                event: ObservationType::ReviewVerdict {
                    reviewer: AgentId::codex("o3"),
                    passed: true,
                    severity: Severity::Info,
                    reason_summary: "All issues resolved".into(),
                    attempt_number: 2,
                    phase_id: Some(3),
                },
            },
        ]
    }

    #[tokio::test]
    async fn extract_run_summary_writes_store_entry() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_run_summary();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        extractor
            .extract_run_summary(&run_id, &observations)
            .await
            .unwrap();

        // Verify store entry exists at expected URI
        let uri_str = format!("viking://runs/{run_id}/summary");
        let uri: VikingUri = uri_str.parse().unwrap();
        let content = store.read(&uri).unwrap().expect("store entry should exist");

        assert_eq!(
            content.abstract_text,
            "Built authentication module with JWT token support."
        );
        assert!(content.overview_text.contains("Phase 1"));
        assert!(content.overview_text.contains("Claude"));
    }

    #[tokio::test]
    async fn extract_run_summary_updates_keyword_index() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_run_summary();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        extractor
            .extract_run_summary(&run_id, &observations)
            .await
            .unwrap();

        // Search for content from the summary
        let results = keywords.search("authentication JWT", 5);
        assert!(!results.is_empty(), "keyword index should find the summary");
        let expected_uri = format!("viking://runs/{run_id}/summary");
        assert_eq!(results[0].uri_str, expected_uri);
    }

    #[tokio::test]
    async fn extract_run_summary_malformed_response_returns_error() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();

        // Mock that returns invalid JSON
        let mock_llm = MockExtractionLlm::new(vec![(
            "abstract_text".to_string(),
            "This is not valid JSON at all!".to_string(),
        )]);
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        let result = extractor
            .extract_run_summary(&run_id, &observations)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, MemoryError::ExtractionError { .. }));
        assert!(err.hint().contains("LLM response"));

        // Verify store was NOT written
        let uri_str = format!("viking://runs/{run_id}/summary");
        let uri: VikingUri = uri_str.parse().unwrap();
        assert!(store.read(&uri).unwrap().is_none());
    }

    #[tokio::test]
    async fn extract_run_summary_empty_observations_is_noop() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_run_summary();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        // Should succeed without calling LLM
        extractor
            .extract_run_summary(&run_id, &[])
            .await
            .unwrap();

        // Nothing should be in the store
        let uri_str = format!("viking://runs/{run_id}/summary");
        let uri: VikingUri = uri_str.parse().unwrap();
        assert!(store.read(&uri).unwrap().is_none());
    }

    #[tokio::test]
    async fn extraction_error_hint_is_actionable() {
        let err = MemoryError::ExtractionError {
            stage: "run_summary".to_string(),
            message: "parse failed".to_string(),
        };
        let hint = err.hint();
        assert!(hint.len() > 20, "hint should be actionable: '{hint}'");
        assert!(
            hint.contains("LLM response") || hint.contains("malformed"),
            "hint should mention LLM response: '{hint}'"
        );
    }

    #[tokio::test]
    async fn mock_llm_returns_deterministic_json() {
        let mock = MockExtractionLlm::with_run_summary();
        let response = mock
            .complete("Prompt with abstract_text keyword", None)
            .await
            .unwrap();

        // Verify it's valid JSON that parses to RunSummary
        let summary: RunSummary = serde_json::from_str(&response).unwrap();
        assert!(!summary.abstract_text.is_empty());
        assert!(!summary.overview.is_empty());
    }

    // ── Convention extraction tests ─────────────────────────────────

    #[tokio::test]
    async fn extract_conventions_writes_store_entries() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_all_stages();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        extractor
            .extract_conventions(&run_id, &observations)
            .await
            .unwrap();

        // Should have two convention entries
        let uri1: VikingUri = "viking://project/conventions/error-handling-pattern"
            .parse()
            .unwrap();
        let content1 = store.read(&uri1).unwrap().expect("convention 1 should exist");
        assert!(content1.abstract_text.contains("hint()"));

        let uri2: VikingUri = "viking://project/conventions/hexagonal-architecture"
            .parse()
            .unwrap();
        let content2 = store.read(&uri2).unwrap().expect("convention 2 should exist");
        assert!(content2.abstract_text.contains("hexagonal"));
    }

    #[tokio::test]
    async fn extract_conventions_merges_with_existing() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let run_id_1 = Uuid::new_v4();
        let run_id_2 = Uuid::new_v4();
        let observations = fixture_observations(run_id_1);

        // First extraction
        let mock_llm_1 = MockExtractionLlm::with_all_stages();
        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm_1, config.clone());
        extractor
            .extract_conventions(&run_id_1, &observations)
            .await
            .unwrap();

        // Second extraction with different mock (has overlapping "error-handling-pattern")
        let mock_llm_2 = MockExtractionLlm::with_all_stages_run2();
        let mut extractor2 = MemoryExtractor::new(&store, &mut keywords, &mock_llm_2, config);
        extractor2
            .extract_conventions(&run_id_2, &observations)
            .await
            .unwrap();

        // Verify merge: error-handling-pattern should contain evidence from both runs
        let uri: VikingUri = "viking://project/conventions/error-handling-pattern"
            .parse()
            .unwrap();
        let content = store.read(&uri).unwrap().expect("convention should exist");
        // The merged overview should contain reference to run_2
        assert!(
            content.overview_text.contains(&format!("Run {run_id_2}")),
            "Overview should reference second run: {}",
            content.overview_text
        );

        // New convention from second run should also exist
        let uri_new: VikingUri = "viking://project/conventions/migration-naming"
            .parse()
            .unwrap();
        assert!(
            store.read(&uri_new).unwrap().is_some(),
            "New convention from run 2 should exist"
        );
    }

    // ── Decision extraction tests ───────────────────────────────────

    #[tokio::test]
    async fn extract_decisions_writes_per_run_uris() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_all_stages();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        extractor
            .extract_decisions(&run_id, &observations)
            .await
            .unwrap();

        // Slug for "Use JWT for authentication" → "use-jwt-for-authentication"
        let uri: VikingUri = format!("viking://runs/{run_id}/decisions/use-jwt-for-authentication")
            .parse()
            .unwrap();
        let content = store.read(&uri).unwrap().expect("decision should exist");
        assert!(content.abstract_text.contains("JWT"));
        assert!(content.overview_text.contains("Rationale"));
    }

    #[tokio::test]
    async fn extract_decisions_no_cross_run_collision() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let run_id_1 = Uuid::new_v4();
        let run_id_2 = Uuid::new_v4();
        let observations = fixture_observations(run_id_1);

        // Extract decisions for run 1
        let mock_1 = MockExtractionLlm::with_all_stages();
        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_1, config.clone());
        extractor
            .extract_decisions(&run_id_1, &observations)
            .await
            .unwrap();

        // Extract decisions for run 2 (different decision)
        let mock_2 = MockExtractionLlm::with_all_stages_run2();
        let mut extractor2 = MemoryExtractor::new(&store, &mut keywords, &mock_2, config);
        extractor2
            .extract_decisions(&run_id_2, &observations)
            .await
            .unwrap();

        // Both runs should have their own decision URIs
        let uri_1: VikingUri = format!("viking://runs/{run_id_1}/decisions/use-jwt-for-authentication")
            .parse()
            .unwrap();
        let uri_2: VikingUri = format!("viking://runs/{run_id_2}/decisions/use-sqlite-for-migrations")
            .parse()
            .unwrap();
        assert!(store.read(&uri_1).unwrap().is_some(), "run 1 decision should exist");
        assert!(store.read(&uri_2).unwrap().is_some(), "run 2 decision should exist");
    }

    // ── Agent profile tests ─────────────────────────────────────────

    #[tokio::test]
    async fn update_agent_profiles_writes_entries() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_all_stages();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let observations = fixture_observations(run_id);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        extractor
            .update_agent_profiles(&run_id, &observations)
            .await
            .unwrap();

        // "claude/opus-4" slugifies to "claude-opus-4"
        let uri: VikingUri = "viking://agents/claude-opus-4/profile".parse().unwrap();
        let content = store.read(&uri).unwrap().expect("claude profile should exist");
        assert!(content.abstract_text.contains("claude/opus-4"));

        let uri2: VikingUri = "viking://agents/codex-o3/profile".parse().unwrap();
        let content2 = store.read(&uri2).unwrap().expect("codex profile should exist");
        assert!(content2.overview_text.contains("thorough review"));
    }

    #[tokio::test]
    async fn update_agent_profiles_merges_across_runs() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let run_id_1 = Uuid::new_v4();
        let run_id_2 = Uuid::new_v4();
        let observations = fixture_observations(run_id_1);

        // First extraction
        let mock_1 = MockExtractionLlm::with_all_stages();
        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_1, config.clone());
        extractor
            .update_agent_profiles(&run_id_1, &observations)
            .await
            .unwrap();

        // Second extraction for claude (different tasks)
        let mock_2 = MockExtractionLlm::with_all_stages_run2();
        let mut extractor2 = MemoryExtractor::new(&store, &mut keywords, &mock_2, config);
        extractor2
            .update_agent_profiles(&run_id_2, &observations)
            .await
            .unwrap();

        // Claude profile should have merged data from both runs
        let uri: VikingUri = "viking://agents/claude-opus-4/profile".parse().unwrap();
        let content = store.read(&uri).unwrap().expect("claude profile should exist");
        assert!(
            content.overview_text.contains(&format!("Run {run_id_2}")),
            "Overview should contain second run data: {}",
            content.overview_text
        );
    }

    // ── extract_all tests ───────────────────────────────────────────

    #[tokio::test]
    async fn extract_all_continues_on_stage_failure() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let obs_root = dir.path().join("observations");

        // Write fixture observations to disk
        let mut writer = ObservationWriter::new(&obs_root, &run_id).unwrap();
        for obs in fixture_observations(run_id) {
            writer.write(&obs).unwrap();
        }
        writer.flush().unwrap();

        // Mock that fails on conventions but succeeds on everything else
        let mock_llm = MockExtractionLlm::new(vec![
            (
                "abstract_text".to_string(),
                serde_json::json!({
                    "abstract_text": "Summary works.",
                    "overview": "Overview works.",
                    "conventions_detected": [],
                    "decisions": [],
                    "issues": []
                })
                .to_string(),
            ),
            // No "convention" match → conventions stage will fail
            (
                "decision".to_string(),
                serde_json::json!([{
                    "decision": "Test decision",
                    "rationale": "Test rationale",
                    "context": "Test context",
                    "impact": "Test impact"
                }])
                .to_string(),
            ),
            (
                "agent_id".to_string(),
                serde_json::json!([{
                    "agent_id": "claude/opus-4",
                    "tasks_handled": ["test"],
                    "strengths": ["testing"],
                    "weaknesses": [],
                    "review_pass_rate": null,
                    "feedback_themes": []
                }])
                .to_string(),
            ),
        ]);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        let result = extractor.extract_all(&run_id, &obs_root).await.unwrap();

        // 3 should succeed, 1 should fail (conventions)
        assert_eq!(result.success_count(), 3, "3 stages should succeed");
        assert_eq!(result.failure_count(), 1, "1 stage should fail");
        assert_eq!(result.failed[0].0, "conventions");

        // Verify the other stages actually produced results
        let summary_uri: VikingUri = format!("viking://runs/{run_id}/summary").parse().unwrap();
        assert!(store.read(&summary_uri).unwrap().is_some(), "summary should exist despite convention failure");
    }

    #[tokio::test]
    async fn extract_all_returns_result_struct() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_all_stages();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let obs_root = dir.path().join("observations");

        let mut writer = ObservationWriter::new(&obs_root, &run_id).unwrap();
        for obs in fixture_observations(run_id) {
            writer.write(&obs).unwrap();
        }
        writer.flush().unwrap();

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        let result = extractor.extract_all(&run_id, &obs_root).await.unwrap();

        assert!(result.all_succeeded(), "all stages should succeed");
        assert_eq!(result.success_count(), 4);
        assert_eq!(result.failure_count(), 0);
        assert!(result.succeeded.contains(&"run_summary".to_string()));
        assert!(result.succeeded.contains(&"conventions".to_string()));
        assert!(result.succeeded.contains(&"decisions".to_string()));
        assert!(result.succeeded.contains(&"agent_profiles".to_string()));
    }

    // ── Slugify tests ───────────────────────────────────────────────

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Use JWT for authentication", 0), "use-jwt-for-authentication");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("claude/opus-4", 0), "claude-opus-4");
    }

    #[test]
    fn slugify_empty_falls_back() {
        assert_eq!(slugify("", 3), "item-3");
    }

    #[test]
    fn slugify_truncates_at_4_words() {
        assert_eq!(
            slugify("one two three four five six", 0),
            "one-two-three-four"
        );
    }

    // ── Integration tests ───────────────────────────────────────────

    /// Full pipeline integration test: fixture observations → extract_all →
    /// verify store entries at expected URIs → verify keyword index finds entries.
    #[tokio::test]
    async fn integration_full_pipeline() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let mock_llm = MockExtractionLlm::with_all_stages();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let obs_root = dir.path().join("observations");

        // Write fixture observations via ObservationWriter
        let mut writer = ObservationWriter::new(&obs_root, &run_id).unwrap();
        for obs in fixture_observations(run_id) {
            writer.write(&obs).unwrap();
        }
        writer.flush().unwrap();

        // Run full pipeline
        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        let result = extractor.extract_all(&run_id, &obs_root).await.unwrap();

        assert!(result.all_succeeded(), "all 4 stages should succeed: {:?}", result);
        assert_eq!(result.success_count(), 4);

        // Verify store entries at expected URIs
        let summary_uri: VikingUri = format!("viking://runs/{run_id}/summary").parse().unwrap();
        let summary = store.read(&summary_uri).unwrap().expect("summary should exist");
        assert!(summary.abstract_text.contains("authentication"));

        let conv_uri: VikingUri = "viking://project/conventions/error-handling-pattern"
            .parse()
            .unwrap();
        let conv = store.read(&conv_uri).unwrap().expect("convention should exist");
        assert!(conv.abstract_text.contains("hint()"));

        let decision_uri: VikingUri =
            format!("viking://runs/{run_id}/decisions/use-jwt-for-authentication")
                .parse()
                .unwrap();
        let decision = store.read(&decision_uri).unwrap().expect("decision should exist");
        assert!(decision.abstract_text.contains("JWT"));

        let agent_uri: VikingUri = "viking://agents/claude-opus-4/profile".parse().unwrap();
        let agent = store.read(&agent_uri).unwrap().expect("agent profile should exist");
        assert!(agent.abstract_text.contains("claude/opus-4"));

        // Verify keyword index returns hits
        let summary_hits = keywords.search("authentication JWT", 5);
        assert!(!summary_hits.is_empty(), "keyword index should find summary");

        let convention_hits = keywords.search("error handling hint", 5);
        assert!(!convention_hits.is_empty(), "keyword index should find convention");

        let decision_hits = keywords.search("JWT stateless", 5);
        assert!(!decision_hits.is_empty(), "keyword index should find decision");

        // Verify we have at least 4 URIs in the store
        let all_uris = store.list().unwrap();
        assert!(
            all_uris.len() >= 4,
            "store should have at least 4 entries, got {}",
            all_uris.len()
        );
    }

    /// Merge behavior test: two extract_all calls accumulate conventions.
    #[tokio::test]
    async fn integration_merge_conventions_across_runs() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let obs_root = dir.path().join("observations");

        // Run 1
        let run_id_1 = Uuid::new_v4();
        let mut writer1 = ObservationWriter::new(&obs_root, &run_id_1).unwrap();
        for obs in fixture_observations(run_id_1) {
            writer1.write(&obs).unwrap();
        }
        writer1.flush().unwrap();

        let mock_1 = MockExtractionLlm::with_all_stages();
        let mut extractor1 = MemoryExtractor::new(&store, &mut keywords, &mock_1, config.clone());
        let result1 = extractor1.extract_all(&run_id_1, &obs_root).await.unwrap();
        assert!(result1.all_succeeded(), "run 1 should succeed");

        // Snapshot: check "error-handling-pattern" after run 1
        let conv_uri: VikingUri = "viking://project/conventions/error-handling-pattern"
            .parse()
            .unwrap();
        let conv_after_run1 = store.read(&conv_uri).unwrap().expect("convention should exist");
        let overview_after_run1 = conv_after_run1.overview_text.clone();

        // Run 2 (overlapping convention + new one)
        let run_id_2 = Uuid::new_v4();
        let mut writer2 = ObservationWriter::new(&obs_root, &run_id_2).unwrap();
        for mut obs in fixture_observations(run_id_2) {
            obs.run_id = run_id_2;
            writer2.write(&obs).unwrap();
        }
        writer2.flush().unwrap();

        let mock_2 = MockExtractionLlm::with_all_stages_run2();
        let mut extractor2 = MemoryExtractor::new(&store, &mut keywords, &mock_2, config);
        let result2 = extractor2.extract_all(&run_id_2, &obs_root).await.unwrap();
        assert!(result2.all_succeeded(), "run 2 should succeed");

        // Verify merge: error-handling-pattern accumulated, not replaced
        let conv_after_run2 = store.read(&conv_uri).unwrap().expect("convention should exist");
        assert!(
            conv_after_run2.overview_text.len() > overview_after_run1.len(),
            "Merged overview should be longer. Before: {}, After: {}",
            overview_after_run1.len(),
            conv_after_run2.overview_text.len()
        );
        assert!(
            conv_after_run2.overview_text.contains(&format!("Run {run_id_2}")),
            "Merged overview should reference run 2"
        );

        // New convention from run 2 should also exist
        let new_conv_uri: VikingUri = "viking://project/conventions/migration-naming"
            .parse()
            .unwrap();
        assert!(
            store.read(&new_conv_uri).unwrap().is_some(),
            "New convention from run 2 should exist"
        );

        // Run 1's unique convention should still exist
        let hex_uri: VikingUri = "viking://project/conventions/hexagonal-architecture"
            .parse()
            .unwrap();
        assert!(
            store.read(&hex_uri).unwrap().is_some(),
            "Run 1's hexagonal-architecture convention should still exist"
        );
    }

    /// Malformed response from one stage doesn't prevent others.
    #[tokio::test]
    async fn integration_malformed_response_isolated() {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let mut keywords = KeywordIndex::new();
        let config = ExtractionConfig::default();

        let run_id = Uuid::new_v4();
        let obs_root = dir.path().join("observations");

        let mut writer = ObservationWriter::new(&obs_root, &run_id).unwrap();
        for obs in fixture_observations(run_id) {
            writer.write(&obs).unwrap();
        }
        writer.flush().unwrap();

        // Mock where conventions returns garbage, everything else works
        let mock_llm = MockExtractionLlm::new(vec![
            (
                "abstract_text".to_string(),
                serde_json::json!({
                    "abstract_text": "Works fine.",
                    "overview": "All good.",
                    "conventions_detected": [],
                    "decisions": [],
                    "issues": []
                })
                .to_string(),
            ),
            (
                "convention".to_string(),
                "NOT VALID JSON AT ALL {{{{".to_string(),
            ),
            (
                "decision".to_string(),
                serde_json::json!([]).to_string(),
            ),
            (
                "agent_id".to_string(),
                serde_json::json!([]).to_string(),
            ),
        ]);

        let mut extractor = MemoryExtractor::new(&store, &mut keywords, &mock_llm, config);
        let result = extractor.extract_all(&run_id, &obs_root).await.unwrap();

        // Conventions should fail, others should succeed
        assert_eq!(result.failure_count(), 1);
        assert_eq!(result.failed[0].0, "conventions");
        assert!(result.succeeded.contains(&"run_summary".to_string()));
        assert!(result.succeeded.contains(&"decisions".to_string()));
        assert!(result.succeeded.contains(&"agent_profiles".to_string()));

        // Summary should still be in the store
        let summary_uri: VikingUri = format!("viking://runs/{run_id}/summary").parse().unwrap();
        assert!(store.read(&summary_uri).unwrap().is_some());
    }
}
