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

pub mod agent;
pub mod error;
pub mod phase;
pub mod plan;
pub mod project;
pub mod report;
pub mod review;

// Re-export key types at crate root for ergonomic imports
pub use agent::{AgentKind, AgentRequest, AgentResponse};
pub use error::ValidationError;
pub use phase::{AgentContribution, PhaseRecord, ReviewAttempt, TokenUsage};
pub use plan::{ContractLabel, ExecutionPlan, PhaseSpec, TaskSpec};
pub use project::{GoalSpec, ProjectSpec, SkillTag};
pub use report::{AgentTotals, PhaseSummary, ReportTotals, RunReport, RunTotals};
pub use review::{CodeSuggestion, ReviewVerdict, Severity};
