//! Phase decomposition: DAG algorithms and error types.
//!
//! This module provides the core graph algorithms for validating and analyzing
//! phase dependency graphs, as well as the error types for decomposition failures.

pub mod dag;
pub mod error;
pub mod prompt;
pub mod validate;
