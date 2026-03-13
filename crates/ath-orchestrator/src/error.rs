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
}
