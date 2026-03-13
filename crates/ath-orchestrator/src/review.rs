//! ReviewEngine: reviewer selection with cross-agent pairing, review prompt
//! construction, and structured verdict parsing.
//!
//! Implements QUAL-01 (cross-agent review where reviewer != author). The reviewer
//! selection logic enforces the never-same-as-author rule, handles circuit breaker
//! fallback, and uses priority tiebreaking (Claude > Gemini > Codex).

use std::collections::HashMap;
use std::mem;

use ath_types::agent::AgentKind;
use ath_types::plan::TaskSpec;
use ath_types::review::{CodeSuggestion, ReviewVerdict, Severity};
use serde::Deserialize;

use crate::phase_runner::TaskOutput;

/// Errors specific to the review subsystem.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// No eligible reviewer is available (all non-author agents are down).
    #[error("no reviewer available for phase '{phase_name}': {reason}")]
    NoReviewerAvailable {
        phase_name: String,
        reason: String,
    },

    /// Failed to parse the review verdict JSON from the reviewer agent.
    #[error("verdict parse failed: {reason}\nraw: {raw}")]
    VerdictParseFailed {
        raw: String,
        reason: String,
    },
}

/// All three agent kinds in priority order (Claude > Gemini > Codex).
const CANDIDATE_ORDER: [fn() -> AgentKind; 3] = [
    || AgentKind::Claude("opus-4".into()),
    || AgentKind::Gemini("2.5-pro".into()),
    || AgentKind::Codex("o3".into()),
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
    task_agents: &[AgentKind],
    phase_name: &str,
    available: impl Fn(&AgentKind) -> bool,
) -> Result<AgentKind, ReviewError> {
    if task_agents.is_empty() {
        return Err(ReviewError::NoReviewerAvailable {
            phase_name: phase_name.into(),
            reason: "no tasks in phase".into(),
        });
    }

    // Count tasks per agent discriminant, track priority index for tiebreaking
    let mut counts: HashMap<mem::Discriminant<AgentKind>, (usize, usize)> = HashMap::new();
    for agent in task_agents {
        let disc = mem::discriminant(agent);
        let priority = priority_index(agent);
        counts
            .entry(disc)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, priority));
    }

    // Find majority author: highest count, then lowest priority index (= highest priority) for ties
    let majority_disc = counts
        .iter()
        .max_by(|a, b| {
            a.1 .0
                .cmp(&b.1 .0) // higher count wins
                .then(b.1 .1.cmp(&a.1 .1)) // lower priority index wins (= higher priority)
        })
        .map(|(disc, _)| *disc);

    // Iterate candidates in priority order, skip majority author
    for make_candidate in &CANDIDATE_ORDER {
        let candidate = make_candidate();
        let disc = mem::discriminant(&candidate);
        if Some(disc) == majority_disc {
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
pub fn build_review_prompt(
    phase_name: &str,
    tasks: &[TaskSpec],
    outputs: &[TaskOutput],
) -> String {
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
pub fn parse_review_verdict(
    json: &str,
    reviewer: AgentKind,
) -> Result<ReviewVerdict, ReviewError> {
    let raw: RawVerdict = serde_json::from_str(json).map_err(|e| ReviewError::VerdictParseFailed {
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

    prompt.push_str("\n## Previous Review Feedback\n");

    let severity_str = match feedback.severity {
        Severity::Critical => "Critical",
        Severity::Warning => "Warning",
        Severity::Info => "Info",
    };
    prompt.push_str(&format!("**Result:** FAILED ({severity_str})\n"));

    // Truncate reason to 500 chars
    let reason = if feedback.reason.len() > 500 {
        let mut truncated = feedback.reason[..500].to_string();
        truncated.push_str("...");
        truncated
    } else {
        feedback.reason.clone()
    };
    prompt.push_str(&format!("**Reason:** {reason}\n"));

    // Max 5 suggestions
    let suggestions: Vec<&CodeSuggestion> = feedback.suggestions.iter().take(5).collect();
    if !suggestions.is_empty() {
        prompt.push_str("\n**Suggestions:**\n");
        for s in suggestions {
            if let Some(line) = s.line {
                prompt.push_str(&format!("- `{}` line {}: {}\n", s.file, line, s.suggestion));
            } else {
                prompt.push_str(&format!("- `{}`: {}\n", s.file, s.suggestion));
            }
        }
    }

    prompt.push_str("\nPlease address the feedback above and produce corrected output.\n");
    prompt
}

/// Returns the priority index for an agent (lower = higher priority).
fn priority_index(agent: &AgentKind) -> usize {
    match agent {
        AgentKind::Claude(_) => 0,
        AgentKind::Gemini(_) => 1,
        AgentKind::Codex(_) => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase_runner::FileOutput;
    use ath_types::project::SkillTag;

    fn always_available(_: &AgentKind) -> bool {
        true
    }

    fn none_available(_: &AgentKind) -> bool {
        false
    }

    fn exclude(kind_fn: fn() -> AgentKind) -> impl Fn(&AgentKind) -> bool {
        move |agent: &AgentKind| mem::discriminant(agent) != mem::discriminant(&kind_fn())
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

    fn sample_output(task_name: &str, agent: AgentKind) -> TaskOutput {
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
        let agents = vec![AgentKind::Claude("opus-4".into())];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 2: Single Gemini-authored phase selects Claude as reviewer
    #[test]
    fn single_gemini_author_selects_claude() {
        let agents = vec![AgentKind::Gemini("2.5-pro".into())];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Claude(_)));
    }

    // Test 3: Multi-author phase (2 Claude, 1 Gemini) excludes Claude, selects Gemini
    #[test]
    fn majority_claude_selects_gemini() {
        let agents = vec![
            AgentKind::Claude("opus-4".into()),
            AgentKind::Claude("opus-4".into()),
            AgentKind::Gemini("2.5-pro".into()),
        ];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 4: Tie (1 Claude, 1 Gemini) excludes Claude (higher priority = majority tiebreak), selects Gemini
    #[test]
    fn tie_excludes_higher_priority_selects_next() {
        let agents = vec![
            AgentKind::Claude("opus-4".into()),
            AgentKind::Gemini("2.5-pro".into()),
        ];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 5: Preferred reviewer circuit breaker tripped, falls back to next
    #[test]
    fn fallback_when_preferred_unavailable() {
        let agents = vec![AgentKind::Gemini("2.5-pro".into())];
        let reviewer = select_reviewer(
            &agents,
            "test-phase",
            exclude(|| AgentKind::Claude("".into())),
        )
        .unwrap();
        assert!(matches!(reviewer, AgentKind::Codex(_)));
    }

    // Test 6: ALL non-author agents unavailable -> error
    #[test]
    fn error_when_all_non_author_unavailable() {
        let agents = vec![AgentKind::Claude("opus-4".into())];
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
        let agents = vec![AgentKind::Codex("o3".into())];
        let reviewer = select_reviewer(
            &agents,
            "test-phase",
            exclude(|| AgentKind::Claude("".into())),
        )
        .unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // ==================== build_review_prompt tests ====================

    // Test 1: build_review_prompt includes task name, description, and file contents
    #[test]
    fn review_prompt_includes_task_and_files() {
        let task = sample_task("Build API", "Create REST endpoints");
        let output = sample_output("Build API", AgentKind::Claude("opus-4".into()));

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
            sample_output("Task A", AgentKind::Claude("opus-4".into())),
            TaskOutput {
                task_name: "Task B".into(),
                agent: AgentKind::Gemini("2.5-pro".into()),
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

        let verdict =
            parse_review_verdict(json, AgentKind::Gemini("2.5-pro".into())).unwrap();

        assert!(!verdict.passed);
        assert_eq!(verdict.severity, Severity::Critical);
        assert_eq!(verdict.reason, "Security vulnerability found");
        assert!(matches!(verdict.reviewer, AgentKind::Gemini(_)));
        assert_eq!(verdict.suggestions.len(), 1);
        assert_eq!(verdict.suggestions[0].file, "src/auth.rs");
        assert_eq!(verdict.suggestions[0].line, Some(42));
    }

    // Test 4: parse_review_verdict returns error on malformed JSON
    #[test]
    fn parse_malformed_json_returns_error() {
        let result = parse_review_verdict("not json", AgentKind::Claude("opus-4".into()));
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

        let verdict =
            parse_review_verdict(json, AgentKind::Claude("opus-4".into())).unwrap();

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
            reviewer: AgentKind::Gemini("2.5-pro".into()),
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
            reviewer: AgentKind::Claude("opus-4".into()),
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
        let verdict =
            parse_review_verdict(json, AgentKind::Claude("opus-4".into())).unwrap();
        assert_eq!(verdict.severity, Severity::Warning);
    }

    // Test: build_retry_prompt with suggestion without line number
    #[test]
    fn retry_prompt_suggestion_without_line() {
        let task = sample_task("Task", "Desc");
        let feedback = ReviewVerdict {
            passed: false,
            reviewer: AgentKind::Claude("opus-4".into()),
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
