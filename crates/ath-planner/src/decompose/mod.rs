//! Phase decomposition: ProjectSpec -> ExecutionPlan via LLM with validation.
//!
//! This module provides the core decomposition pipeline:
//! 1. Send ProjectSpec to Claude with a structured prompt
//! 2. Parse the structured JSON response into phases
//! 3. Validate the DAG structure (cycles, orphans, contracts)
//! 4. Retry with error feedback on failure (up to 3 attempts)
//! 5. Compute execution order, parallel groups, and critical path

pub mod dag;
pub mod display;
pub mod error;
pub mod prompt;
pub mod validate;

pub use display::display_execution_plan;
pub use error::DecomposeError;
pub use validate::PlanWarning;

use ath_agents::backend::AgentBackend;
use ath_types::plan::{ExecutionPlan, PhaseSpec};
use ath_types::project::ProjectSpec;
use ath_types::ValidationError;
use serde::Deserialize;

use dag::{compute_parallel_groups, critical_path_length, topological_sort};
use prompt::build_decompose_request;
use validate::validate_plan;

/// Maximum number of decomposition attempts before giving up.
const MAX_DECOMPOSE_ATTEMPTS: usize = 3;

/// Raw LLM response wrapper -- the LLM returns `{"phases": [...]}`.
#[derive(Deserialize)]
struct RawPlanResponse {
    phases: Vec<PhaseSpec>,
}

/// Decompose a ProjectSpec into an ExecutionPlan via LLM with retry logic.
///
/// Follows the `parse_to_project_spec` pattern:
/// 1. Build request (with error feedback on retry)
/// 2. Send to backend
/// 3. Parse response as phases
/// 4. Validate DAG structure
/// 5. On failure, retry with errors appended to prompt
/// 6. On success, compute execution_order, parallel_groups, critical_path_length
///
/// Returns `(ExecutionPlan, Vec<PlanWarning>)` on success. Warnings are non-blocking.
pub async fn decompose_project_spec(
    project: &ProjectSpec,
    backend: &dyn AgentBackend,
) -> Result<(ExecutionPlan, Vec<PlanWarning>), DecomposeError> {
    let mut last_error: Option<String> = None;

    for _attempt in 0..MAX_DECOMPOSE_ATTEMPTS {
        let request = build_decompose_request(project, last_error.as_deref());
        let response = backend.send(request).await.map_err(DecomposeError::Agent)?;

        // Parse the LLM response as phases
        let raw = match serde_json::from_str::<RawPlanResponse>(&response.content) {
            Ok(raw) => raw,
            Err(e) => {
                last_error = Some(format!("JSON parse error: {}", e));
                continue;
            }
        };

        // Validate the phase graph
        match validate_plan(&raw.phases, project) {
            Ok(warnings) => {
                // Compute DAG-derived fields
                let sorted = topological_sort(&raw.phases).map_err(|_| {
                    // This shouldn't happen since validate_plan already checked,
                    // but handle gracefully
                    DecomposeError::DecomposeFailed {
                        attempts: MAX_DECOMPOSE_ATTEMPTS,
                        last_error: Some("Unexpected cycle after validation".into()),
                    }
                })?;

                let parallel_groups = compute_parallel_groups(&raw.phases, &sorted);
                let cpl = critical_path_length(&raw.phases, &sorted);

                let plan = ExecutionPlan {
                    phases: raw.phases,
                    execution_order: sorted,
                    parallel_groups,
                    critical_path_length: cpl,
                };

                return Ok((plan, warnings));
            }
            Err(errors) => {
                last_error = Some(format_validation_errors(&errors));
            }
        }
    }

    Err(DecomposeError::DecomposeFailed {
        attempts: MAX_DECOMPOSE_ATTEMPTS,
        last_error,
    })
}

/// Format validation errors into a summary string for retry feedback.
///
/// Caps at 5 errors to avoid prompt bloat.
fn format_validation_errors(errors: &[ValidationError]) -> String {
    let mut lines: Vec<String> = errors.iter().take(5).map(|e| format!("- {}", e)).collect();

    if errors.len() > 5 {
        lines.push(format!("... and {} more errors", errors.len() - 5));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_agents::mock::MockBackend;
    use ath_types::agent::{AgentKind, AgentResponse};
    use chrono::Utc;
    use uuid::Uuid;

    fn test_project() -> ProjectSpec {
        use ath_types::project::{GoalSpec, SkillTag};
        ProjectSpec {
            name: "test-app".into(),
            description: "A test application".into(),
            goals: vec![
                GoalSpec {
                    description: "Build the API".into(),
                    skill_tags: vec![SkillTag("rust".into())],
                },
                GoalSpec {
                    description: "Add auth".into(),
                    skill_tags: vec![SkillTag("security".into())],
                },
            ],
            constraints: vec![],
            target_language: Some("Rust".into()),
            target_framework: None,
            expected_files: vec!["src/main.rs".into()],
        }
    }

    /// Valid plan JSON: 3 phases in a linear chain, covering both goals.
    fn valid_plan_json() -> String {
        serde_json::json!({
            "phases": [
                {
                    "id": 1,
                    "name": "Foundation",
                    "description": "Set up project structure",
                    "tasks": [{
                        "name": "Init project",
                        "description": "Create project skeleton",
                        "skill_tags": ["rust"],
                        "expected_output_files": ["Cargo.toml"],
                        "acceptance_criteria": ["Project compiles"],
                        "goal_indices": [0]
                    }],
                    "depends_on": [],
                    "produces": ["project-structure"],
                    "consumes": []
                },
                {
                    "id": 2,
                    "name": "API Layer",
                    "description": "Build REST endpoints",
                    "tasks": [{
                        "name": "Create endpoints",
                        "description": "Implement CRUD API",
                        "skill_tags": ["rust", "api"],
                        "expected_output_files": ["src/api.rs"],
                        "acceptance_criteria": ["All endpoints return JSON"],
                        "goal_indices": [0]
                    }],
                    "depends_on": [1],
                    "produces": ["api-types"],
                    "consumes": ["project-structure"]
                },
                {
                    "id": 3,
                    "name": "Authentication",
                    "description": "Add auth layer",
                    "tasks": [{
                        "name": "Implement auth",
                        "description": "JWT authentication",
                        "skill_tags": ["security"],
                        "expected_output_files": ["src/auth.rs"],
                        "acceptance_criteria": ["Auth middleware works"],
                        "goal_indices": [1]
                    }],
                    "depends_on": [2],
                    "produces": ["auth-layer"],
                    "consumes": ["api-types"]
                }
            ]
        })
        .to_string()
    }

    /// Plan JSON with a circular dependency (1 -> 2 -> 3 -> 1).
    fn cyclic_plan_json() -> String {
        serde_json::json!({
            "phases": [
                {
                    "id": 1,
                    "name": "A",
                    "description": "Phase A",
                    "tasks": [{
                        "name": "Task A",
                        "description": "Do A",
                        "skill_tags": ["rust"],
                        "expected_output_files": [],
                        "acceptance_criteria": [],
                        "goal_indices": [0]
                    }],
                    "depends_on": [3],
                    "produces": [],
                    "consumes": []
                },
                {
                    "id": 2,
                    "name": "B",
                    "description": "Phase B",
                    "tasks": [{
                        "name": "Task B",
                        "description": "Do B",
                        "skill_tags": ["rust"],
                        "expected_output_files": [],
                        "acceptance_criteria": [],
                        "goal_indices": [0, 1]
                    }],
                    "depends_on": [1],
                    "produces": [],
                    "consumes": []
                },
                {
                    "id": 3,
                    "name": "C",
                    "description": "Phase C",
                    "tasks": [{
                        "name": "Task C",
                        "description": "Do C",
                        "skill_tags": ["rust"],
                        "expected_output_files": [],
                        "acceptance_criteria": [],
                        "goal_indices": [1]
                    }],
                    "depends_on": [2],
                    "produces": [],
                    "consumes": []
                }
            ]
        })
        .to_string()
    }

    fn make_response(content: &str) -> Result<AgentResponse, ath_agents::error::AgentError> {
        Ok(AgentResponse {
            request_id: Uuid::new_v4(),
            agent: AgentKind::Claude("mock".into()),
            content: content.to_string(),
            input_tokens: 0,
            output_tokens: 0,
            created_at: Utc::now(),
        })
    }

    #[tokio::test]
    async fn decompose_valid_response_returns_execution_plan() {
        let mock = MockBackend::always_ok(&valid_plan_json());
        let project = test_project();

        let result = decompose_project_spec(&project, &mock).await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());

        let (plan, warnings) = result.unwrap();
        assert_eq!(plan.phases.len(), 3);
        assert_eq!(plan.execution_order, vec![1, 2, 3]);
        assert_eq!(plan.parallel_groups.len(), 3); // Linear chain: 3 levels
        assert_eq!(plan.critical_path_length, 3);
        assert!(warnings.is_empty());
    }

    #[tokio::test]
    async fn decompose_retries_on_invalid_json_then_succeeds() {
        let mock = MockBackend::new(vec![
            make_response("this is not valid json"),
            make_response(&valid_plan_json()),
        ]);
        let project = test_project();

        let result = decompose_project_spec(&project, &mock).await;
        assert!(result.is_ok());

        let (plan, _) = result.unwrap();
        assert_eq!(plan.phases.len(), 3);
    }

    #[tokio::test]
    async fn decompose_retries_on_validation_error_then_succeeds() {
        // First response has a cycle, second is valid
        let mock = MockBackend::new(vec![
            make_response(&cyclic_plan_json()),
            make_response(&valid_plan_json()),
        ]);
        let project = test_project();

        let result = decompose_project_spec(&project, &mock).await;
        assert!(result.is_ok());

        let (plan, _) = result.unwrap();
        assert_eq!(plan.phases.len(), 3);
        assert_eq!(plan.execution_order, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn decompose_fails_after_3_attempts() {
        let mock = MockBackend::always_ok("not valid json");
        let project = test_project();

        let result = decompose_project_spec(&project, &mock).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            DecomposeError::DecomposeFailed {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3);
                assert!(last_error.is_some());
                assert!(last_error.unwrap().contains("JSON parse error"));
            }
            other => panic!("expected DecomposeFailed, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn decompose_returns_warnings_alongside_plan() {
        // Create a plan with 6 tasks in one phase (triggers LargePhase warning)
        let large_plan = serde_json::json!({
            "phases": [{
                "id": 1,
                "name": "Big Phase",
                "description": "A large phase",
                "tasks": [
                    { "name": "T1", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [0] },
                    { "name": "T2", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [0] },
                    { "name": "T3", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [0] },
                    { "name": "T4", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [1] },
                    { "name": "T5", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [1] },
                    { "name": "T6", "description": "d", "skill_tags": ["r"], "expected_output_files": [], "acceptance_criteria": [], "goal_indices": [1] }
                ],
                "depends_on": [],
                "produces": [],
                "consumes": []
            }]
        })
        .to_string();

        let mock = MockBackend::always_ok(&large_plan);
        let project = test_project();

        let result = decompose_project_spec(&project, &mock).await;
        assert!(result.is_ok());

        let (plan, warnings) = result.unwrap();
        assert_eq!(plan.phases.len(), 1);
        assert!(!warnings.is_empty());
        assert!(warnings
            .iter()
            .any(|w| matches!(w, PlanWarning::LargePhase { task_count: 6, .. })));
    }

    #[test]
    fn format_validation_errors_caps_at_5() {
        let errors: Vec<ValidationError> = (0..8)
            .map(|i| ValidationError::EmptyPhase {
                phase_id: i,
                phase_name: format!("Phase {}", i),
                hint: "add tasks".into(),
            })
            .collect();

        let formatted = format_validation_errors(&errors);
        let lines: Vec<&str> = formatted.lines().collect();
        assert_eq!(lines.len(), 6); // 5 errors + "... and 3 more"
        assert!(formatted.contains("and 3 more"));
    }
}
