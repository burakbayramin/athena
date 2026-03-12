//! # ath-planner
//!
//! Phase planning and DAG decomposition.
//!
//! Analyzes project specifications and decomposes them into
//! dependency-aware phases with correct agent assignments
//! and parallelization opportunities.

pub mod input;
pub mod decompose;
