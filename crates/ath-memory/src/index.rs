//! HNSW-based vector index for memory search.
//!
//! Wraps the [`hnsw`] crate to provide cosine-similarity vector search
//! with URI string keys. Supports persistence via serde serialization:
//! the stored vectors and mappings are saved to JSON, and the HNSW graph
//! is rebuilt from scratch on load (fast for typical index sizes).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Utc;
use rand_pcg::Pcg64;
use serde::{Deserialize, Serialize};
use tracing::{info_span, instrument};

use crate::error::MemoryError;

// ── Cosine distance metric ──────────────────────────────────────────────────

/// Cosine distance metric for use with the `hnsw` crate.
///
/// Returns `1.0 - cosine_similarity` mapped to `u32` via float-to-bits
/// so that the metric is unsigned and orderable as required by `space::Metric`.
#[derive(Clone)]
struct CosineDistance;

impl space::Metric<Vec<f32>> for CosineDistance {
    type Unit = u32;

    fn distance(&self, a: &Vec<f32>, b: &Vec<f32>) -> u32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        let denom = norm_a * norm_b;
        let similarity = if denom > 0.0 { dot / denom } else { 0.0 };
        // Clamp to [0, 2] range (cosine distance).
        let distance = (1.0 - similarity).clamp(0.0, 2.0);
        // Convert to u32 via float bits for ordering.
        // Positive f32 values have the same ordering as their u32 bit representations.
        distance.to_bits()
    }
}

// ── HNSW type aliases ───────────────────────────────────────────────────────

/// M = 12 connections per layer, M0 = 24 connections on layer 0.
/// Standard HNSW parameters for reasonable recall/speed tradeoff.
type HnswGraph = hnsw::Hnsw<CosineDistance, Vec<f32>, Pcg64, 12, 24>;

// ── MemoryIndex ─────────────────────────────────────────────────────────────

/// HNSW vector index with cosine similarity and string keys.
///
/// Each entry maps a URI string to a dense f32 vector. Search returns
/// the nearest neighbours ranked by cosine distance.
///
/// `Debug` is manually implemented because the underlying HNSW graph
/// does not derive it.
pub struct MemoryIndex {
    graph: HnswGraph,
    dimension: usize,
    /// Maps URI string → HNSW internal ID.
    uri_to_id: HashMap<String, usize>,
    /// Maps HNSW internal ID → URI string.
    id_to_uri: HashMap<usize, String>,
    /// Stored vectors keyed by URI (for persistence and rebuild).
    vectors: HashMap<String, Vec<f32>>,
    /// Tracks soft-deleted IDs (the `hnsw` crate doesn't support deletion,
    /// so we filter them out during search).
    deleted: HashSet<usize>,
}

impl std::fmt::Debug for MemoryIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryIndex")
            .field("dimension", &self.dimension)
            .field("live_entries", &self.uri_to_id.len())
            .field("graph_entries", &self.graph.len())
            .field("deleted", &self.deleted.len())
            .finish()
    }
}

impl MemoryIndex {
    /// Creates a new empty index.
    ///
    /// - `dimension`: the dimensionality of vectors (e.g. 384 for MiniLM).
    /// - `_max_nodes`: reserved for API compatibility (the `hnsw` crate grows dynamically).
    pub fn new(dimension: usize, _max_nodes: usize) -> Self {
        let graph = HnswGraph::new(CosineDistance);
        Self {
            graph,
            dimension,
            uri_to_id: HashMap::new(),
            id_to_uri: HashMap::new(),
            vectors: HashMap::new(),
            deleted: HashSet::new(),
        }
    }

    /// Insert or update a vector for the given URI string.
    ///
    /// If the key already exists, the old entry is soft-deleted and a new
    /// one is inserted (upsert semantics).
    #[instrument(skip(self, vector), fields(dim = self.dimension))]
    pub fn upsert(&mut self, uri_str: &str, vector: &[f32]) -> Result<(), MemoryError> {
        if vector.len() != self.dimension {
            return Err(MemoryError::DimensionMismatch {
                expected: self.dimension,
                actual: vector.len(),
            });
        }

        // If key already exists, soft-delete the old graph entry.
        if let Some(&old_id) = self.uri_to_id.get(uri_str) {
            self.deleted.insert(old_id);
            self.id_to_uri.remove(&old_id);
        }

        let vec_owned = vector.to_vec();
        let mut searcher = hnsw::Searcher::default();
        let id = self.graph.insert(vec_owned.clone(), &mut searcher);

        self.uri_to_id.insert(uri_str.to_string(), id);
        self.id_to_uri.insert(id, uri_str.to_string());
        self.vectors.insert(uri_str.to_string(), vec_owned);

        tracing::debug!(uri = uri_str, id, "upserted vector");
        Ok(())
    }

    /// Search for the `top_k` nearest vectors to the query.
    ///
    /// Returns results ordered by similarity (closest first).
    /// Cosine distance is converted to a similarity score: `1.0 - distance`.
    ///
    /// Returns an empty vec if the index is empty.
    pub fn search(
        &self,
        query_vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let _span = info_span!(
            "index_search",
            dim = self.dimension,
            top_k,
            index_len = self.live_len()
        )
        .entered();

        if query_vector.len() != self.dimension {
            return Err(MemoryError::DimensionMismatch {
                expected: self.dimension,
                actual: query_vector.len(),
            });
        }

        if self.is_empty() {
            return Ok(Vec::new());
        }

        // Request extra results to compensate for soft-deleted entries.
        // Cap at graph.len() because the hnsw crate panics if dest > items.
        let graph_len = self.graph.len();
        let ef = (top_k + self.deleted.len())
            .max(top_k * 2)
            .min(200)
            .min(graph_len);
        let query = query_vector.to_vec();
        let mut dest = vec![
            space::Neighbor {
                index: 0,
                distance: u32::MAX,
            };
            ef
        ];
        let mut searcher = hnsw::Searcher::default();

        let neighbors = self.graph.nearest(&query, ef, &mut searcher, &mut dest);

        let results: Vec<SearchResult> = neighbors
            .iter()
            .filter(|n| !self.deleted.contains(&n.index))
            .filter_map(|n| {
                let uri = self.id_to_uri.get(&n.index)?;
                let distance = f32::from_bits(n.distance);
                Some(SearchResult {
                    uri_str: uri.clone(),
                    score: 1.0 - distance,
                })
            })
            .take(top_k)
            .collect();

        tracing::debug!(results_count = results.len(), "search complete");
        Ok(results)
    }

    /// Soft-delete a key from the index.
    ///
    /// The key won't appear in search results but the vector data remains
    /// in the graph until a rebuild.
    pub fn delete(&mut self, uri_str: &str) -> bool {
        if let Some(&id) = self.uri_to_id.get(uri_str) {
            self.deleted.insert(id);
            self.uri_to_id.remove(uri_str);
            self.id_to_uri.remove(&id);
            self.vectors.remove(uri_str);
            true
        } else {
            false
        }
    }

    /// Persist the index to a directory.
    ///
    /// Writes two files:
    /// - `index.json` — vectors and URI mappings (graph is rebuilt on load)
    /// - `meta.json` — metadata (dimension, count, timestamps)
    #[instrument(skip(self), fields(dim = self.dimension, len = self.live_len()))]
    pub fn save(&self, dir: &Path) -> Result<(), MemoryError> {
        std::fs::create_dir_all(dir).map_err(|e| MemoryError::IoError {
            path: dir.display().to_string(),
            message: "failed to create index directory".to_string(),
            source: e,
        })?;

        // Save vectors and mappings (graph is rebuilt on load from these).
        let data = IndexData {
            vectors: &self.vectors,
        };
        let index_path = dir.join("index.json");
        let json = serde_json::to_string(&data).map_err(|e| MemoryError::SerializationError {
            message: format!("failed to serialize index: {}", e),
            source: Some(e),
        })?;
        std::fs::write(&index_path, &json).map_err(|e| MemoryError::IoError {
            path: index_path.display().to_string(),
            message: "failed to write index.json".to_string(),
            source: e,
        })?;

        // Save metadata.
        let meta = IndexMeta {
            dimension: self.dimension,
            count: self.live_len(),
            created_at: Utc::now().to_rfc3339(),
        };
        let meta_path = dir.join("meta.json");
        let meta_json =
            serde_json::to_string_pretty(&meta).map_err(|e| MemoryError::SerializationError {
                message: format!("failed to serialize meta.json: {}", e),
                source: Some(e),
            })?;
        std::fs::write(&meta_path, meta_json).map_err(|e| MemoryError::IoError {
            path: meta_path.display().to_string(),
            message: "failed to write meta.json".to_string(),
            source: e,
        })?;

        tracing::info!(dir = %dir.display(), "index saved");
        Ok(())
    }

    /// Load an index from a directory previously written by [`save`](Self::save).
    ///
    /// The `expected_dimension` is checked against the stored metadata to
    /// catch model changes early. The HNSW graph is rebuilt from the stored
    /// vectors (fast for typical index sizes).
    #[instrument(fields(expected_dim = expected_dimension))]
    pub fn load(dir: &Path, expected_dimension: usize) -> Result<Self, MemoryError> {
        // Load and validate metadata.
        let meta_path = dir.join("meta.json");
        let meta_raw = std::fs::read_to_string(&meta_path).map_err(|e| MemoryError::IoError {
            path: meta_path.display().to_string(),
            message: "failed to read meta.json".to_string(),
            source: e,
        })?;
        let meta: IndexMeta =
            serde_json::from_str(&meta_raw).map_err(|e| MemoryError::SerializationError {
                message: format!("failed to parse meta.json: {}", e),
                source: Some(e),
            })?;

        if meta.dimension != expected_dimension {
            return Err(MemoryError::DimensionMismatch {
                expected: expected_dimension,
                actual: meta.dimension,
            });
        }

        // Load vectors.
        let index_path = dir.join("index.json");
        let index_raw = std::fs::read_to_string(&index_path).map_err(|e| MemoryError::IoError {
            path: index_path.display().to_string(),
            message: "failed to read index.json".to_string(),
            source: e,
        })?;
        let data: IndexDataOwned =
            serde_json::from_str(&index_raw).map_err(|e| MemoryError::SerializationError {
                message: format!("failed to parse index.json: {}", e),
                source: Some(e),
            })?;

        // Rebuild graph from vectors.
        let mut graph = HnswGraph::new(CosineDistance);
        let mut uri_to_id = HashMap::new();
        let mut id_to_uri = HashMap::new();
        let mut searcher = hnsw::Searcher::default();

        for (uri, vector) in &data.vectors {
            let id = graph.insert(vector.clone(), &mut searcher);
            uri_to_id.insert(uri.clone(), id);
            id_to_uri.insert(id, uri.clone());
        }

        tracing::info!(
            dir = %dir.display(),
            dimension = meta.dimension,
            count = meta.count,
            "index loaded (graph rebuilt)"
        );

        Ok(Self {
            graph,
            dimension: meta.dimension,
            uri_to_id,
            id_to_uri,
            vectors: data.vectors,
            deleted: HashSet::new(),
        })
    }

    /// Number of entries ever inserted into the graph (including soft-deleted).
    pub fn len(&self) -> usize {
        self.graph.len()
    }

    /// Number of live (non-deleted) entries.
    pub fn live_len(&self) -> usize {
        self.uri_to_id.len()
    }

    /// Returns `true` if the index contains no live entries.
    pub fn is_empty(&self) -> bool {
        self.uri_to_id.is_empty()
    }

    /// Returns the configured vector dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

/// A search result from the vector index (before joining with store content).
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The URI string of the matched entry.
    pub uri_str: String,
    /// Cosine similarity score (0.0 to 1.0, higher is more similar).
    pub score: f32,
}

/// Metadata persisted alongside the index files.
#[derive(Debug, Serialize, Deserialize)]
struct IndexMeta {
    dimension: usize,
    count: usize,
    created_at: String,
}

/// Serialization wrapper for index data (borrows).
#[derive(Serialize)]
struct IndexData<'a> {
    vectors: &'a HashMap<String, Vec<f32>>,
}

/// Deserialization wrapper for index data (owns).
#[derive(Deserialize)]
struct IndexDataOwned {
    vectors: HashMap<String, Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Generate a unit vector pointing along one axis, for predictable search.
    fn axis_vector(dim: usize, axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[axis % dim] = 1.0;
        v
    }

    #[test]
    fn insert_and_search() {
        let dim = 8;
        let mut index = MemoryIndex::new(dim, 100);

        // Insert 10 synthetic vectors, each pointing along a different axis.
        for i in 0..8 {
            let v = axis_vector(dim, i);
            index.upsert(&format!("viking://entry/{i}"), &v).unwrap();
        }

        // Search for vector near axis 3 — should return entry/3 as closest.
        let mut query = vec![0.0f32; dim];
        query[3] = 1.0;
        query[4] = 0.1; // slight perturbation

        let results = index.search(&query, 3).unwrap();
        assert!(!results.is_empty(), "should have search results");
        assert_eq!(
            results[0].uri_str, "viking://entry/3",
            "closest result should be entry/3, got {:?}",
            results
        );
    }

    #[test]
    fn persist_and_reload() {
        let dim = 8;
        let dir = TempDir::new().unwrap();
        let index_dir = dir.path().join("index");

        // Build index.
        let mut index = MemoryIndex::new(dim, 100);
        for i in 0..8 {
            let v = axis_vector(dim, i);
            index.upsert(&format!("viking://entry/{i}"), &v).unwrap();
        }

        // Search before save.
        let query = axis_vector(dim, 5);
        let results_before = index.search(&query, 3).unwrap();

        // Save.
        index.save(&index_dir).unwrap();

        // Verify meta.json exists and is readable.
        let meta_path = index_dir.join("meta.json");
        assert!(meta_path.exists());
        let meta_raw = std::fs::read_to_string(&meta_path).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta_raw).unwrap();
        assert_eq!(meta["dimension"], dim);

        // Reload.
        let loaded = MemoryIndex::load(&index_dir, dim).unwrap();
        assert_eq!(loaded.dimension(), dim);
        assert_eq!(loaded.live_len(), 8);

        // Search after reload — should return same top result.
        let results_after = loaded.search(&query, 3).unwrap();
        assert!(!results_after.is_empty());
        assert_eq!(results_before[0].uri_str, results_after[0].uri_str);
    }

    #[test]
    fn dimension_mismatch_on_load() {
        let dim = 8;
        let dir = TempDir::new().unwrap();
        let index_dir = dir.path().join("index");

        let mut index = MemoryIndex::new(dim, 100);
        index
            .upsert("viking://test/entry", &axis_vector(dim, 0))
            .unwrap();
        index.save(&index_dir).unwrap();

        // Try to load with wrong dimension.
        let err = MemoryIndex::load(&index_dir, 16).unwrap_err();
        assert!(
            matches!(
                err,
                MemoryError::DimensionMismatch {
                    expected: 16,
                    actual: 8
                }
            ),
            "expected DimensionMismatch, got: {:?}",
            err
        );
        // Verify the error message is clear.
        let msg = format!("{err}");
        assert!(msg.contains("16"));
        assert!(msg.contains("8"));
    }

    #[test]
    fn empty_index_search_returns_empty() {
        let index = MemoryIndex::new(4, 100);
        let results = index.search(&[0.1, 0.2, 0.3, 0.4], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn delete_removes_from_results() {
        let dim = 4;
        let mut index = MemoryIndex::new(dim, 100);

        index.upsert("viking://a", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.upsert("viking://b", &[0.0, 1.0, 0.0, 0.0]).unwrap();

        // Verify both are searchable.
        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        let keys: Vec<&str> = results.iter().map(|r| r.uri_str.as_str()).collect();
        assert!(keys.contains(&"viking://a"));

        // Delete entry a.
        let deleted = index.delete("viking://a");
        assert!(deleted);

        // Search again — should not contain "viking://a".
        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 5).unwrap();
        let keys: Vec<&str> = results.iter().map(|r| r.uri_str.as_str()).collect();
        assert!(
            !keys.contains(&"viking://a"),
            "deleted key should not appear in results"
        );
    }

    #[test]
    fn upsert_updates_existing() {
        let dim = 4;
        let mut index = MemoryIndex::new(dim, 100);

        // Insert pointing along axis 0.
        index
            .upsert("viking://mutable", &[1.0, 0.0, 0.0, 0.0])
            .unwrap();

        // Upsert same key with vector pointing along axis 1.
        index
            .upsert("viking://mutable", &[0.0, 1.0, 0.0, 0.0])
            .unwrap();

        // Search along axis 1 — should find "viking://mutable" as closest.
        let results = index.search(&[0.0, 1.0, 0.0, 0.0], 3).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].uri_str, "viking://mutable");
        // Score should be high (close to 1.0) since we updated to match the query.
        assert!(
            results[0].score > 0.9,
            "score should be high after upsert, got {}",
            results[0].score
        );
    }

    #[test]
    fn dimension_mismatch_on_upsert() {
        let mut index = MemoryIndex::new(4, 100);
        let err = index.upsert("viking://bad", &[1.0, 2.0]).unwrap_err();
        assert!(matches!(
            err,
            MemoryError::DimensionMismatch {
                expected: 4,
                actual: 2
            }
        ));
    }

    #[test]
    fn dimension_mismatch_on_search() {
        let mut index = MemoryIndex::new(4, 100);
        index
            .upsert("viking://test", &[1.0, 0.0, 0.0, 0.0])
            .unwrap();
        let err = index.search(&[1.0, 2.0], 3).unwrap_err();
        assert!(matches!(
            err,
            MemoryError::DimensionMismatch {
                expected: 4,
                actual: 2
            }
        ));
    }

    // ── Integration tests: store + index combined ───────────────────────

    #[test]
    fn integration_store_and_index_persist_and_search() {
        use crate::keyword::KeywordIndex;
        use crate::store::VikingStore;
        use crate::types::LayeredContent;
        use crate::uri::VikingUri;

        let dir = TempDir::new().unwrap();
        let store_dir = dir.path().join("store");
        let index_dir = dir.path().join("index");
        let keyword_path = dir.path().join("keyword_index.json");
        let dim = 4;

        // Create store and indices.
        let store = VikingStore::new(&store_dir).unwrap();
        let mut vector_index = MemoryIndex::new(dim, 100);
        let mut keyword_index = KeywordIndex::new();

        // Write several LayeredContent entries to the store.
        let entries = vec![
            (
                "viking://project/architecture",
                "Hexagonal architecture with ports and adapters.",
                "Uses domain-driven design principles.",
                [1.0, 0.0, 0.0, 0.0],
            ),
            (
                "viking://project/testing",
                "Comprehensive testing strategy.",
                "Unit tests, integration tests, and property-based tests.",
                [0.0, 1.0, 0.0, 0.0],
            ),
            (
                "viking://project/deployment",
                "CI/CD pipeline with automated deployment.",
                "Uses Docker containers and Kubernetes orchestration.",
                [0.0, 0.0, 1.0, 0.0],
            ),
            (
                "viking://project/security",
                "Security best practices.",
                "Input validation, auth middleware, rate limiting.",
                [0.0, 0.0, 0.0, 1.0],
            ),
        ];

        for (uri_str, abstract_text, overview_text, vector) in &entries {
            let uri: VikingUri = uri_str.parse().unwrap();
            let content =
                LayeredContent::new(uri, abstract_text.to_string(), overview_text.to_string());
            store.write(&content).unwrap();
            vector_index.upsert(uri_str, vector).unwrap();

            // Add both abstract and overview to keyword index.
            let full_text = format!("{} {}", abstract_text, overview_text);
            keyword_index.add(uri_str, &full_text);
        }

        // Vector search: query near axis 0 → should find architecture.
        let results = vector_index.search(&[0.9, 0.1, 0.0, 0.0], 2).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].uri_str, "viking://project/architecture");

        // Verify we can look up the content from the store using the search result.
        let hit_uri: VikingUri = results[0].uri_str.parse().unwrap();
        let content = store.read(&hit_uri).unwrap().unwrap();
        assert!(content.abstract_text.contains("Hexagonal"));

        // Keyword search: query "docker kubernetes" → should find deployment.
        let kw_results = keyword_index.search("docker kubernetes", 2);
        assert!(!kw_results.is_empty());
        assert_eq!(kw_results[0].uri_str, "viking://project/deployment");

        // ── Persist everything to disk ──────────────────────────────────
        vector_index.save(&index_dir).unwrap();
        keyword_index.save(&keyword_path).unwrap();
        // (Store is already on disk from the writes above.)

        // ── Reload everything from disk ─────────────────────────────────
        let loaded_store = VikingStore::new(&store_dir).unwrap();
        let loaded_vector_index = MemoryIndex::load(&index_dir, dim).unwrap();
        let loaded_keyword_index = KeywordIndex::load(&keyword_path).unwrap();

        // Verify store round-trip.
        let uris = loaded_store.list().unwrap();
        assert_eq!(uris.len(), 4);

        // Vector search after reload — should return same results.
        let results_after = loaded_vector_index
            .search(&[0.9, 0.1, 0.0, 0.0], 2)
            .unwrap();
        assert!(!results_after.is_empty());
        assert_eq!(
            results_after[0].uri_str, "viking://project/architecture",
            "vector search after reload should return same top result"
        );

        // Keyword search after reload — should return same results.
        let kw_after = loaded_keyword_index.search("docker kubernetes", 2);
        assert!(!kw_after.is_empty());
        assert_eq!(
            kw_after[0].uri_str, "viking://project/deployment",
            "keyword search after reload should return same top result"
        );

        // Cross-check: vector result URI matches a real store entry.
        let hit_uri: VikingUri = results_after[0].uri_str.parse().unwrap();
        let loaded_content = loaded_store.read(&hit_uri).unwrap().unwrap();
        assert_eq!(
            loaded_content.abstract_text,
            "Hexagonal architecture with ports and adapters."
        );
    }
}
