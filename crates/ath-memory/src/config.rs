//! TOML-based configuration for the memory subsystem.
//!
//! Loads from `.ath/memory/config.toml`. All fields default gracefully —
//! a missing or malformed file produces usable defaults with a warning.

use std::path::Path;

use serde::Deserialize;
use tracing::instrument;

use crate::extract::ExtractionConfig;
use crate::inject::InjectionConfig;

/// Top-level memory configuration, loaded from `.ath/memory/config.toml`.
///
/// All sections are optional and default independently.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Context injection budget settings.
    pub injection: InjectionConfig,
    /// Extraction pipeline settings.
    pub extraction: ExtractionConfig,
    /// Garbage collection settings.
    pub gc: GcConfig,
}

impl MemoryConfig {
    /// Load configuration from `.ath/memory/config.toml` under the given project root.
    ///
    /// Returns `Default` if the file is missing. Logs a warning and returns
    /// `Default` if the file exists but cannot be parsed.
    #[instrument(skip_all, fields(root = %root.display()))]
    pub fn load(root: &Path) -> Self {
        let config_path = root.join(".ath").join("memory").join("config.toml");

        let raw = match std::fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = %config_path.display(), "config file not found, using defaults");
                return Self::default();
            }
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "failed to read config file, using defaults"
                );
                return Self::default();
            }
        };

        match toml::from_str(&raw) {
            Ok(config) => {
                tracing::info!(path = %config_path.display(), "loaded memory config");
                config
            }
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "failed to parse config file, using defaults"
                );
                Self::default()
            }
        }
    }
}

/// Garbage collection settings for observation cleanup.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GcConfig {
    /// Number of days to retain observation files before GC.
    pub retention_days: u32,
    /// Optional cap on the number of observation files to keep.
    pub max_observation_files: Option<usize>,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            max_observation_files: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn config_deserialize_full_toml() {
        let toml_str = r#"
[injection]
total_budget = 8000
project_identity = 400
semantic_results = 5000
agent_notes = 600
recent_run = 1000

[extraction]
max_observation_count = 200
max_serialization_bytes = 100000

[gc]
retention_days = 14
max_observation_files = 500
"#;
        let config: MemoryConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.injection.total_budget, 8000);
        assert_eq!(config.injection.project_identity, 400);
        assert_eq!(config.extraction.max_observation_count, 200);
        assert_eq!(config.extraction.max_serialization_bytes, 100_000);
        assert_eq!(config.gc.retention_days, 14);
        assert_eq!(config.gc.max_observation_files, Some(500));
    }

    #[test]
    fn config_deserialize_partial_only_gc() {
        let toml_str = r#"
[gc]
retention_days = 7
"#;
        let config: MemoryConfig = toml::from_str(toml_str).unwrap();
        // gc overridden
        assert_eq!(config.gc.retention_days, 7);
        assert_eq!(config.gc.max_observation_files, None);
        // injection and extraction should be defaults
        assert_eq!(config.injection.total_budget, 4000);
        assert_eq!(config.extraction.max_observation_count, 100);
    }

    #[test]
    fn config_deserialize_empty_toml_uses_defaults() {
        let config: MemoryConfig = toml::from_str("").unwrap();
        assert_eq!(config.injection.total_budget, 4000);
        assert_eq!(config.extraction.max_observation_count, 100);
        assert_eq!(config.gc.retention_days, 30);
        assert_eq!(config.gc.max_observation_files, None);
    }

    #[test]
    fn config_load_missing_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let config = MemoryConfig::load(dir.path());
        assert_eq!(config.gc.retention_days, 30);
        assert_eq!(config.injection.total_budget, 4000);
    }

    #[test]
    fn config_load_malformed_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().join(".ath").join("memory");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "{{{{ not valid toml").unwrap();

        let config = MemoryConfig::load(dir.path());
        assert_eq!(config.gc.retention_days, 30);
        assert_eq!(config.injection.total_budget, 4000);
    }

    #[test]
    fn config_load_valid_file() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().join(".ath").join("memory");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.toml"),
            "[gc]\nretention_days = 60\n",
        )
        .unwrap();

        let config = MemoryConfig::load(dir.path());
        assert_eq!(config.gc.retention_days, 60);
        // Other sections should still be defaults
        assert_eq!(config.injection.total_budget, 4000);
    }

    #[test]
    fn gc_config_defaults() {
        let gc = GcConfig::default();
        assert_eq!(gc.retention_days, 30);
        assert_eq!(gc.max_observation_files, None);
    }
}
