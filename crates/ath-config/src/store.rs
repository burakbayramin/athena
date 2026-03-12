//! ConfigStore: layered configuration with merge logic.
//!
//! Precedence (highest to lowest): env vars > project-local > global config.
//! Missing keys gracefully degrade — providers without API keys are simply unavailable.

use std::path::PathBuf;

use crate::env;
use crate::error::ConfigError;
use crate::file::{self, RawFileConfig};

/// Default model for Claude (Anthropic).
const DEFAULT_CLAUDE_MODEL: &str = "opus-4";
/// Default model for Gemini (Google).
const DEFAULT_GEMINI_MODEL: &str = "2.5-pro";
/// Default model for Codex (OpenAI).
const DEFAULT_CODEX_MODEL: &str = "o3";

/// Central configuration store for the Athena orchestrator.
///
/// Merges configuration from three layers (env > project-local > global)
/// and provides access to provider API keys and model defaults.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    /// Anthropic API key, if configured.
    pub anthropic_api_key: Option<String>,
    /// Google API key, if configured.
    pub google_api_key: Option<String>,
    /// OpenAI API key, if configured.
    pub openai_api_key: Option<String>,
    /// Claude model selection (defaults to "opus-4").
    pub claude_model: String,
    /// Gemini model selection (defaults to "2.5-pro").
    pub gemini_model: String,
    /// Codex model selection (defaults to "o3").
    pub codex_model: String,
}

impl ConfigStore {
    /// Load configuration from all sources with correct precedence.
    ///
    /// 1. Global config: `~/.config/ath/config.toml`
    /// 2. Project-local config: `.ath.toml` in current directory
    /// 3. Environment variables (highest precedence)
    ///
    /// Missing files are silently ignored. Missing env vars result in `None`.
    pub fn load() -> Result<Self, ConfigError> {
        let global = match Self::global_config_path() {
            Ok(path) => file::load_config_file(&path).ok(),
            Err(_) => None,
        };

        let local = file::load_config_file(std::path::Path::new(".ath.toml")).ok();

        let env_config = env::load_env_config();

        Ok(Self::load_from_layers(global, local, env_config))
    }

    /// Merge configuration from pre-loaded layers.
    ///
    /// Precedence: env > local > global. For each field, the first `Some` wins
    /// in env-then-local-then-global order. Model fields fall back to defaults.
    pub fn load_from_layers(
        global: Option<RawFileConfig>,
        local: Option<RawFileConfig>,
        env: RawFileConfig,
    ) -> Self {
        let global_ref = global.as_ref();
        let local_ref = local.as_ref();

        // API keys: env > local > global
        let anthropic_api_key = env
            .anthropic_api_key()
            .or_else(|| local_ref.and_then(|c| c.anthropic_api_key()))
            .or_else(|| global_ref.and_then(|c| c.anthropic_api_key()))
            .map(String::from);

        let google_api_key = env
            .google_api_key()
            .or_else(|| local_ref.and_then(|c| c.google_api_key()))
            .or_else(|| global_ref.and_then(|c| c.google_api_key()))
            .map(String::from);

        let openai_api_key = env
            .openai_api_key()
            .or_else(|| local_ref.and_then(|c| c.openai_api_key()))
            .or_else(|| global_ref.and_then(|c| c.openai_api_key()))
            .map(String::from);

        // Model defaults: env > local > global > hardcoded default
        let claude_model = env
            .claude_model()
            .or_else(|| local_ref.and_then(|c| c.claude_model()))
            .or_else(|| global_ref.and_then(|c| c.claude_model()))
            .unwrap_or(DEFAULT_CLAUDE_MODEL)
            .to_string();

        let gemini_model = env
            .gemini_model()
            .or_else(|| local_ref.and_then(|c| c.gemini_model()))
            .or_else(|| global_ref.and_then(|c| c.gemini_model()))
            .unwrap_or(DEFAULT_GEMINI_MODEL)
            .to_string();

        let codex_model = env
            .codex_model()
            .or_else(|| local_ref.and_then(|c| c.codex_model()))
            .or_else(|| global_ref.and_then(|c| c.codex_model()))
            .unwrap_or(DEFAULT_CODEX_MODEL)
            .to_string();

        Self {
            anthropic_api_key,
            google_api_key,
            openai_api_key,
            claude_model,
            gemini_model,
            codex_model,
        }
    }

    /// Resolve the global config file path.
    ///
    /// Returns `~/.config/ath/config.toml` (cross-platform via `dirs::config_dir`).
    fn global_config_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join("ath").join("config.toml"))
    }

    /// Returns the names of providers that have API keys configured.
    pub fn available_providers(&self) -> Vec<&str> {
        let mut providers = Vec::new();
        if self.anthropic_api_key.is_some() {
            providers.push("anthropic");
        }
        if self.google_api_key.is_some() {
            providers.push("google");
        }
        if self.openai_api_key.is_some() {
            providers.push("openai");
        }
        providers
    }

    /// Returns true if at least one provider has an API key configured.
    pub fn has_any_provider(&self) -> bool {
        self.anthropic_api_key.is_some()
            || self.google_api_key.is_some()
            || self.openai_api_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::{DefaultsConfig, ProvidersConfig, RawFileConfig};

    fn make_config(
        anthropic: Option<&str>,
        google: Option<&str>,
        openai: Option<&str>,
        claude_model: Option<&str>,
    ) -> RawFileConfig {
        RawFileConfig {
            providers: Some(ProvidersConfig {
                anthropic_api_key: anthropic.map(String::from),
                google_api_key: google.map(String::from),
                openai_api_key: openai.map(String::from),
            }),
            defaults: claude_model.map(|m| DefaultsConfig {
                claude_model: Some(m.to_string()),
                gemini_model: None,
                codex_model: None,
            }),
        }
    }

    fn empty_config() -> RawFileConfig {
        RawFileConfig::default()
    }

    #[test]
    fn merge_all_three_layers() {
        let global = Some(make_config(
            Some("global-ant"),
            Some("global-goog"),
            Some("global-oai"),
            Some("sonnet-4"),
        ));
        let local = Some(make_config(
            Some("local-ant"),
            None,
            Some("local-oai"),
            None,
        ));
        let env = make_config(Some("env-ant"), None, None, None);

        let store = ConfigStore::load_from_layers(global, local, env);

        // env wins for anthropic
        assert_eq!(store.anthropic_api_key.as_deref(), Some("env-ant"));
        // local wins for google (env has None, local has None, so global wins)
        assert_eq!(store.google_api_key.as_deref(), Some("global-goog"));
        // local wins for openai (env has None)
        assert_eq!(store.openai_api_key.as_deref(), Some("local-oai"));
        // global model set, no local/env override
        assert_eq!(store.claude_model, "sonnet-4");
    }

    #[test]
    fn env_overrides_project_local() {
        let local = Some(make_config(Some("local-key"), None, None, None));
        let env = make_config(Some("env-key"), None, None, None);

        let store = ConfigStore::load_from_layers(None, local, env);
        assert_eq!(store.anthropic_api_key.as_deref(), Some("env-key"));
    }

    #[test]
    fn project_local_overrides_global() {
        let global = Some(make_config(Some("global-key"), None, None, None));
        let local = Some(make_config(Some("local-key"), None, None, None));
        let env = empty_config();

        let store = ConfigStore::load_from_layers(global, local, env);
        assert_eq!(store.anthropic_api_key.as_deref(), Some("local-key"));
    }

    #[test]
    fn no_config_all_none_no_panic() {
        let store = ConfigStore::load_from_layers(None, None, empty_config());
        assert!(store.anthropic_api_key.is_none());
        assert!(store.google_api_key.is_none());
        assert!(store.openai_api_key.is_none());
        // Defaults still apply
        assert_eq!(store.claude_model, "opus-4");
        assert_eq!(store.gemini_model, "2.5-pro");
        assert_eq!(store.codex_model, "o3");
    }

    #[test]
    fn available_providers_returns_configured_only() {
        let env = make_config(Some("ant-key"), None, Some("oai-key"), None);
        let store = ConfigStore::load_from_layers(None, None, env);

        let providers = store.available_providers();
        assert_eq!(providers, vec!["anthropic", "openai"]);
        assert!(store.has_any_provider());
    }

    #[test]
    fn zero_providers_graceful_degradation() {
        let store = ConfigStore::load_from_layers(None, None, empty_config());
        assert!(store.available_providers().is_empty());
        assert!(!store.has_any_provider());
    }

    #[test]
    fn model_defaults_when_not_specified() {
        let store = ConfigStore::load_from_layers(None, None, empty_config());
        assert_eq!(store.claude_model, "opus-4");
        assert_eq!(store.gemini_model, "2.5-pro");
        assert_eq!(store.codex_model, "o3");
    }

    #[test]
    fn model_override_from_global() {
        let global = Some(RawFileConfig {
            providers: None,
            defaults: Some(DefaultsConfig {
                claude_model: Some("sonnet-4".to_string()),
                gemini_model: Some("2.0-flash".to_string()),
                codex_model: None,
            }),
        });

        let store = ConfigStore::load_from_layers(global, None, empty_config());
        assert_eq!(store.claude_model, "sonnet-4");
        assert_eq!(store.gemini_model, "2.0-flash");
        assert_eq!(store.codex_model, "o3"); // default fallback
    }
}
