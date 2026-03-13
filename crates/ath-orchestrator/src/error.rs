//! Isolation error types with fix hints.
//!
//! Errors specific to module isolation: file ownership conflicts,
//! agent availability failures, and unassigned task detection.

use thiserror::Error;

/// Errors returned during module isolation validation and agent routing.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum IsolationError {
    /// Two tasks in parallel phases claim the same file.
    #[error("File '{file}' claimed by task '{task_a}' (phase {phase_a}) and task '{task_b}' (phase {phase_b})")]
    FileConflict {
        file: String,
        task_a: String,
        phase_a: u32,
        task_b: String,
        phase_b: u32,
        hint: String,
    },

    /// All agent providers are unavailable (circuit breakers tripped).
    #[error("All agents unavailable: {providers}")]
    AllAgentsUnavailable {
        providers: String,
        hint: String,
    },

    /// A task has no assigned agent after routing.
    #[error("Task '{task_name}' has no assigned agent")]
    UnassignedTask {
        task_name: String,
        hint: String,
    },
}

impl IsolationError {
    /// Returns the fix hint for this error.
    pub fn hint(&self) -> &str {
        match self {
            IsolationError::FileConflict { hint, .. }
            | IsolationError::AllAgentsUnavailable { hint, .. }
            | IsolationError::UnassignedTask { hint, .. } => hint,
        }
    }
}

/// Errors returned during phase runner execution.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum PhaseRunnerError {
    /// No reviewer agent is available for the phase.
    #[error("No reviewer available for phase '{phase_name}': {reason}")]
    NoReviewerAvailable {
        phase_name: String,
        reason: String,
    },

    /// Maximum retry attempts exhausted for a phase.
    #[error("Phase '{phase_name}' exceeded max retries after review by '{reviewer}' on attempt {attempts}: {final_reason}")]
    MaxRetriesExceeded {
        phase_name: String,
        reviewer: String,
        attempts: u32,
        final_reason: String,
    },

    /// A task execution failed.
    #[error("Task '{task_name}' failed (agent: {agent}): {reason}")]
    TaskExecutionFailed {
        task_name: String,
        agent: String,
        reason: String,
    },

    /// Failed to dispatch a review request.
    #[error("Review dispatch to '{reviewer}' failed: {reason}")]
    ReviewDispatchFailed {
        reviewer: String,
        reason: String,
    },

    /// Atomic file write failed.
    #[error("Atomic write to '{path}' failed: {reason}")]
    AtomicWriteFailed {
        path: String,
        reason: String,
    },
}

impl PhaseRunnerError {
    /// Returns actionable guidance for resolving this error.
    pub fn hint(&self) -> &str {
        match self {
            PhaseRunnerError::NoReviewerAvailable { .. } => {
                "Check that at least one review-capable agent is configured and available"
            }
            PhaseRunnerError::MaxRetriesExceeded { .. } => {
                "Review the failing phase's tasks and acceptance criteria; consider manual intervention"
            }
            PhaseRunnerError::TaskExecutionFailed { .. } => {
                "Check the agent's error output and retry the task, or reassign to a different agent"
            }
            PhaseRunnerError::ReviewDispatchFailed { .. } => {
                "Verify the reviewer agent is online and accepting requests"
            }
            PhaseRunnerError::AtomicWriteFailed { .. } => {
                "Check file permissions and disk space at the target path"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_conflict_display() {
        let err = IsolationError::FileConflict {
            file: "src/main.rs".into(),
            task_a: "Build API".into(),
            phase_a: 1,
            task_b: "Add CLI".into(),
            phase_b: 2,
            hint: "Split into separate phases or consolidate into one task".into(),
        };
        assert_eq!(
            err.to_string(),
            "File 'src/main.rs' claimed by task 'Build API' (phase 1) and task 'Add CLI' (phase 2)"
        );
    }

    #[test]
    fn file_conflict_hint() {
        let err = IsolationError::FileConflict {
            file: "src/lib.rs".into(),
            task_a: "A".into(),
            phase_a: 1,
            task_b: "B".into(),
            phase_b: 2,
            hint: "Consolidate file ownership".into(),
        };
        assert_eq!(err.hint(), "Consolidate file ownership");
    }

    #[test]
    fn all_agents_unavailable_display() {
        let err = IsolationError::AllAgentsUnavailable {
            providers: "Anthropic, Google, OpenAI".into(),
            hint: "Check API keys and provider quotas".into(),
        };
        assert_eq!(
            err.to_string(),
            "All agents unavailable: Anthropic, Google, OpenAI"
        );
    }

    #[test]
    fn all_agents_unavailable_hint() {
        let err = IsolationError::AllAgentsUnavailable {
            providers: "Anthropic".into(),
            hint: "Check API keys".into(),
        };
        assert_eq!(err.hint(), "Check API keys");
    }

    #[test]
    fn unassigned_task_display() {
        let err = IsolationError::UnassignedTask {
            task_name: "Build database".into(),
            hint: "Run the router before dispatching tasks".into(),
        };
        assert_eq!(
            err.to_string(),
            "Task 'Build database' has no assigned agent"
        );
    }

    #[test]
    fn unassigned_task_hint() {
        let err = IsolationError::UnassignedTask {
            task_name: "Test".into(),
            hint: "Assign an agent first".into(),
        };
        assert_eq!(err.hint(), "Assign an agent first");
    }

    // -- PhaseRunnerError tests --

    #[test]
    fn no_reviewer_available_display() {
        let err = PhaseRunnerError::NoReviewerAvailable {
            phase_name: "code-review".into(),
            reason: "all agents offline".into(),
        };
        assert_eq!(
            err.to_string(),
            "No reviewer available for phase 'code-review': all agents offline"
        );
    }

    #[test]
    fn max_retries_exceeded_display() {
        let err = PhaseRunnerError::MaxRetriesExceeded {
            phase_name: "build-phase".into(),
            reviewer: "Google/2.5-pro".into(),
            attempts: 3,
            final_reason: "tests still failing".into(),
        };
        assert_eq!(
            err.to_string(),
            "Phase 'build-phase' exceeded max retries after review by 'Google/2.5-pro' on attempt 3: tests still failing"
        );
    }

    #[test]
    fn task_execution_failed_display() {
        let err = PhaseRunnerError::TaskExecutionFailed {
            task_name: "implement-auth".into(),
            agent: "Claude(opus-4)".into(),
            reason: "timeout".into(),
        };
        assert_eq!(
            err.to_string(),
            "Task 'implement-auth' failed (agent: Claude(opus-4)): timeout"
        );
    }

    #[test]
    fn review_dispatch_failed_display() {
        let err = PhaseRunnerError::ReviewDispatchFailed {
            reviewer: "Gemini(2.5-pro)".into(),
            reason: "connection refused".into(),
        };
        assert_eq!(
            err.to_string(),
            "Review dispatch to 'Gemini(2.5-pro)' failed: connection refused"
        );
    }

    #[test]
    fn atomic_write_failed_display() {
        let err = PhaseRunnerError::AtomicWriteFailed {
            path: "/tmp/output.rs".into(),
            reason: "permission denied".into(),
        };
        assert_eq!(
            err.to_string(),
            "Atomic write to '/tmp/output.rs' failed: permission denied"
        );
    }

    #[test]
    fn all_phase_runner_variants_have_hints() {
        let errors: Vec<PhaseRunnerError> = vec![
            PhaseRunnerError::NoReviewerAvailable {
                phase_name: "p".into(),
                reason: "r".into(),
            },
            PhaseRunnerError::MaxRetriesExceeded {
                phase_name: "p".into(),
                reviewer: "Google/2.5-pro".into(),
                attempts: 3,
                final_reason: "r".into(),
            },
            PhaseRunnerError::TaskExecutionFailed {
                task_name: "t".into(),
                agent: "a".into(),
                reason: "r".into(),
            },
            PhaseRunnerError::ReviewDispatchFailed {
                reviewer: "rev".into(),
                reason: "r".into(),
            },
            PhaseRunnerError::AtomicWriteFailed {
                path: "p".into(),
                reason: "r".into(),
            },
        ];
        for err in &errors {
            assert!(!err.hint().is_empty(), "hint should not be empty for {:?}", err);
        }
    }
}
