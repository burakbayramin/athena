//! Context injection for agent prompts.
//!
//! Reads from [`VikingStore`] and [`KeywordIndex`] to build a budget-constrained
//! context string that gets prepended to agent prompts. Each section (project
//! identity, semantic search results, agent notes, recent run summary) has a
//! configurable token budget allocation.
//!
//! All reads are fail-soft — missing URIs produce empty sections, never errors.

mod injector;

pub use injector::{ContextInjector, InjectedContext, InjectionConfig};
