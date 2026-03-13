//! Core memory content types.
//!
//! `LayeredContent` represents knowledge stored at three levels of detail:
//! - L0 (`abstract_text`): A brief abstract or summary (1-2 sentences)
//! - L1 (`overview_text`): A structured overview (key points, relationships)
//! - L2 (`detail`): Full detail text, optional (may be omitted for small entries)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::uri::VikingUri;

/// Knowledge content stored at three layers of detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayeredContent {
    /// The Viking URI identifying this content.
    pub uri: VikingUri,

    /// L0: Brief abstract (1-2 sentences).
    pub abstract_text: String,

    /// L1: Structured overview with key points.
    pub overview_text: String,

    /// L2: Full detail text. `None` for entries where L1 is sufficient.
    pub detail: Option<String>,

    /// When this content was first created.
    pub created_at: DateTime<Utc>,

    /// When this content was last updated.
    pub updated_at: DateTime<Utc>,
}

impl LayeredContent {
    /// Creates a new `LayeredContent` with timestamps set to now.
    pub fn new(uri: VikingUri, abstract_text: String, overview_text: String) -> Self {
        let now = Utc::now();
        Self {
            uri,
            abstract_text,
            overview_text,
            detail: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Sets the L2 detail text.
    pub fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// A search result from the memory index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    /// The URI of the matched content.
    pub uri: VikingUri,

    /// Similarity score (0.0 to 1.0, higher is more similar).
    pub score: f32,

    /// The matched content.
    pub content: LayeredContent,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_content() -> LayeredContent {
        let uri: VikingUri = "viking://project/conventions".parse().unwrap();
        LayeredContent::new(
            uri,
            "Project follows hexagonal architecture.".to_string(),
            "## Conventions\n- Hexagonal architecture\n- Error types use hint() method".to_string(),
        )
        .with_detail("Full detail about project conventions...".to_string())
    }

    #[test]
    fn layered_content_serde_round_trip() {
        let content = sample_content();
        let json = serde_json::to_string_pretty(&content).unwrap();
        let deserialized: LayeredContent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.uri.to_string(), "viking://project/conventions");
        assert_eq!(deserialized.abstract_text, content.abstract_text);
        assert_eq!(deserialized.overview_text, content.overview_text);
        assert_eq!(deserialized.detail, content.detail);
        // Timestamps survive round-trip.
        assert_eq!(deserialized.created_at, content.created_at);
        assert_eq!(deserialized.updated_at, content.updated_at);
    }

    #[test]
    fn layered_content_without_detail() {
        let uri: VikingUri = "viking://small/entry".parse().unwrap();
        let content = LayeredContent::new(uri, "Small.".into(), "Overview.".into());
        assert!(content.detail.is_none());

        let json = serde_json::to_string(&content).unwrap();
        let deserialized: LayeredContent = serde_json::from_str(&json).unwrap();
        assert!(deserialized.detail.is_none());
    }

    #[test]
    fn memory_hit_serde_round_trip() {
        let content = sample_content();
        let hit = MemoryHit {
            uri: content.uri.clone(),
            score: 0.95,
            content,
        };

        let json = serde_json::to_string(&hit).unwrap();
        let deserialized: MemoryHit = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.score, 0.95);
        assert_eq!(
            deserialized.uri.to_string(),
            "viking://project/conventions"
        );
    }
}
