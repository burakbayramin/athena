//! # ath-types
//!
//! Shared type system for the Athena multi-agent orchestrator.
//!
//! This crate defines the core domain types used across all ath-* crates:
//! - Agent identification and model variants
//! - Project specifications and goals
//! - Inter-agent request/response schemas
//! - Review verdicts and severity levels
//! - Phase records and audit trails
//! - Unified validation error types

pub mod error;
pub mod agent;
pub mod project;
pub mod review;
pub mod phase;

// Re-export key types at crate root for ergonomic imports
pub use error::ValidationError;
pub use agent::{AgentKind, AgentRequest, AgentResponse};
pub use project::{ProjectSpec, GoalSpec, SkillTag};
pub use review::{ReviewVerdict, Severity, CodeSuggestion};
pub use phase::{PhaseRecord, ReviewAttempt, TokenUsage, AgentContribution};
