//! # ath-memory
//!
//! Viking memory subsystem for the Athena orchestrator.
//!
//! Provides a layered knowledge store backed by `viking://` URIs,
//! filesystem persistence, and vector-indexed search.
//!
//! ## Core Types
//!
//! - [`VikingUri`] — Parsed `viking://` URI with path traversal protection
//! - [`LayeredContent`] — Knowledge stored at three detail levels (L0/L1/L2)
//! - [`MemoryHit`] — Search result with similarity score
//! - [`MemoryIndex`] — HNSW vector index with cosine similarity
//! - [`KeywordIndex`] — Inverted index for keyword fallback search
//! - [`MemoryError`] — Error enum with actionable [`MemoryError::hint()`] messages

pub mod config;
pub mod error;
pub mod extract;
pub mod index;
pub mod inject;
pub mod keyword;
pub mod observe;
pub mod store;
pub mod types;
pub mod uri;

// Re-export primary types at crate root for convenience.
pub use config::{GcConfig, MemoryConfig};
pub use error::MemoryError;
pub use index::{MemoryIndex, SearchResult};
pub use keyword::{KeywordHit, KeywordIndex};
pub use store::VikingStore;
pub use types::{LayeredContent, MemoryHit};
pub use observe::{
    FileOpKind, Observation, ObservationBuffer, ObservationReader, ObservationType,
    ObservationWriter,
};
pub use extract::{
    ExtractionConfig, ExtractionLlm, ExtractionResult, MemoryExtractor, RunSummary,
};
pub use inject::{ContextInjector, InjectedContext, InjectionConfig};
pub use uri::VikingUri;
