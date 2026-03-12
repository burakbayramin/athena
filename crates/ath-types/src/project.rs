//! Project specification types.
//!
//! `ProjectSpec` defines the full contract for a project that Athena will work on,
//! including goals, constraints, and skill requirements.

use serde::{Deserialize, Serialize};

use crate::error::ValidationError;

/// A skill tag associated with a project goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTag(pub String);

/// A single project goal with associated skill requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalSpec {
    /// Description of what this goal achieves.
    pub description: String,
    /// Skills needed to accomplish this goal.
    pub skill_tags: Vec<SkillTag>,
}

/// Full project specification — the top-level contract for a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSpec {
    /// Project name.
    pub name: String,
    /// Project description.
    pub description: String,
    /// List of goals for the project.
    pub goals: Vec<GoalSpec>,
    /// Constraints that must be respected.
    pub constraints: Vec<String>,
    /// Target programming language (optional).
    pub target_language: Option<String>,
    /// Target framework (optional).
    pub target_framework: Option<String>,
    /// Expected output files.
    pub expected_files: Vec<String>,
}

impl ProjectSpec {
    /// Validates the project spec, returning a typed error with a fix hint on failure.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyField {
                field: "name".into(),
                hint: "Provide a project name".into(),
            });
        }
        if self.goals.is_empty() {
            return Err(ValidationError::EmptyField {
                field: "goals".into(),
                hint: "Provide at least one project goal".into(),
            });
        }
        for (i, goal) in self.goals.iter().enumerate() {
            if goal.description.trim().is_empty() {
                return Err(ValidationError::EmptyField {
                    field: format!("goals[{}].description", i),
                    hint: "Each goal must have a non-empty description".into(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_project_spec() -> ProjectSpec {
        ProjectSpec {
            name: "todo-app".into(),
            description: "A REST API todo application".into(),
            goals: vec![GoalSpec {
                description: "CRUD endpoints".into(),
                skill_tags: vec![SkillTag("rust".into()), SkillTag("api".into())],
            }],
            constraints: vec!["Must use PostgreSQL".into()],
            target_language: Some("Rust".into()),
            target_framework: Some("Axum".into()),
            expected_files: vec!["src/main.rs".into()],
        }
    }

    #[test]
    fn project_spec_round_trip() {
        let spec = valid_project_spec();
        let json = serde_json::to_string(&spec).expect("serialize");
        let deserialized: ProjectSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(spec, deserialized);
    }

    #[test]
    fn project_spec_empty_name_fails_validation() {
        let mut spec = valid_project_spec();
        spec.name = "".into();
        let err = spec.validate().unwrap_err();
        match &err {
            ValidationError::EmptyField { field, hint } => {
                assert_eq!(field, "name");
                assert_eq!(hint, "Provide a project name");
            }
            _ => panic!("Expected EmptyField error, got: {:?}", err),
        }
    }

    #[test]
    fn project_spec_empty_goals_fails_validation() {
        let mut spec = valid_project_spec();
        spec.goals = vec![];
        let err = spec.validate().unwrap_err();
        match &err {
            ValidationError::EmptyField { field, .. } => {
                assert_eq!(field, "goals");
            }
            _ => panic!("Expected EmptyField error, got: {:?}", err),
        }
    }

    #[test]
    fn project_spec_valid_passes() {
        let spec = valid_project_spec();
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn project_spec_empty_goal_description_fails() {
        let mut spec = valid_project_spec();
        spec.goals = vec![GoalSpec {
            description: "".into(),
            skill_tags: vec![],
        }];
        let err = spec.validate().unwrap_err();
        match &err {
            ValidationError::EmptyField { field, .. } => {
                assert!(field.contains("goals[0].description"));
            }
            _ => panic!("Expected EmptyField error, got: {:?}", err),
        }
    }
}
