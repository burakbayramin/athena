//! LLM prompt templates and request builders for ProjectSpec extraction.
//!
//! Provides system prompts, JSON schema generation, and AgentRequest builders
//! for natural language, spec file, and codebase input modes.

use ath_types::agent::{AgentId, AgentRequest};
use chrono::Utc;
use uuid::Uuid;

/// System prompt for natural language ProjectSpec extraction.
pub const SYSTEM_PROMPT: &str = r#"You are a project specification parser. Given a user's project description, extract a structured ProjectSpec.

Rules:
- name: Derive a short kebab-case project name from the description
- description: Expand the user's intent into a clear 1-2 sentence description
- goals: Break the project into 3-8 concrete goals, each with relevant skill_tags (plain strings like "rust", "api", "frontend")
- constraints: Extract any explicit constraints; if none stated, leave empty array
- target_language: Infer when obvious (e.g., "React app" -> "TypeScript"); null when ambiguous
- target_framework: Infer when obvious; null when ambiguous
- expected_files: List 3-10 key files the project would produce

Respond with valid JSON matching the ProjectSpec schema."#;

/// System prompt for spec file ProjectSpec extraction.
pub const SPEC_FILE_SYSTEM_PROMPT: &str = r#"You are reading a project specification document. Extract the ProjectSpec from this markdown document.

Rules:
- name: Derive a short kebab-case project name from the document
- description: Summarize the project in 1-2 sentences
- goals: Extract 3-8 concrete goals with relevant skill_tags (plain strings)
- constraints: Extract any explicit constraints; if none stated, leave empty array
- target_language: Infer when obvious; null when ambiguous
- target_framework: Infer when obvious; null when ambiguous
- expected_files: List 3-10 key files the project would produce

Respond with valid JSON matching the ProjectSpec schema."#;

/// System prompt for codebase analysis ProjectSpec extraction.
pub const CODEBASE_SYSTEM_PROMPT: &str = r#"You are analyzing an existing codebase. Based on the file tree and key file contents, extract a ProjectSpec describing what to build next.

Rules:
- name: Derive a short kebab-case project name from the codebase
- description: Describe the next development phase in 1-2 sentences
- goals: Identify 3-8 concrete next-step goals with relevant skill_tags (plain strings)
- constraints: Extract any constraints from the codebase (e.g., existing tech choices); leave empty if none
- target_language: Infer from the codebase's primary language
- target_framework: Infer from the codebase's framework usage; null if none
- expected_files: List 3-10 key files that would be created or modified

Respond with valid JSON matching the ProjectSpec schema."#;

/// Returns the JSON schema for ProjectSpec structured output.
///
/// This schema is passed as `AgentRequest.json_schema` and used by genai's
/// `ChatResponseFormat::JsonSpec` to enforce structured output at the provider level.
pub fn project_spec_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "description": { "type": "string" },
            "goals": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string" },
                        "skill_tags": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["description", "skill_tags"]
                }
            },
            "constraints": { "type": "array", "items": { "type": "string" } },
            "target_language": { "type": ["string", "null"] },
            "target_framework": { "type": ["string", "null"] },
            "expected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name", "description", "goals", "constraints", "expected_files"]
    })
}

/// Build an AgentRequest for natural language ProjectSpec extraction.
///
/// If `last_error` is Some, appends error feedback to the prompt for retry.
pub fn build_natural_language_request(description: &str, last_error: Option<&str>) -> AgentRequest {
    let mut prompt = description.to_string();
    if let Some(err) = last_error {
        prompt.push_str(&format!(
            "\n\n[Previous attempt failed: {}. Please fix and try again.]",
            err
        ));
    }

    AgentRequest {
        id: Uuid::new_v4(),
        agent: AgentId::claude("opus-4"),
        prompt,
        context: Some(SYSTEM_PROMPT.to_string()),
        json_schema: Some(project_spec_json_schema()),
        created_at: Utc::now(),
    }
}

/// Build an AgentRequest for spec file ProjectSpec extraction.
///
/// If `last_error` is Some, appends error feedback to the prompt for retry.
pub fn build_spec_file_request(content: &str, last_error: Option<&str>) -> AgentRequest {
    let mut prompt = content.to_string();
    if let Some(err) = last_error {
        prompt.push_str(&format!(
            "\n\n[Previous attempt failed: {}. Please fix and try again.]",
            err
        ));
    }

    AgentRequest {
        id: Uuid::new_v4(),
        agent: AgentId::claude("opus-4"),
        prompt,
        context: Some(SPEC_FILE_SYSTEM_PROMPT.to_string()),
        json_schema: Some(project_spec_json_schema()),
        created_at: Utc::now(),
    }
}

/// Build an AgentRequest for codebase analysis ProjectSpec extraction.
///
/// If `last_error` is Some, appends error feedback to the prompt for retry.
pub fn build_codebase_request(
    tree: &str,
    key_files: &[(String, String)],
    intent: Option<&str>,
    last_error: Option<&str>,
) -> AgentRequest {
    let mut prompt = format!("## File Tree\n\n```\n{}\n```\n", tree);

    if !key_files.is_empty() {
        prompt.push_str("\n## Key Files\n\n");
        for (path, content) in key_files {
            prompt.push_str(&format!("### {}\n\n```\n{}\n```\n\n", path, content));
        }
    }

    if let Some(intent_str) = intent {
        prompt.push_str(&format!("\n## User Intent\n\n{}\n", intent_str));
    }

    if let Some(err) = last_error {
        prompt.push_str(&format!(
            "\n\n[Previous attempt failed: {}. Please fix and try again.]",
            err
        ));
    }

    AgentRequest {
        id: Uuid::new_v4(),
        agent: AgentId::claude("opus-4"),
        prompt,
        context: Some(CODEBASE_SYSTEM_PROMPT.to_string()),
        json_schema: Some(project_spec_json_schema()),
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_natural_language_request_includes_description() {
        let req = build_natural_language_request("build a todo app", None);
        assert!(req.prompt.contains("build a todo app"));
        assert_eq!(req.context.as_deref(), Some(SYSTEM_PROMPT));
        assert!(req.json_schema.is_some());
    }

    #[test]
    fn build_natural_language_request_includes_error_on_retry() {
        let req = build_natural_language_request("build a todo app", Some("invalid JSON"));
        assert!(req.prompt.contains("build a todo app"));
        assert!(req.prompt.contains("invalid JSON"));
    }

    #[test]
    fn build_spec_file_request_includes_content() {
        let req = build_spec_file_request("# My Project\n\nSome spec content", None);
        assert!(req.prompt.contains("My Project"));
        assert!(req.prompt.contains("Some spec content"));
        assert_eq!(req.context.as_deref(), Some(SPEC_FILE_SYSTEM_PROMPT));
        assert!(req.json_schema.is_some());
    }

    #[test]
    fn build_codebase_request_includes_tree_and_files() {
        let tree = "src/\n  main.rs\n  lib.rs";
        let key_files = vec![(
            "Cargo.toml".to_string(),
            "[package]\nname = \"test\"".to_string(),
        )];
        let req = build_codebase_request(tree, &key_files, Some("add auth"), None);
        assert!(req.prompt.contains("main.rs"));
        assert!(req.prompt.contains("Cargo.toml"));
        assert!(req.prompt.contains("add auth"));
        assert_eq!(req.context.as_deref(), Some(CODEBASE_SYSTEM_PROMPT));
    }

    #[test]
    fn project_spec_json_schema_is_valid_json_object() {
        let schema = project_spec_json_schema();
        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "object");

        // Check required fields are present in properties
        let props = obj.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("description"));
        assert!(props.contains_key("goals"));
        assert!(props.contains_key("constraints"));
        assert!(props.contains_key("target_language"));
        assert!(props.contains_key("target_framework"));
        assert!(props.contains_key("expected_files"));

        // Check required array
        let required = obj.get("required").unwrap().as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"name"));
        assert!(required_strs.contains(&"description"));
        assert!(required_strs.contains(&"goals"));
    }
}
