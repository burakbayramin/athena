//! # ath-agents
//!
//! Agent dispatch and communication layer.
//!
//! This crate provides the interface between the orchestrator
//! and LLM providers (Anthropic, Google, OpenAI) via a unified
//! agent abstraction.

pub mod circuit_breaker;
pub mod error;

pub use circuit_breaker::CircuitBreaker;
pub use error::AgentError;
