//! Memory extraction pipeline.
//!
//! Converts raw observations from a completed run into structured memory
//! entries (run summaries, conventions, decisions, agent profiles) using
//! an LLM via the [`ExtractionLlm`] trait.
//!
//! ## Architecture
//!
//! The `ExtractionLlm` trait is defined here (not in `ath-agents`) to avoid
//! coupling `ath-memory` to provider infrastructure. The orchestrator bridges
//! `AgentBackend` → `ExtractionLlm` at the integration layer.
//!
//! ## Pipeline
//!
//! ```text
//! Observations → preprocess → prompt → LLM → parse → store → index
//! ```

pub mod pipeline;
pub mod prompts;
pub mod types;

pub use pipeline::MemoryExtractor;
pub use types::{
    AgentProfileUpdate, Convention, Decision, ExtractionConfig, ExtractionLlm, ExtractionResult,
    RunSummary,
};
