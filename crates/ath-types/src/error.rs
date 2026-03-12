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
}

impl ValidationError {
    /// Returns the fix hint for this error.
    pub fn hint(&self) -> &str {
        match self {
            ValidationError::EmptyField { hint, .. } => hint,
            ValidationError::InvalidValue { hint, .. } => hint,
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
}
