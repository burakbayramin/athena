//! Configuration error types with actionable fix hints.

/// Errors that can occur during configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Config file not found at the given path.
    #[error("Config file not found: {path}")]
    FileNotFound {
        /// The path that was searched.
        path: String,
    },

    /// The TOML content is invalid or cannot be parsed.
    #[error("Invalid TOML in config file")]
    InvalidToml {
        /// The underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },

    /// Could not determine the platform config directory.
    #[error("Could not determine config directory")]
    NoConfigDir,

    /// Invalid agent configuration entry.
    #[error("Invalid agent config at index {index}: {reason}")]
    InvalidAgentConfig {
        /// Zero-based index of the agent entry.
        index: usize,
        /// What's wrong with it.
        reason: String,
    },
}

impl ConfigError {
    /// Returns an actionable hint for resolving this error.
    pub fn hint(&self) -> &str {
        match self {
            ConfigError::FileNotFound { .. } => {
                "Run `ath init` to create a config file"
            }
            ConfigError::InvalidToml { .. } => {
                "Check your config file for TOML syntax errors (mismatched quotes, missing brackets)"
            }
            ConfigError::NoConfigDir => {
                "Set the HOME or XDG_CONFIG_HOME environment variable"
            }
            ConfigError::InvalidAgentConfig { .. } => {
                "Check .ath/agents.toml — each [[agents]] entry needs provider, model, and api_key_env"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_not_found_displays_path() {
        let err = ConfigError::FileNotFound {
            path: "/home/user/.config/ath/config.toml".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/home/user/.config/ath/config.toml"));
    }

    #[test]
    fn invalid_toml_has_source() {
        let bad_toml = "[[[ invalid";
        let parse_err = toml::from_str::<toml::Value>(bad_toml).unwrap_err();
        let err = ConfigError::InvalidToml { source: parse_err };
        let msg = format!("{err}");
        assert!(msg.contains("Invalid TOML"));
        // source chain works
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn no_config_dir_message() {
        let err = ConfigError::NoConfigDir;
        assert_eq!(format!("{err}"), "Could not determine config directory");
    }

    #[test]
    fn hints_are_actionable() {
        let err1 = ConfigError::FileNotFound { path: "x".into() };
        assert!(err1.hint().contains("ath init"));

        let bad_toml = "[[[ invalid";
        let parse_err = toml::from_str::<toml::Value>(bad_toml).unwrap_err();
        let err2 = ConfigError::InvalidToml { source: parse_err };
        assert!(err2.hint().contains("syntax"));

        let err3 = ConfigError::NoConfigDir;
        assert!(err3.hint().contains("HOME"));
    }
}
