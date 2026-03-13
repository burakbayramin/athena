//! Observation capture layer for the memory subsystem.
//!
//! Provides typed observation events, an in-memory buffer, and append-only
//! JSONL storage for persisting execution observations across runs.
//!
//! ## Architecture
//!
//! - [`ObservationType`] — Internally tagged enum of 7 event variants
//! - [`Observation`] — Wrapper with identity, run correlation, and timestamp
//! - [`ObservationBuffer`] — Thread-safe in-memory collector with cap enforcement
//! - [`ObservationWriter`] — Append-only JSONL file writer (one file per run)
//! - [`ObservationReader`] — JSONL reader for post-run hydration

pub mod buffer;
pub mod storage;
pub mod types;

pub use buffer::ObservationBuffer;
pub use storage::{ObservationReader, ObservationWriter};
pub use types::{FileOpKind, Observation, ObservationType};
