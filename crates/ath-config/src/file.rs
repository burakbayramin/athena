//! TOML config file reading and parsing.

use std::path::Path;

use serde::Deserialize;

use crate::error::ConfigError;

/// Raw configuration as read from a TOML file.
///
/// All fields are optional — a partial config is valid.
/// The `[providers]` and `[defaults]` sections map to nested structs.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct RawFileConfig {
    /// Provider API keys.
    pub providers: Option<ProvidersConfig>,
    /// Default model selections.
    pub defaults: Option<DefaultsConfig>,
}

/// API key configuration for each provider.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ProvidersConfig {
    pub anthropic_api_key: Option<String>,
    pub google_api_key: Option<String>,
    pub openai_api_key: Option<String>,
}

/// Default model selections per provider.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DefaultsConfig {
    pub claude_model: Option<String>,
    pub gemini_model: Option<String>,
    pub codex_model: Option<String>,
}

impl RawFileConfig {
    /// Helper to get the anthropic API key, if set.
    pub fn anthropic_api_key(&self) -> Option<&str> {
        self.providers.as_ref()?.anthropic_api_key.as_deref()
    }

    /// Helper to get the google API key, if set.
    pub fn google_api_key(&self) -> Option<&str> {
        self.providers.as_ref()?.google_api_key.as_deref()
    }

    /// Helper to get the openai API key, if set.
    pub fn openai_api_key(&self) -> Option<&str> {
        self.providers.as_ref()?.openai_api_key.as_deref()
    }

    /// Helper to get the claude model, if set.
    pub fn claude_model(&self) -> Option<&str> {
        self.defaults.as_ref()?.claude_model.as_deref()
    }

    /// Helper to get the gemini model, if set.
    pub fn gemini_model(&self) -> Option<&str> {
        self.defaults.as_ref()?.gemini_model.as_deref()
    }

    /// Helper to get the codex model, if set.
    pub fn codex_model(&self) -> Option<&str> {
        self.defaults.as_ref()?.codex_model.as_deref()
    }
}

/// Load and parse a TOML config file from the given path.
///
/// Returns `ConfigError::FileNotFound` if the file does not exist.
/// Returns `ConfigError::InvalidToml` if the file cannot be parsed.
pub fn load_config_file(path: &Path) -> Result<RawFileConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound {
        path: path.display().to_string(),
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::InvalidToml { source: e })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_full_toml() {
        let toml_str = r#"
[providers]
anthropic_api_key = "sk-ant-123"
google_api_key = "AIza-456"
openai_api_key = "sk-789"

[defaults]
claude_model = "opus-4"
gemini_model = "2.5-pro"
codex_model = "o3"
"#;
        let config: RawFileConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.anthropic_api_key(), Some("sk-ant-123"));
        assert_eq!(config.google_api_key(), Some("AIza-456"));
        assert_eq!(config.openai_api_key(), Some("sk-789"));
        assert_eq!(config.claude_model(), Some("opus-4"));
        assert_eq!(config.gemini_model(), Some("2.5-pro"));
        assert_eq!(config.codex_model(), Some("o3"));
    }

    #[test]
    fn parse_partial_toml_missing_optional_fields() {
        let toml_str = r#"
[providers]
anthropic_api_key = "sk-ant-123"
"#;
        let config: RawFileConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.anthropic_api_key(), Some("sk-ant-123"));
        assert_eq!(config.google_api_key(), None);
        assert_eq!(config.openai_api_key(), None);
        assert_eq!(config.claude_model(), None);
    }

    #[test]
    fn parse_empty_toml_succeeds() {
        let config: RawFileConfig = toml::from_str("").unwrap();
        assert_eq!(config.anthropic_api_key(), None);
        assert_eq!(config.google_api_key(), None);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = toml::from_str::<RawFileConfig>("[[[ not valid toml");
        assert!(result.is_err());
    }

    #[test]
    fn load_nonexistent_file_returns_file_not_found() {
        let result = load_config_file(Path::new("/nonexistent/path/config.toml"));
        assert!(matches!(result, Err(ConfigError::FileNotFound { .. })));
    }

    #[test]
    fn load_valid_file_from_disk() {
        let dir = std::env::temp_dir().join("ath-config-test-file");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            r#"
[providers]
anthropic_api_key = "test-key"
"#,
        )
        .unwrap();

        let config = load_config_file(&path).unwrap();
        assert_eq!(config.anthropic_api_key(), Some("test-key"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_invalid_toml_file_returns_error() {
        let dir = std::env::temp_dir().join("ath-config-test-invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.toml");
        std::fs::write(&path, "[[[ not valid").unwrap();

        let result = load_config_file(&path);
        assert!(matches!(result, Err(ConfigError::InvalidToml { .. })));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
