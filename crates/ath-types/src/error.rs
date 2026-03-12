//! Validation error types with fix hints.
//!
//! Every validation error includes an actionable hint so users know how to fix the issue.

use thiserror::Error;

/// Errors returned by `validate()` methods on inter-agent schema types.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum ValidationError {
    /// A required field is empty or missing.
    #[error("Required field '{field}' is empty")]
    EmptyField {
        field: String,
        hint: String,
    },

    /// A field has an invalid value.
    #[error("Invalid value for '{field}': {reason}")]
    InvalidValue {
        field: String,
        reason: String,
        hint: String,
    },

    /// A circular dependency was detected in the phase DAG.
    #[error("Circular dependency detected: {cycle_path}")]
    CircularDependency {
        cycle_path: String,
        hint: String,
    },

    /// A phase depends on a non-existent phase.
    #[error("Phase {phase_id} depends on non-existent phase {missing_id}")]
    MissingDependencyTarget {
        phase_id: u32,
        missing_id: u32,
        hint: String,
    },

    /// A project goal has no task mapping in any phase.
    #[error("Goal {goal_index} has no task mapping: {goal_description}")]
    OrphanedGoal {
        goal_index: usize,
        goal_description: String,
        hint: String,
    },

    /// A phase has no tasks.
    #[error("Phase {phase_id} '{phase_name}' has no tasks")]
    EmptyPhase {
        phase_id: u32,
        phase_name: String,
        hint: String,
    },

    /// A phase consumes a contract that no dependency produces.
    #[error("Phase {phase_id} consumes '{contract}' but no dependency produces it")]
    UnsatisfiedContract {
        phase_id: u32,
        contract: String,
        hint: String,
    },
}

impl ValidationError {
    /// Returns the fix hint for this error.
    pub fn hint(&self) -> &str {
        match self {
            ValidationError::EmptyField { hint, .. }
            | ValidationError::InvalidValue { hint, .. }
            | ValidationError::CircularDependency { hint, .. }
            | ValidationError::MissingDependencyTarget { hint, .. }
            | ValidationError::OrphanedGoal { hint, .. }
            | ValidationError::EmptyPhase { hint, .. }
            | ValidationError::UnsatisfiedContract { hint, .. } => hint,
        }
    }

    /// Convenience constructor for empty field errors.
    pub fn empty_field(field: &str, hint: &str) -> Self {
        ValidationError::EmptyField {
            field: field.into(),
            hint: hint.into(),
        }
    }

    /// Convenience constructor for invalid value errors.
    pub fn invalid_value(field: &str, reason: &str, hint: &str) -> Self {
        ValidationError::InvalidValue {
            field: field.into(),
            reason: reason.into(),
            hint: hint.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_returns_fix_hint_for_empty_field() {
        let err = ValidationError::empty_field("name", "Provide a project name");
        assert_eq!(err.hint(), "Provide a project name");
    }

    #[test]
    fn hint_returns_fix_hint_for_invalid_value() {
        let err = ValidationError::invalid_value("severity", "unknown level", "Use critical, warning, or info");
        assert_eq!(err.hint(), "Use critical, warning, or info");
    }

    #[test]
    fn display_format_empty_field() {
        let err = ValidationError::empty_field("name", "Provide a name");
        assert_eq!(err.to_string(), "Required field 'name' is empty");
    }

    #[test]
    fn display_format_invalid_value() {
        let err = ValidationError::invalid_value("age", "must be positive", "Use a positive number");
        assert_eq!(err.to_string(), "Invalid value for 'age': must be positive");
    }

    #[test]
    fn circular_dependency_display_and_hint() {
        let err = ValidationError::CircularDependency {
            cycle_path: "1 -> 2 -> 3 -> 1".into(),
            hint: "Remove one dependency edge to break the cycle".into(),
        };
        assert_eq!(err.to_string(), "Circular dependency detected: 1 -> 2 -> 3 -> 1");
        assert_eq!(err.hint(), "Remove one dependency edge to break the cycle");
    }

    #[test]
    fn missing_dependency_target_display_and_hint() {
        let err = ValidationError::MissingDependencyTarget {
            phase_id: 3,
            missing_id: 99,
            hint: "Check that depends_on references valid phase IDs".into(),
        };
        assert_eq!(err.to_string(), "Phase 3 depends on non-existent phase 99");
        assert_eq!(err.hint(), "Check that depends_on references valid phase IDs");
    }

    #[test]
    fn orphaned_goal_display_and_hint() {
        let err = ValidationError::OrphanedGoal {
            goal_index: 2,
            goal_description: "Build the UI".into(),
            hint: "Add a task that maps to this goal".into(),
        };
        assert_eq!(err.to_string(), "Goal 2 has no task mapping: Build the UI");
        assert_eq!(err.hint(), "Add a task that maps to this goal");
    }

    #[test]
    fn empty_phase_display_and_hint() {
        let err = ValidationError::EmptyPhase {
            phase_id: 5,
            phase_name: "Testing".into(),
            hint: "Add at least one task to the phase".into(),
        };
        assert_eq!(err.to_string(), "Phase 5 'Testing' has no tasks");
        assert_eq!(err.hint(), "Add at least one task to the phase");
    }

    #[test]
    fn unsatisfied_contract_display_and_hint() {
        let err = ValidationError::UnsatisfiedContract {
            phase_id: 4,
            contract: "db-schema".into(),
            hint: "Add a dependency on the phase that produces 'db-schema'".into(),
        };
        assert_eq!(
            err.to_string(),
            "Phase 4 consumes 'db-schema' but no dependency produces it"
        );
        assert_eq!(
            err.hint(),
            "Add a dependency on the phase that produces 'db-schema'"
        );
    }
}
