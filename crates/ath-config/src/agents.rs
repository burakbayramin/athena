//! Agent configuration: define agents via `.ath/agents.toml`.
//!
//! Each agent entry specifies a provider, model, and environment variable name
//! for the API key. Optional fields include `base_url` (for self-hosted or
//! OpenAI-compatible endpoints) and `display_name` (for CLI output).
//!
//! When no `.ath/agents.toml` exists, defaults are synthesized from the
//! existing environment variables (ANTHROPIC_API_KEY, GOOGLE_API_KEY, etc.).

use std::path::Path;

use serde::Deserialize;

use crate::error::ConfigError;

/// Well-known provider names (must match AgentId conventions in ath-types).
const PROVIDER_ANTHROPIC: &str = "anthropic";
const PROVIDER_GOOGLE: &str = "google";
const PROVIDER_OPENAI: &str = "openai";

/// Default models per well-known provider.
const DEFAULT_CLAUDE_MODEL: &str = "opus-4";
const DEFAULT_GEMINI_MODEL: &str = "2.5-pro";
const DEFAULT_CODEX_MODEL: &str = "o3";

/// Default env var names for well-known providers.
const ENV_ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";
const ENV_GOOGLE_API_KEY: &str = "GOOGLE_API_KEY";
const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";

/// A single agent definition from config.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentConfig {
    /// Provider name (e.g., "anthropic", "google", "openai", "ollama").
    pub provider: String,
    /// Model identifier (e.g., "opus-4", "2.5-pro", "o3", "llama3.3").
    pub model: String,
    /// Environment variable name that holds the API key (e.g., "ANTHROPIC_API_KEY").
    pub api_key_env: String,
    /// Optional base URL for custom/self-hosted endpoints.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Optional human-readable name for CLI output.
    #[serde(default)]
    pub display_name: Option<String>,
}

impl AgentConfig {
    /// Resolve the API key from the environment.
    ///
    /// Returns `None` if the env var is not set or is empty.
    pub fn resolve_api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env)
            .ok()
            .filter(|v| !v.is_empty())
    }

    /// The display name, falling back to "Provider/model".
    pub fn name(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| format!("{}/{}", self.provider, self.model))
    }

    /// Is this a well-known provider (anthropic/google/openai)?
    pub fn is_builtin_provider(&self) -> bool {
        matches!(
            self.provider.as_str(),
            PROVIDER_ANTHROPIC | PROVIDER_GOOGLE | PROVIDER_OPENAI
        )
    }
}

/// Top-level agents configuration.
///
/// Parsed from `.ath/agents.toml`:
/// ```toml
/// [[agents]]
/// provider = "anthropic"
/// model = "opus-4"
/// api_key_env = "ANTHROPIC_API_KEY"
///
/// [[agents]]
/// provider = "ollama"
/// model = "llama3.3"
/// api_key_env = "OLLAMA_API_KEY"
/// base_url = "http://localhost:11434"
/// display_name = "Local Llama"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct AgentsConfig {
    /// Agent definitions.
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
}

impl AgentsConfig {
    /// Load agents config from a TOML file.
    ///
    /// Returns `Ok(config)` on success, `Err` on parse/validation failure.
    /// Does NOT fall back to defaults — caller should use `default_agents_from_env()`
    /// when file is not found.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound {
            path: path.display().to_string(),
        })?;
        Self::parse(&content)
    }

    /// Parse agents config from a TOML string.
    pub fn parse(toml_str: &str) -> Result<Self, ConfigError> {
        let config: Self =
            toml::from_str(toml_str).map_err(|e| ConfigError::InvalidToml { source: e })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the parsed config.
    fn validate(&self) -> Result<(), ConfigError> {
        for (i, agent) in self.agents.iter().enumerate() {
            if agent.provider.is_empty() {
                return Err(ConfigError::InvalidAgentConfig {
                    index: i,
                    reason: "provider is required".into(),
                });
            }
            if agent.model.is_empty() {
                return Err(ConfigError::InvalidAgentConfig {
                    index: i,
                    reason: "model is required".into(),
                });
            }
            if agent.api_key_env.is_empty() {
                return Err(ConfigError::InvalidAgentConfig {
                    index: i,
                    reason: "api_key_env is required".into(),
                });
            }
        }

        // Check for duplicate provider+model combos
        let mut seen = std::collections::HashSet::new();
        for (i, agent) in self.agents.iter().enumerate() {
            let key = (agent.provider.to_lowercase(), agent.model.clone());
            if !seen.insert(key) {
                return Err(ConfigError::InvalidAgentConfig {
                    index: i,
                    reason: format!("duplicate agent: {}/{}", agent.provider, agent.model),
                });
            }
        }

        Ok(())
    }

    /// Returns agents that have their API key available in the environment.
    pub fn available_agents(&self) -> Vec<&AgentConfig> {
        self.agents
            .iter()
            .filter(|a| a.resolve_api_key().is_some())
            .collect()
    }

    /// Returns true if any agent has its API key available.
    pub fn has_any_available(&self) -> bool {
        self.agents.iter().any(|a| a.resolve_api_key().is_some())
    }
}

/// Build default agent definitions from environment variables.
///
/// Checks for well-known env vars (ANTHROPIC_API_KEY, GOOGLE_API_KEY,
/// OPENAI_API_KEY) and creates agent entries for those that are set.
/// This is the fallback when no `.ath/agents.toml` exists.
pub fn default_agents_from_env() -> AgentsConfig {
    let mut agents = Vec::new();

    // Check each well-known provider
    if std::env::var(ENV_ANTHROPIC_API_KEY)
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        agents.push(AgentConfig {
            provider: PROVIDER_ANTHROPIC.into(),
            model: DEFAULT_CLAUDE_MODEL.into(),
            api_key_env: ENV_ANTHROPIC_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    if std::env::var(ENV_GOOGLE_API_KEY)
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        agents.push(AgentConfig {
            provider: PROVIDER_GOOGLE.into(),
            model: DEFAULT_GEMINI_MODEL.into(),
            api_key_env: ENV_GOOGLE_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    if std::env::var(ENV_OPENAI_API_KEY)
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        agents.push(AgentConfig {
            provider: PROVIDER_OPENAI.into(),
            model: DEFAULT_CODEX_MODEL.into(),
            api_key_env: ENV_OPENAI_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    AgentsConfig { agents }
}

/// Build default agent definitions from a ConfigStore.
///
/// Uses the ConfigStore's existing API keys and model selections to create
/// agent entries. This bridges the old config format to the new one.
pub fn default_agents_from_config(
    anthropic_key: Option<&str>,
    google_key: Option<&str>,
    openai_key: Option<&str>,
    claude_model: &str,
    gemini_model: &str,
    codex_model: &str,
) -> AgentsConfig {
    let mut agents = Vec::new();

    if anthropic_key.is_some() {
        agents.push(AgentConfig {
            provider: PROVIDER_ANTHROPIC.into(),
            model: claude_model.into(),
            api_key_env: ENV_ANTHROPIC_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    if google_key.is_some() {
        agents.push(AgentConfig {
            provider: PROVIDER_GOOGLE.into(),
            model: gemini_model.into(),
            api_key_env: ENV_GOOGLE_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    if openai_key.is_some() {
        agents.push(AgentConfig {
            provider: PROVIDER_OPENAI.into(),
            model: codex_model.into(),
            api_key_env: ENV_OPENAI_API_KEY.into(),
            base_url: None,
            display_name: None,
        });
    }

    AgentsConfig { agents }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_full_agents_toml() {
        let toml = r#"
[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = "ANTHROPIC_API_KEY"

[[agents]]
provider = "google"
model = "2.5-pro"
api_key_env = "GOOGLE_API_KEY"

[[agents]]
provider = "openai"
model = "o3"
api_key_env = "OPENAI_API_KEY"
"#;
        let config = AgentsConfig::parse(toml).unwrap();
        assert_eq!(config.agents.len(), 3);
        assert_eq!(config.agents[0].provider, "anthropic");
        assert_eq!(config.agents[0].model, "opus-4");
        assert_eq!(config.agents[1].provider, "google");
        assert_eq!(config.agents[2].provider, "openai");
    }

    #[test]
    fn parse_custom_provider_with_base_url() {
        let toml = r#"
[[agents]]
provider = "ollama"
model = "llama3.3"
api_key_env = "OLLAMA_API_KEY"
base_url = "http://localhost:11434"
display_name = "Local Llama"
"#;
        let config = AgentsConfig::parse(toml).unwrap();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].provider, "ollama");
        assert_eq!(
            config.agents[0].base_url.as_deref(),
            Some("http://localhost:11434")
        );
        assert_eq!(
            config.agents[0].display_name.as_deref(),
            Some("Local Llama")
        );
    }

    #[test]
    fn parse_empty_agents_list() {
        let config = AgentsConfig::parse("").unwrap();
        assert!(config.agents.is_empty());
    }

    #[test]
    fn parse_explicit_empty_agents() {
        let toml = "agents = []";
        let config = AgentsConfig::parse(toml).unwrap();
        assert!(config.agents.is_empty());
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    #[test]
    fn validate_missing_provider() {
        let toml = r#"
[[agents]]
provider = ""
model = "opus-4"
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let err = AgentsConfig::parse(toml).unwrap_err();
        match &err {
            ConfigError::InvalidAgentConfig { index, reason } => {
                assert_eq!(*index, 0);
                assert!(reason.contains("provider"));
            }
            _ => panic!("expected InvalidAgentConfig, got: {err:?}"),
        }
    }

    #[test]
    fn validate_missing_model() {
        let toml = r#"
[[agents]]
provider = "anthropic"
model = ""
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let err = AgentsConfig::parse(toml).unwrap_err();
        match &err {
            ConfigError::InvalidAgentConfig { index, reason } => {
                assert_eq!(*index, 0);
                assert!(reason.contains("model"));
            }
            _ => panic!("expected InvalidAgentConfig, got: {err:?}"),
        }
    }

    #[test]
    fn validate_missing_api_key_env() {
        let toml = r#"
[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = ""
"#;
        let err = AgentsConfig::parse(toml).unwrap_err();
        match &err {
            ConfigError::InvalidAgentConfig { index, reason } => {
                assert_eq!(*index, 0);
                assert!(reason.contains("api_key_env"));
            }
            _ => panic!("expected InvalidAgentConfig, got: {err:?}"),
        }
    }

    #[test]
    fn validate_duplicate_provider_model() {
        let toml = r#"
[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = "KEY1"

[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = "KEY2"
"#;
        let err = AgentsConfig::parse(toml).unwrap_err();
        match &err {
            ConfigError::InvalidAgentConfig { index, reason } => {
                assert_eq!(*index, 1);
                assert!(reason.contains("duplicate"));
            }
            _ => panic!("expected InvalidAgentConfig, got: {err:?}"),
        }
    }

    #[test]
    fn validate_different_models_same_provider_ok() {
        let toml = r#"
[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = "ANTHROPIC_API_KEY"

[[agents]]
provider = "anthropic"
model = "sonnet-4"
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let config = AgentsConfig::parse(toml).unwrap();
        assert_eq!(config.agents.len(), 2);
    }

    // -----------------------------------------------------------------------
    // AgentConfig methods
    // -----------------------------------------------------------------------

    #[test]
    fn agent_config_name_with_display_name() {
        let agent = AgentConfig {
            provider: "ollama".into(),
            model: "llama3.3".into(),
            api_key_env: "KEY".into(),
            base_url: None,
            display_name: Some("Local Llama".into()),
        };
        assert_eq!(agent.name(), "Local Llama");
    }

    #[test]
    fn agent_config_name_fallback() {
        let agent = AgentConfig {
            provider: "anthropic".into(),
            model: "opus-4".into(),
            api_key_env: "KEY".into(),
            base_url: None,
            display_name: None,
        };
        assert_eq!(agent.name(), "anthropic/opus-4");
    }

    #[test]
    fn agent_config_is_builtin() {
        let anthropic = AgentConfig {
            provider: "anthropic".into(),
            model: "opus-4".into(),
            api_key_env: "KEY".into(),
            base_url: None,
            display_name: None,
        };
        assert!(anthropic.is_builtin_provider());

        let ollama = AgentConfig {
            provider: "ollama".into(),
            model: "llama3.3".into(),
            api_key_env: "KEY".into(),
            base_url: None,
            display_name: None,
        };
        assert!(!ollama.is_builtin_provider());
    }

    // -----------------------------------------------------------------------
    // Defaults from config
    // -----------------------------------------------------------------------

    #[test]
    fn default_agents_from_config_all_providers() {
        let config = default_agents_from_config(
            Some("key1"),
            Some("key2"),
            Some("key3"),
            "opus-4",
            "2.5-pro",
            "o3",
        );
        assert_eq!(config.agents.len(), 3);
        assert_eq!(config.agents[0].provider, "anthropic");
        assert_eq!(config.agents[0].model, "opus-4");
        assert_eq!(config.agents[1].provider, "google");
        assert_eq!(config.agents[1].model, "2.5-pro");
        assert_eq!(config.agents[2].provider, "openai");
        assert_eq!(config.agents[2].model, "o3");
    }

    #[test]
    fn default_agents_from_config_partial() {
        let config =
            default_agents_from_config(Some("key1"), None, None, "sonnet-4", "2.5-pro", "o3");
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].provider, "anthropic");
        assert_eq!(config.agents[0].model, "sonnet-4");
    }

    #[test]
    fn default_agents_from_config_none() {
        let config = default_agents_from_config(None, None, None, "opus-4", "2.5-pro", "o3");
        assert!(config.agents.is_empty());
    }

    // -----------------------------------------------------------------------
    // File I/O
    // -----------------------------------------------------------------------

    #[test]
    fn load_from_file() {
        let dir = std::env::temp_dir().join("ath-agents-config-test-load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agents.toml");
        std::fs::write(
            &path,
            r#"
[[agents]]
provider = "anthropic"
model = "opus-4"
api_key_env = "ANTHROPIC_API_KEY"
"#,
        )
        .unwrap();

        let config = AgentsConfig::load(&path).unwrap();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].provider, "anthropic");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_nonexistent_file() {
        let result = AgentsConfig::load(Path::new("/nonexistent/agents.toml"));
        assert!(matches!(result, Err(ConfigError::FileNotFound { .. })));
    }

    #[test]
    fn load_invalid_toml_file() {
        let dir = std::env::temp_dir().join("ath-agents-config-test-invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agents.toml");
        std::fs::write(&path, "[[[ not valid toml").unwrap();

        let result = AgentsConfig::load(&path);
        assert!(matches!(result, Err(ConfigError::InvalidToml { .. })));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
