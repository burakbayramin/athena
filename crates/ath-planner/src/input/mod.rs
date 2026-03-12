//! Input mode resolution and parsing infrastructure.
//!
//! Determines how the user wants to provide project input:
//! natural language description, spec file, or existing codebase.
//! Provides LLM-based parsing with retry logic for all input modes.

pub mod error;
pub mod prompt;
pub mod spec_file;

use std::path::{Path, PathBuf};

use ath_agents::backend::AgentBackend;
use ath_types::agent::AgentRequest;
use ath_types::project::ProjectSpec;
use serde::{Deserialize, Serialize};

pub use error::InputError;
pub use prompt::*;
pub use spec_file::read_spec_file;

/// Maximum number of LLM parsing attempts before giving up.
const MAX_PARSE_ATTEMPTS: usize = 3;

/// Discriminated union of the three input modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputMode {
    /// Natural language description from positional arg.
    NaturalLanguage(String),
    /// Markdown spec file path.
    SpecFile(PathBuf),
    /// Codebase directory with optional intent description.
    Codebase {
        path: PathBuf,
        intent: Option<String>,
    },
}

/// Resolve which input mode the user intended based on CLI arguments.
///
/// Rules (per locked decisions):
/// - If spec is Some and (description is Some OR codebase is Some) -> Err(SpecExclusive)
/// - If spec is Some -> Ok(SpecFile)
/// - If codebase is Some and description is Some -> Ok(Codebase { intent: Some(desc) })
/// - If codebase is Some and description is None -> Err(CodebaseWithoutIntent)
/// - If description is Some -> Ok(NaturalLanguage)
/// - None/None/None -> Err(NoInput)
pub fn resolve_input_mode(
    description: Option<&str>,
    spec: Option<&Path>,
    codebase: Option<&Path>,
) -> Result<InputMode, InputError> {
    // --spec is exclusive
    if let Some(spec_path) = spec {
        if description.is_some() || codebase.is_some() {
            return Err(InputError::SpecExclusive {
                hint: "Use --spec alone without a description or --codebase".into(),
            });
        }
        return Ok(InputMode::SpecFile(spec_path.to_path_buf()));
    }

    // --codebase requires a description
    if let Some(codebase_path) = codebase {
        if let Some(desc) = description {
            return Ok(InputMode::Codebase {
                path: codebase_path.to_path_buf(),
                intent: Some(desc.to_string()),
            });
        }
        return Err(InputError::CodebaseWithoutIntent {
            hint: "Provide a description: ath run --codebase ./path 'your description'".into(),
        });
    }

    // Natural language description
    if let Some(desc) = description {
        return Ok(InputMode::NaturalLanguage(desc.to_string()));
    }

    // No input at all
    Err(InputError::NoInput {
        hint: "Provide a description, --spec file, or --codebase path. See `ath run --help`."
            .into(),
    })
}

/// Parse input into a ProjectSpec via LLM with retry logic.
///
/// The `request_builder` closure constructs an `AgentRequest` for each attempt,
/// receiving the last error message (if any) for retry feedback.
///
/// Attempts up to 3 times:
/// 1. Sends the request to the backend
/// 2. Parses the response content as ProjectSpec JSON
/// 3. Validates the parsed spec
/// 4. On failure, retries with the error appended to the prompt
pub async fn parse_to_project_spec(
    backend: &dyn AgentBackend,
    request_builder: impl Fn(Option<&str>) -> AgentRequest,
) -> Result<ProjectSpec, InputError> {
    let mut last_error: Option<String> = None;

    for _attempt in 0..MAX_PARSE_ATTEMPTS {
        let request = request_builder(last_error.as_deref());
        let response = backend.send(request).await.map_err(InputError::Agent)?;

        // Try to parse as ProjectSpec
        match serde_json::from_str::<ProjectSpec>(&response.content) {
            Ok(spec) => {
                // Validate the parsed spec
                match spec.validate() {
                    Ok(()) => return Ok(spec),
                    Err(e) => {
                        last_error = Some(format!(
                            "Validation failed: {}. Hint: {}",
                            e,
                            e.hint()
                        ));
                    }
                }
            }
            Err(e) => {
                last_error = Some(format!("JSON parse error: {}", e));
            }
        }
    }

    Err(InputError::ParseFailed {
        attempts: MAX_PARSE_ATTEMPTS,
        last_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_agents::mock::MockBackend;
    use ath_types::agent::{AgentKind, AgentResponse};
    use chrono::Utc;
    use std::path::Path;
    use uuid::Uuid;

    /// Helper: valid ProjectSpec JSON string.
    fn valid_project_spec_json() -> String {
        serde_json::json!({
            "name": "todo-app",
            "description": "A REST API todo application",
            "goals": [{
                "description": "CRUD endpoints",
                "skill_tags": ["rust", "api"]
            }],
            "constraints": ["Must use PostgreSQL"],
            "target_language": "Rust",
            "target_framework": "Axum",
            "expected_files": ["src/main.rs"]
        })
        .to_string()
    }

    /// Helper: valid ProjectSpec JSON but with empty name (fails validation).
    fn invalid_name_project_spec_json() -> String {
        serde_json::json!({
            "name": "",
            "description": "A todo app",
            "goals": [{
                "description": "CRUD endpoints",
                "skill_tags": ["rust"]
            }],
            "constraints": [],
            "target_language": null,
            "target_framework": null,
            "expected_files": ["src/main.rs"]
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

    fn simple_request_builder(last_error: Option<&str>) -> AgentRequest {
        build_natural_language_request("build a todo app", last_error)
    }

    // --- resolve_input_mode tests (from 04-01) ---

    #[test]
    fn natural_language_round_trip() {
        let mode = InputMode::NaturalLanguage("build a todo app".into());
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: InputMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn spec_file_round_trip() {
        let mode = InputMode::SpecFile(PathBuf::from("spec.md"));
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: InputMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn codebase_round_trip() {
        let mode = InputMode::Codebase {
            path: PathBuf::from("./project"),
            intent: Some("add auth".into()),
        };
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: InputMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn resolve_natural_language() {
        let result = resolve_input_mode(Some("desc"), None, None);
        assert!(matches!(result, Ok(InputMode::NaturalLanguage(ref s)) if s == "desc"));
    }

    #[test]
    fn resolve_spec_file() {
        let spec = Path::new("spec.md");
        let result = resolve_input_mode(None, Some(spec), None);
        assert!(matches!(result, Ok(InputMode::SpecFile(ref p)) if p == spec));
    }

    #[test]
    fn resolve_codebase_without_description_errors() {
        let codebase = Path::new("./proj");
        let result = resolve_input_mode(None, None, Some(codebase));
        assert!(matches!(result, Err(InputError::CodebaseWithoutIntent { .. })));
    }

    #[test]
    fn resolve_codebase_with_description() {
        let codebase = Path::new("./proj");
        let result = resolve_input_mode(Some("desc"), None, Some(codebase));
        assert!(matches!(
            result,
            Ok(InputMode::Codebase { ref path, ref intent })
            if path == codebase && intent.as_deref() == Some("desc")
        ));
    }

    #[test]
    fn resolve_spec_with_description_errors() {
        let spec = Path::new("spec.md");
        let result = resolve_input_mode(Some("desc"), Some(spec), None);
        assert!(matches!(result, Err(InputError::SpecExclusive { .. })));
    }

    #[test]
    fn resolve_spec_with_codebase_errors() {
        let spec = Path::new("spec.md");
        let codebase = Path::new("./proj");
        let result = resolve_input_mode(None, Some(spec), Some(codebase));
        assert!(matches!(result, Err(InputError::SpecExclusive { .. })));
    }

    #[test]
    fn resolve_no_input_errors() {
        let result = resolve_input_mode(None, None, None);
        assert!(matches!(result, Err(InputError::NoInput { .. })));
    }

    // --- parse_to_project_spec tests ---

    #[tokio::test]
    async fn parse_succeeds_on_first_try_with_valid_json() {
        let mock = MockBackend::always_ok(&valid_project_spec_json());
        let result = parse_to_project_spec(&mock, simple_request_builder).await;
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.name, "todo-app");
        assert_eq!(spec.goals.len(), 1);
    }

    #[tokio::test]
    async fn parse_retries_on_invalid_json_then_succeeds() {
        let mock = MockBackend::new(vec![
            make_response("this is not valid json"),
            make_response(&valid_project_spec_json()),
        ]);
        let result = parse_to_project_spec(&mock, simple_request_builder).await;
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.name, "todo-app");
    }

    #[tokio::test]
    async fn parse_fails_after_3_attempts_with_always_invalid_json() {
        // MockBackend::always_ok returns the same (invalid) content every time
        let mock = MockBackend::always_ok("not json at all");
        let result = parse_to_project_spec(&mock, simple_request_builder).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            InputError::ParseFailed {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3);
                assert!(last_error.is_some());
                assert!(last_error.unwrap().contains("JSON parse error"));
            }
            other => panic!("expected ParseFailed, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn parse_retries_on_validation_failure_then_succeeds() {
        let mock = MockBackend::new(vec![
            make_response(&invalid_name_project_spec_json()),
            make_response(&valid_project_spec_json()),
        ]);
        let result = parse_to_project_spec(&mock, simple_request_builder).await;
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.name, "todo-app");
    }
}
