//! Environment variable loading for configuration.

use crate::file::{ProvidersConfig, RawFileConfig};

/// Load configuration from environment variables.
///
/// This function is infallible — missing environment variables result in `None`
/// fields, not errors. Calls `dotenvy::dotenv().ok()` to optionally load a
/// `.env` file if present.
pub fn load_env_config() -> RawFileConfig {
    // Attempt to load .env file; ignore errors (file is optional)
    dotenvy::dotenv().ok();

    let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let google_api_key = std::env::var("GOOGLE_API_KEY").ok();
    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();

    let has_providers =
        anthropic_api_key.is_some() || google_api_key.is_some() || openai_api_key.is_some();

    RawFileConfig {
        providers: if has_providers {
            Some(ProvidersConfig {
                anthropic_api_key,
                google_api_key,
                openai_api_key,
            })
        } else {
            None
        },
        defaults: None, // Model defaults are not set via env vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to prevent parallel env var tests from interfering
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env_vars<F, R>(vars: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _lock = ENV_MUTEX.lock().unwrap();

        // Save and set
        let saved: Vec<_> = vars
            .iter()
            .map(|(key, val)| {
                let old = std::env::var(key).ok();
                match val {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
                (*key, old)
            })
            .collect();

        let result = f();

        // Restore
        for (key, old) in saved {
            match old {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }

        result
    }

    #[test]
    fn reads_all_api_keys_from_env() {
        with_env_vars(
            &[
                ("ANTHROPIC_API_KEY", Some("sk-ant-test")),
                ("GOOGLE_API_KEY", Some("AIza-test")),
                ("OPENAI_API_KEY", Some("sk-test")),
            ],
            || {
                let config = load_env_config();
                assert_eq!(config.anthropic_api_key(), Some("sk-ant-test"));
                assert_eq!(config.google_api_key(), Some("AIza-test"));
                assert_eq!(config.openai_api_key(), Some("sk-test"));
            },
        );
    }

    #[test]
    fn missing_env_vars_result_in_none() {
        with_env_vars(
            &[
                ("ANTHROPIC_API_KEY", None),
                ("GOOGLE_API_KEY", None),
                ("OPENAI_API_KEY", None),
            ],
            || {
                let config = load_env_config();
                assert_eq!(config.anthropic_api_key(), None);
                assert_eq!(config.google_api_key(), None);
                assert_eq!(config.openai_api_key(), None);
            },
        );
    }

    #[test]
    fn partial_env_vars_work() {
        with_env_vars(
            &[
                ("ANTHROPIC_API_KEY", Some("only-this-one")),
                ("GOOGLE_API_KEY", None),
                ("OPENAI_API_KEY", None),
            ],
            || {
                let config = load_env_config();
                assert_eq!(config.anthropic_api_key(), Some("only-this-one"));
                assert_eq!(config.google_api_key(), None);
                assert_eq!(config.openai_api_key(), None);
            },
        );
    }

    #[test]
    fn defaults_section_is_none() {
        with_env_vars(
            &[
                ("ANTHROPIC_API_KEY", None),
                ("GOOGLE_API_KEY", None),
                ("OPENAI_API_KEY", None),
            ],
            || {
                let config = load_env_config();
                assert!(config.defaults.is_none());
            },
        );
    }
}
