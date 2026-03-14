//! Skill routing configuration: map skill tags to agents via `.ath/skills.toml`.
//!
//! Allows users to customize which agent handles which skill tags.
//! When no config file exists, the hardcoded defaults in taxonomy.rs apply.
//!
//! Example `.ath/skills.toml`:
//! ```toml
//! [default]
//! provider = "anthropic"
//! model = "opus-4"
//!
//! [routes]
//! rust = { provider = "anthropic", model = "opus-4" }
//! docs = { provider = "google", model = "2.5-pro" }
//! codegen = { provider = "openai", model = "o3" }
//! python = { provider = "ollama", model = "codestral" }
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::ConfigError;

/// Reference to an agent by provider and model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentRef {
    /// Provider name (e.g., "anthropic", "google", "openai", "ollama").
    pub provider: String,
    /// Model identifier (e.g., "opus-4", "2.5-pro", "o3").
    pub model: String,
}

/// Skills routing configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkillsConfig {
    /// Default agent for unrecognized skill tags. If absent, uses hardcoded default.
    #[serde(default)]
    pub default: Option<AgentRef>,
    /// Skill tag → agent mappings. Tags are case-insensitive (stored lowercase).
    #[serde(default)]
    pub routes: HashMap<String, AgentRef>,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            default: None,
            routes: HashMap::new(),
        }
    }
}

impl SkillsConfig {
    /// Load skills config from a TOML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound {
                path: path.display().to_string(),
            })?;
        Self::parse(&content)
    }

    /// Parse skills config from a TOML string.
    pub fn parse(toml_str: &str) -> Result<Self, ConfigError> {
        let mut config: Self =
            toml::from_str(toml_str).map_err(|e| ConfigError::InvalidToml { source: e })?;

        // Normalize route keys to lowercase
        let routes = std::mem::take(&mut config.routes);
        config.routes = routes
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();

        config.validate()?;
        Ok(config)
    }

    /// Validate the parsed config.
    fn validate(&self) -> Result<(), ConfigError> {
        if let Some(ref default) = self.default {
            if default.provider.is_empty() || default.model.is_empty() {
                return Err(ConfigError::InvalidSkillsConfig {
                    reason: "default agent must have non-empty provider and model".into(),
                });
            }
        }

        for (tag, agent_ref) in &self.routes {
            if agent_ref.provider.is_empty() || agent_ref.model.is_empty() {
                return Err(ConfigError::InvalidSkillsConfig {
                    reason: format!(
                        "route for tag '{}' must have non-empty provider and model",
                        tag
                    ),
                });
            }
        }

        Ok(())
    }

    /// Returns true if this config has any routes or a custom default.
    pub fn has_overrides(&self) -> bool {
        self.default.is_some() || !self.routes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_skills_config() {
        let toml = r#"
[default]
provider = "anthropic"
model = "opus-4"

[routes]
rust = { provider = "anthropic", model = "opus-4" }
docs = { provider = "google", model = "2.5-pro" }
codegen = { provider = "openai", model = "o3" }
python = { provider = "ollama", model = "codestral" }
"#;
        let config = SkillsConfig::parse(toml).unwrap();
        assert!(config.default.is_some());
        assert_eq!(config.default.as_ref().unwrap().provider, "anthropic");
        assert_eq!(config.routes.len(), 4);
        assert_eq!(config.routes["rust"].provider, "anthropic");
        assert_eq!(config.routes["python"].provider, "ollama");
        assert_eq!(config.routes["python"].model, "codestral");
    }

    #[test]
    fn parse_routes_only() {
        let toml = r#"
[routes]
rust = { provider = "anthropic", model = "opus-4" }
"#;
        let config = SkillsConfig::parse(toml).unwrap();
        assert!(config.default.is_none());
        assert_eq!(config.routes.len(), 1);
    }

    #[test]
    fn parse_empty_config() {
        let config = SkillsConfig::parse("").unwrap();
        assert!(config.default.is_none());
        assert!(config.routes.is_empty());
        assert!(!config.has_overrides());
    }

    #[test]
    fn routes_normalized_to_lowercase() {
        let toml = r#"
[routes]
Rust = { provider = "anthropic", model = "opus-4" }
DOCS = { provider = "google", model = "2.5-pro" }
"#;
        let config = SkillsConfig::parse(toml).unwrap();
        assert!(config.routes.contains_key("rust"));
        assert!(config.routes.contains_key("docs"));
        assert!(!config.routes.contains_key("Rust"));
    }

    #[test]
    fn validate_empty_provider_in_default() {
        let toml = r#"
[default]
provider = ""
model = "opus-4"
"#;
        let err = SkillsConfig::parse(toml).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSkillsConfig { .. }));
    }

    #[test]
    fn validate_empty_model_in_route() {
        let toml = r#"
[routes]
rust = { provider = "anthropic", model = "" }
"#;
        let err = SkillsConfig::parse(toml).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSkillsConfig { .. }));
    }

    #[test]
    fn has_overrides_true_with_routes() {
        let toml = r#"
[routes]
rust = { provider = "anthropic", model = "opus-4" }
"#;
        let config = SkillsConfig::parse(toml).unwrap();
        assert!(config.has_overrides());
    }

    #[test]
    fn has_overrides_true_with_default_only() {
        let toml = r#"
[default]
provider = "anthropic"
model = "opus-4"
"#;
        let config = SkillsConfig::parse(toml).unwrap();
        assert!(config.has_overrides());
    }

    #[test]
    fn load_from_file() {
        let dir = std::env::temp_dir().join("ath-skills-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("skills.toml");
        std::fs::write(
            &path,
            r#"
[routes]
rust = { provider = "anthropic", model = "opus-4" }
"#,
        )
        .unwrap();

        let config = SkillsConfig::load(&path).unwrap();
        assert_eq!(config.routes.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_nonexistent_file() {
        let result = SkillsConfig::load(Path::new("/nonexistent/skills.toml"));
        assert!(matches!(result, Err(ConfigError::FileNotFound { .. })));
    }
}
