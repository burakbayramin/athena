//! # ath-agents
//!
//! Agent dispatch and communication layer.
//!
//! This crate provides the interface between the orchestrator
//! and LLM providers (Anthropic, Google, OpenAI) via a unified
//! agent abstraction.

pub mod actor;
pub mod backend;
pub mod circuit_breaker;
pub mod error;
pub mod mock;

pub use backend::AgentBackend;
pub use circuit_breaker::CircuitBreaker;
pub use error::AgentError;
pub use mock::MockBackend;
