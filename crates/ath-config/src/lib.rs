//! # ath-config
//!
//! Configuration loading and management for the Athena orchestrator.
//!
//! Supports a layered config model:
//! - Global config at `~/.config/ath/config.toml`
//! - Project-local override at `.ath.toml`
//! - Environment variables (for CI)
//! - CLI flags (highest precedence)
//!
//! Precedence: CLI flags > env vars > project-local > global config

pub mod agents;
pub mod env;
pub mod error;
pub mod file;
pub mod store;

// Re-export primary types at crate root for convenience.
pub use agents::{AgentConfig, AgentsConfig};
pub use error::ConfigError;
pub use store::ConfigStore;

// Validate the ath-types dependency is wired correctly.
use ath_types as _;
