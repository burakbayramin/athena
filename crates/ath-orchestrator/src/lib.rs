//! # ath-orchestrator
//!
//! Multi-agent orchestration engine.
//!
//! The conductor: dispatches agents, manages phase execution,
//! collects results, triggers reviews, and maintains the
//! complete audit trail of a project run.

pub mod coordinator;
pub mod error;
pub mod isolation;
pub mod phase_runner;
pub mod review;
pub mod router;
pub mod taxonomy;
