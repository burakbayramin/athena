//! ReviewEngine: reviewer selection with cross-agent pairing, review prompt
//! construction, and structured verdict parsing.
//!
//! Implements QUAL-01 (cross-agent review where reviewer != author). The reviewer
//! selection logic enforces the never-same-as-author rule, handles circuit breaker
//! fallback, and uses priority tiebreaking (Claude > Gemini > Codex).

use std::collections::HashMap;

use ath_types::agent::AgentId;
use ath_types::plan::TaskSpec;
use ath_types::review::{CodeSuggestion, ReviewVerdict, Severity};
use serde::Deserialize;

use crate::phase_runner::TaskOutput;

/// Errors specific to the review subsystem.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// No eligible reviewer is available (all non-author agents are down).
    #[error("no reviewer available for phase '{phase_name}': {reason}")]
    NoReviewerAvailable { phase_name: String, reason: String },

    /// Failed to parse the review verdict JSON from the reviewer agent.
    #[error("verdict parse failed: {reason}\nraw: {raw}")]
    VerdictParseFailed { raw: String, reason: String },
}

/// All three agent kinds in priority order (Claude > Gemini > Codex).
const CANDIDATE_ORDER: [fn() -> AgentId; 3] = [
    || AgentId::claude("opus-4"),
    || AgentId::gemini("2.5-pro"),
    || AgentId::codex("o3"),
];

/// Selects a reviewer agent that is different from the majority author.
///
/// Algorithm:
/// 1. Count tasks per agent discriminant.
/// 2. Find the majority author (ties broken by priority: Claude > Gemini > Codex).
/// 3. Iterate candidates in priority order, skip majority author.
/// 4. Return the first available candidate.
/// 5. If none available, return `NoReviewerAvailable`.
pub fn select_reviewer(
    task_agents: &[AgentId],
    phase_name: &str,
    available: impl Fn(&AgentId) -> bool,
) -> Result<AgentId, ReviewError> {
    if task_agents.is_empty() {
        return Err(ReviewError::NoReviewerAvailable {
            phase_name: phase_name.into(),
            reason: "no tasks in phase".into(),
        });
    }

    // Count tasks per provider, track priority index for tiebreaking
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
    for agent in task_agents {
        let provider = agent.provider().to_string();
        let priority = priority_index(agent);
        counts
            .entry(provider)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, priority));
    }

    // Find majority author: highest count, then lowest priority index (= highest priority) for ties
    let majority_provider = counts
        .iter()
        .max_by(|a, b| {
            a.1 .0
                .cmp(&b.1 .0) // higher count wins
                .then(b.1 .1.cmp(&a.1 .1)) // lower priority index wins (= higher priority)
        })
        .map(|(provider, _)| provider.clone());

    // Iterate candidates in priority order, skip majority author
    for make_candidate in &CANDIDATE_ORDER {
        let candidate = make_candidate();
        if Some(candidate.provider().to_string()) == majority_provider {
            continue;
        }
        if available(&candidate) {
            return Ok(candidate);
        }
    }

    Err(ReviewError::NoReviewerAvailable {
        phase_name: phase_name.into(),
        reason: "all non-author agents are unavailable".into(),
    })
}

/// Builds a review prompt containing all task outputs for a phase.
///
/// The reviewer receives the full context: original task spec, all produced files
/// with contents, and the agent's explanation of what it did.
pub fn build_review_prompt(phase_name: &str, tasks: &[TaskSpec], outputs: &[TaskOutput]) -> String {
    let mut prompt = format!("## Phase Review: {phase_name}\n\n");

    for (task, output) in tasks.iter().zip(outputs.iter()) {
        prompt.push_str(&format!("### Task: {}\n", task.name));
        prompt.push_str(&format!("**Description:** {}\n", task.description));
        prompt.push_str(&format!("**Agent:** {}\n\n", output.agent.provider_name()));

        prompt.push_str("**Files Produced:**\n");
        for file in &output.files_produced {
            prompt.push_str(&format!("#### {}\n```\n{}\n```\n", file.path, file.content));
        }

        prompt.push_str(&format!("**Explanation:** {}\n\n", output.explanation));
    }

    prompt
}

/// Returns a JSON schema describing the expected review verdict structure.
///
/// Required fields: passed, severity, reason, suggestions.
pub fn review_verdict_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "passed": { "type": "boolean" },
            "severity": { "type": "string", "enum": ["critical", "warning", "info"] },
            "reason": { "type": "string" },
            "suggestions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string" },
                        "line": { "type": ["integer", "null"] },
                        "suggestion": { "type": "string" }
                    },
                    "required": ["file", "suggestion"]
                }
            }
        },
        "required": ["passed", "severity", "reason", "suggestions"]
    })
}

/// Intermediate deserialization struct for parsing review verdicts from JSON.
#[derive(Debug, Deserialize)]
struct RawVerdict {
    passed: bool,
    severity: String,
    reason: String,
    #[serde(default)]
    suggestions: Vec<RawSuggestion>,
}

#[derive(Debug, Deserialize)]
struct RawSuggestion {
    file: String,
    line: Option<u32>,
    suggestion: String,
}

/// Parses a review verdict from JSON, associating it with the given reviewer.
///
/// Severity is parsed case-insensitively. Missing optional `suggestions` defaults
/// to an empty list.
pub fn parse_review_verdict(json: &str, reviewer: AgentId) -> Result<ReviewVerdict, ReviewError> {
    let raw: RawVerdict =
        serde_json::from_str(json).map_err(|e| ReviewError::VerdictParseFailed {
            raw: json.to_string(),
            reason: e.to_string(),
        })?;

    let severity = match raw.severity.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        other => {
            return Err(ReviewError::VerdictParseFailed {
                raw: json.to_string(),
                reason: format!("unknown severity: {other}"),
            });
        }
    };

    let suggestions = raw
        .suggestions
        .into_iter()
        .map(|s| CodeSuggestion {
            file: s.file,
            line: s.line,
            suggestion: s.suggestion,
        })
        .collect();

    Ok(ReviewVerdict {
        passed: raw.passed,
        reviewer,
        severity,
        reason: raw.reason,
        suggestions,
    })
}

/// Builds a retry prompt that includes the original task spec and review feedback.
///
/// Feedback is structured as a "Previous Review Feedback" section with:
/// - Result line: "FAILED ({severity})"
/// - Reason: truncated to 500 chars if longer
/// - Suggestions: max 5, formatted with file and optional line number
pub fn build_retry_prompt(task: &TaskSpec, feedback: &ReviewVerdict) -> String {
    let mut prompt = format!(
        "## Task\n{}\n\n## Description\n{}\n",
        task.name, task.description
    );

    prompt.push_str(&format_retry_feedback_context(feedback));

    prompt.push_str("\nPlease address the feedback above and produce corrected output.\n");
    prompt
}

/// Formats the retry feedback section reused by retry prompts and verbose transcript capture.
pub fn format_retry_feedback_context(feedback: &ReviewVerdict) -> String {
    let mut section = "\n## Previous Review Feedback\n".to_string();

    let severity_str = match feedback.severity {
        Severity::Critical => "Critical",
        Severity::Warning => "Warning",
        Severity::Info => "Info",
    };
    section.push_str(&format!("**Result:** FAILED ({severity_str})\n"));

    let reason = if feedback.reason.len() > 500 {
        let mut truncated = feedback.reason[..500].to_string();
        truncated.push_str("...");
        truncated
    } else {
        feedback.reason.clone()
    };
    section.push_str(&format!("**Reason:** {reason}\n"));

    let suggestions: Vec<&CodeSuggestion> = feedback.suggestions.iter().take(5).collect();
    if !suggestions.is_empty() {
        section.push_str("\n**Suggestions:**\n");
        for suggestion in suggestions {
            if let Some(line) = suggestion.line {
                section.push_str(&format!(
                    "- `{}` line {}: {}\n",
                    suggestion.file, line, suggestion.suggestion
                ));
            } else {
                section.push_str(&format!(
                    "- `{}`: {}\n",
                    suggestion.file, suggestion.suggestion
                ));
            }
        }
    }

    section
}

/// Returns the priority index for an agent (lower = higher priority).
fn priority_index(agent: &AgentId) -> usize {
    if agent.is_claude() {
        0
    } else if agent.is_gemini() {
        1
    } else if agent.is_codex() {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase_runner::FileOutput;
    use ath_types::project::SkillTag;

    fn always_available(_: &AgentId) -> bool {
        true
    }

    fn none_available(_: &AgentId) -> bool {
        false
    }

    fn exclude(kind_fn: fn() -> AgentId) -> impl Fn(&AgentId) -> bool {
        move |agent: &AgentId| agent.provider() != kind_fn().provider()
    }

    fn sample_task(name: &str, desc: &str) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: desc.into(),
            skill_tags: vec![SkillTag("rust".into())],
            expected_output_files: vec![],
            acceptance_criteria: vec![],
            goal_indices: vec![],
            assigned_agent: None,
        }
    }

    fn sample_output(task_name: &str, agent: AgentId) -> TaskOutput {
        TaskOutput {
            task_name: task_name.into(),
            agent,
            files_produced: vec![FileOutput {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            }],
            explanation: "Implemented the main function.".into(),
            issues_encountered: vec![],
            input_tokens: 100,
            output_tokens: 50,
        }
    }

    // ==================== select_reviewer tests ====================

    // Test 1: Single Claude-authored phase selects Gemini as reviewer
    #[test]
    fn single_claude_author_selects_gemini() {
        let agents = vec![AgentId::claude("opus-4")];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(reviewer.is_gemini());
    }

    // Test 2: Single Gemini-authored phase selects Claude as reviewer
    #[test]
    fn single_gemini_author_selects_claude() {
        let agents = vec![AgentId::gemini("2.5-pro")];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(reviewer.is_claude());
    }

    // Test 3: Multi-author phase (2 Claude, 1 Gemini) excludes Claude, selects Gemini
    #[test]
    fn majority_claude_selects_gemini() {
        let agents = vec![
            AgentId::claude("opus-4"),
            AgentId::claude("opus-4"),
            AgentId::gemini("2.5-pro"),
        ];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(reviewer.is_gemini());
    }

    // Test 4: Tie (1 Claude, 1 Gemini) excludes Claude (higher priority = majority tiebreak), selects Gemini
    #[test]
    fn tie_excludes_higher_priority_selects_next() {
        let agents = vec![AgentId::claude("opus-4"), AgentId::gemini("2.5-pro")];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(reviewer.is_gemini());
    }

    // Test 5: Preferred reviewer circuit breaker tripped, falls back to next
    #[test]
    fn fallback_when_preferred_unavailable() {
        let agents = vec![AgentId::gemini("2.5-pro")];
        let reviewer =
            select_reviewer(&agents, "test-phase", exclude(|| AgentId::claude(""))).unwrap();
        assert!(reviewer.is_codex());
    }

    // Test 6: ALL non-author agents unavailable -> error
    #[test]
    fn error_when_all_non_author_unavailable() {
        let agents = vec![AgentId::claude("opus-4")];
        let result = select_reviewer(&agents, "my-phase", none_available);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ReviewError::NoReviewerAvailable { .. }));
        let msg = err.to_string();
        assert!(msg.contains("my-phase"));
    }

    // Test 7: Codex-only phase with Claude unavailable selects Gemini
    #[test]
    fn codex_only_claude_unavailable_selects_gemini() {
        let agents = vec![AgentId::codex("o3")];
        let reviewer =
            select_reviewer(&agents, "test-phase", exclude(|| AgentId::claude(""))).unwrap();
        assert!(reviewer.is_gemini());
    }

    // ==================== build_review_prompt tests ====================

    // Test 1: build_review_prompt includes task name, description, and file contents
    #[test]
    fn review_prompt_includes_task_and_files() {
        let task = sample_task("Build API", "Create REST endpoints");
        let output = sample_output("Build API", AgentId::claude("opus-4"));

        let prompt = build_review_prompt("api-phase", &[task], &[output]);

        assert!(prompt.contains("## Phase Review: api-phase"));
        assert!(prompt.contains("### Task: Build API"));
        assert!(prompt.contains("**Description:** Create REST endpoints"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("fn main() {}"));
        assert!(prompt.contains("**Explanation:** Implemented the main function."));
    }

    // Test 2: build_review_prompt with multiple tasks includes all task outputs
    #[test]
    fn review_prompt_multiple_tasks() {
        let tasks = vec![
            sample_task("Task A", "First task"),
            sample_task("Task B", "Second task"),
        ];
        let outputs = vec![
            sample_output("Task A", AgentId::claude("opus-4")),
            TaskOutput {
                task_name: "Task B".into(),
                agent: AgentId::gemini("2.5-pro"),
                files_produced: vec![FileOutput {
                    path: "src/lib.rs".into(),
                    content: "pub fn lib() {}".into(),
                }],
                explanation: "Built the library.".into(),
                issues_encountered: vec![],
                input_tokens: 200,
                output_tokens: 100,
            },
        ];

        let prompt = build_review_prompt("multi-phase", &tasks, &outputs);

        assert!(prompt.contains("### Task: Task A"));
        assert!(prompt.contains("### Task: Task B"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("src/lib.rs"));
        assert!(prompt.contains("pub fn lib() {}"));
    }

    // ==================== parse_review_verdict tests ====================

    // Test 3: parse_review_verdict deserializes valid JSON into ReviewVerdict
    #[test]
    fn parse_valid_verdict() {
        let json = r#"{
            "passed": false,
            "severity": "critical",
            "reason": "Security vulnerability found",
            "suggestions": [
                {"file": "src/auth.rs", "line": 42, "suggestion": "Use bcrypt"}
            ]
        }"#;

        let verdict = parse_review_verdict(json, AgentId::gemini("2.5-pro")).unwrap();

        assert!(!verdict.passed);
        assert_eq!(verdict.severity, Severity::Critical);
        assert_eq!(verdict.reason, "Security vulnerability found");
        assert!(verdict.reviewer.is_gemini());
        assert_eq!(verdict.suggestions.len(), 1);
        assert_eq!(verdict.suggestions[0].file, "src/auth.rs");
        assert_eq!(verdict.suggestions[0].line, Some(42));
    }

    // Test 4: parse_review_verdict returns error on malformed JSON
    #[test]
    fn parse_malformed_json_returns_error() {
        let result = parse_review_verdict("not json", AgentId::claude("opus-4"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ReviewError::VerdictParseFailed { .. }));
        assert!(err.to_string().contains("not json"));
    }

    // Test 5: parse_review_verdict handles missing optional fields gracefully
    #[test]
    fn parse_verdict_missing_suggestions_defaults_empty() {
        let json = r#"{
            "passed": true,
            "severity": "info",
            "reason": "Looks good"
        }"#;

        let verdict = parse_review_verdict(json, AgentId::claude("opus-4")).unwrap();

        assert!(verdict.passed);
        assert_eq!(verdict.severity, Severity::Info);
        assert!(verdict.suggestions.is_empty());
    }

    // ==================== review_verdict_schema tests ====================

    // Test 6: review_verdict_schema returns valid JSON schema with required fields
    #[test]
    fn verdict_schema_has_required_fields() {
        let schema = review_verdict_schema();

        assert_eq!(schema["type"], "object");
        let required = schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"passed"));
        assert!(required_strs.contains(&"severity"));
        assert!(required_strs.contains(&"reason"));
        assert!(required_strs.contains(&"suggestions"));
    }

    // ==================== build_retry_prompt tests ====================

    // Test 7: build_retry_prompt includes original task + feedback reason + suggestions (capped at 5)
    #[test]
    fn retry_prompt_includes_feedback_capped_at_5() {
        let task = sample_task("Fix Auth", "Fix the authentication module");
        let feedback = ReviewVerdict {
            passed: false,
            reviewer: AgentId::gemini("2.5-pro"),
            severity: Severity::Critical,
            reason: "Multiple issues found".into(),
            suggestions: (0..7)
                .map(|i| CodeSuggestion {
                    file: format!("file{i}.rs"),
                    line: Some(i as u32),
                    suggestion: format!("Fix issue {i}"),
                })
                .collect(),
        };

        let prompt = build_retry_prompt(&task, &feedback);

        assert!(prompt.contains("## Task\nFix Auth"));
        assert!(prompt.contains("## Description\nFix the authentication module"));
        assert!(prompt.contains("## Previous Review Feedback"));
        assert!(prompt.contains("FAILED (Critical)"));
        assert!(prompt.contains("Multiple issues found"));
        // Should have exactly 5 suggestions (capped)
        assert!(prompt.contains("file4.rs"));
        assert!(!prompt.contains("file5.rs"));
        assert!(!prompt.contains("file6.rs"));
        assert!(prompt.contains("Please address the feedback above"));
    }

    // Test 8: build_retry_prompt truncates reason to 500 chars if longer
    #[test]
    fn retry_prompt_truncates_long_reason() {
        let task = sample_task("Task", "Description");
        let long_reason = "x".repeat(600);
        let feedback = ReviewVerdict {
            passed: false,
            reviewer: AgentId::claude("opus-4"),
            severity: Severity::Warning,
            reason: long_reason,
            suggestions: vec![],
        };

        let prompt = build_retry_prompt(&task, &feedback);

        // Should contain truncated reason (500 chars + "...")
        assert!(prompt.contains(&"x".repeat(500)));
        assert!(prompt.contains("..."));
        // Should NOT contain the full 600-char reason
        assert!(!prompt.contains(&"x".repeat(501)));
    }

    // Test: severity is case-insensitive in parse_review_verdict
    #[test]
    fn parse_verdict_case_insensitive_severity() {
        let json = r#"{"passed": true, "severity": "WARNING", "reason": "ok", "suggestions": []}"#;
        let verdict = parse_review_verdict(json, AgentId::claude("opus-4")).unwrap();
        assert_eq!(verdict.severity, Severity::Warning);
    }

    // Test: build_retry_prompt with suggestion without line number
    #[test]
    fn retry_prompt_suggestion_without_line() {
        let task = sample_task("Task", "Desc");
        let feedback = ReviewVerdict {
            passed: false,
            reviewer: AgentId::claude("opus-4"),
            severity: Severity::Info,
            reason: "Minor issue".into(),
            suggestions: vec![CodeSuggestion {
                file: "src/lib.rs".into(),
                line: None,
                suggestion: "Add docs".into(),
            }],
        };

        let prompt = build_retry_prompt(&task, &feedback);

        assert!(prompt.contains("- `src/lib.rs`: Add docs"));
    }
}
