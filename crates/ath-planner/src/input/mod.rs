//! Input mode resolution and parsing infrastructure.
//!
//! Determines how the user wants to provide project input:
//! natural language description, spec file, or existing codebase.

pub mod error;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use error::InputError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
}
