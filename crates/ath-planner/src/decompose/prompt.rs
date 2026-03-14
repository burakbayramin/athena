//! LLM decomposition prompt and structured output schema.
//!
//! Provides the system prompt, JSON schema, and request builder for
//! decomposing a ProjectSpec into phases via Claude.

use ath_types::agent::{AgentId, AgentRequest};
use ath_types::project::ProjectSpec;
use chrono::Utc;
use uuid::Uuid;

/// System prompt instructing Claude to decompose a ProjectSpec into phases.
pub const DECOMPOSE_SYSTEM_PROMPT: &str = r#"You are a project decomposition engine. Given a ProjectSpec (JSON), decompose it into an execution plan of ordered phases.

Rules:
- Generate 3-10 phases (soft bound)
- Each phase must have:
  - id: unique integer, sequential starting from 1
  - name: short descriptive name
  - description: what this phase accomplishes
  - tasks: 2-5 tasks per phase (ordered, sequential execution)
  - depends_on: array of phase IDs this phase depends on (must be acyclic)
  - produces: array of contract labels this phase makes available to dependents
  - consumes: array of contract labels this phase needs from its dependencies
- Each task must have:
  - name: short descriptive name
  - description: what to implement
  - skill_tags: array of plain strings (e.g., "rust", "api-design", "testing")
  - expected_output_files: files this task creates or modifies
  - acceptance_criteria: how to verify the task is complete
  - goal_indices: array of 0-based indices into the ProjectSpec goals array
- Every goal in the ProjectSpec must be covered by at least one task's goal_indices
- Dependencies must form a directed acyclic graph (no circular dependencies)
- Each consumed contract must be produced by some phase in the dependency chain
- Respond with valid JSON matching the provided schema"#;

/// Returns the JSON schema for the LLM's structured output (phases array only).
///
/// The LLM generates only `phases`; `execution_order`, `parallel_groups`, and
/// `critical_path_length` are computed server-side after validation.
pub fn execution_plan_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "phases": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "tasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "description": { "type": "string" },
                                    "skill_tags": { "type": "array", "items": { "type": "string" } },
                                    "expected_output_files": { "type": "array", "items": { "type": "string" } },
                                    "acceptance_criteria": { "type": "array", "items": { "type": "string" } },
                                    "goal_indices": { "type": "array", "items": { "type": "integer" } }
                                },
                                "required": ["name", "description", "skill_tags", "expected_output_files", "acceptance_criteria", "goal_indices"]
                            }
                        },
                        "depends_on": { "type": "array", "items": { "type": "integer" } },
                        "produces": { "type": "array", "items": { "type": "string" } },
                        "consumes": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["id", "name", "description", "tasks", "depends_on", "produces", "consumes"]
                }
            }
        },
        "required": ["phases"]
    })
}

/// Build an AgentRequest for decomposing a ProjectSpec into phases.
///
/// Serializes the ProjectSpec as JSON in the user message. If `last_error` is Some,
/// appends error feedback for retry.
pub fn build_decompose_request(project: &ProjectSpec, last_error: Option<&str>) -> AgentRequest {
    let project_json =
        serde_json::to_string_pretty(project).expect("ProjectSpec should serialize to JSON");

    let mut prompt = format!(
        "Decompose the following ProjectSpec into an execution plan:\n\n```json\n{}\n```",
        project_json
    );

    if let Some(err) = last_error {
        prompt.push_str(&format!(
            "\n\nYour previous attempt had errors:\n{}\n\nPlease fix these issues and try again.",
            err
        ));
    }

    AgentRequest {
        id: Uuid::new_v4(),
        agent: AgentId::claude("decompose"),
        prompt,
        context: Some(DECOMPOSE_SYSTEM_PROMPT.to_string()),
        json_schema: Some(execution_plan_json_schema()),
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::project::{GoalSpec, SkillTag};

    fn test_project() -> ProjectSpec {
        ProjectSpec {
            name: "test-app".into(),
            description: "A test application".into(),
            goals: vec![GoalSpec {
                description: "Build the API".into(),
                skill_tags: vec![SkillTag("rust".into())],
            }],
            constraints: vec![],
            target_language: Some("Rust".into()),
            target_framework: None,
            expected_files: vec!["src/main.rs".into()],
        }
    }

    #[test]
    fn execution_plan_json_schema_is_valid_object() {
        let schema = execution_plan_json_schema();
        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "object");

        let props = obj.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("phases"));

        let required = obj.get("required").unwrap().as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"phases"));
    }

    #[test]
    fn build_decompose_request_includes_project_spec() {
        let project = test_project();
        let req = build_decompose_request(&project, None);
        assert!(req.prompt.contains("test-app"));
        assert!(req.prompt.contains("Build the API"));
        assert_eq!(req.context.as_deref(), Some(DECOMPOSE_SYSTEM_PROMPT));
        assert!(req.json_schema.is_some());
        assert!(req.agent.is_claude() && req.agent.model() == "decompose");
    }

    #[test]
    fn build_decompose_request_with_error_appends_feedback() {
        let project = test_project();
        let req =
            build_decompose_request(&project, Some("Circular dependency detected: 1 -> 2 -> 1"));
        assert!(req.prompt.contains("test-app"));
        assert!(req.prompt.contains("Your previous attempt had errors"));
        assert!(req.prompt.contains("Circular dependency detected"));
    }

    #[test]
    fn build_decompose_request_without_error_has_no_feedback() {
        let project = test_project();
        let req = build_decompose_request(&project, None);
        assert!(!req.prompt.contains("previous attempt"));
    }
}
