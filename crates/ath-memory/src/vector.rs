//! Embedding-based vector index for semantic memory search.
//!
//! Stores document embeddings as `Vec<f32>` vectors and retrieves
//! the most similar documents using cosine similarity scoring.
//! Persists to JSON for simplicity.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MemoryError;

/// Entry storing an embedding vector alongside its source URI.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorEntry {
    uri: String,
    embedding: Vec<f32>,
}

/// In-memory vector index with cosine similarity search.
///
/// Stores embeddings keyed by URI string. Search returns URIs ranked
/// by cosine similarity to a query vector. O(n) per query — suitable
/// for indexes up to ~10K entries.
#[derive(Debug)]
pub struct VectorIndex {
    entries: Vec<VectorEntry>,
    /// Fast lookup from URI to index in entries vec.
    uri_to_idx: HashMap<String, usize>,
}

impl VectorIndex {
    /// Create a new empty vector index.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            uri_to_idx: HashMap::new(),
        }
    }

    /// Add or update an embedding for a URI.
    pub fn add(&mut self, uri_str: &str, embedding: Vec<f32>) {
        if let Some(&idx) = self.uri_to_idx.get(uri_str) {
            // Update existing entry
            self.entries[idx].embedding = embedding;
        } else {
            let idx = self.entries.len();
            self.entries.push(VectorEntry {
                uri: uri_str.to_string(),
                embedding,
            });
            self.uri_to_idx.insert(uri_str.to_string(), idx);
        }
    }

    /// Remove an entry by URI.
    pub fn remove(&mut self, uri_str: &str) {
        if let Some(idx) = self.uri_to_idx.remove(uri_str) {
            self.entries.swap_remove(idx);
            // Fix up the swapped entry's index (if it wasn't the last)
            if idx < self.entries.len() {
                let swapped_uri = self.entries[idx].uri.clone();
                self.uri_to_idx.insert(swapped_uri, idx);
            }
        }
    }

    /// Number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Search for the top-k most similar entries to a query vector.
    ///
    /// Returns (uri, score) pairs sorted by descending similarity.
    /// Score is cosine similarity in [-1, 1] range.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if self.entries.is_empty() || query.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(String, f32)> = self
            .entries
            .iter()
            .map(|entry| {
                let score = cosine_similarity(query, &entry.embedding);
                (entry.uri.clone(), score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Check if a URI exists in the index.
    pub fn contains(&self, uri_str: &str) -> bool {
        self.uri_to_idx.contains_key(uri_str)
    }

    /// Save the index to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let data: Vec<&VectorEntry> = self.entries.iter().collect();
        let json =
            serde_json::to_string_pretty(&data).map_err(|e| MemoryError::SerializationError {
                message: "failed to serialize vector index".into(),
                source: Some(e),
            })?;

        // Atomic write: temp + rename
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json).map_err(|e| MemoryError::IoError {
            path: path.display().to_string(),
            message: "failed to write vector index".into(),
            source: e,
        })?;
        std::fs::rename(&tmp, path).map_err(|e| MemoryError::IoError {
            path: path.display().to_string(),
            message: "failed to rename vector index".into(),
            source: e,
        })?;

        Ok(())
    }

    /// Load the index from a JSON file.
    pub fn load(path: &Path) -> Result<Self, MemoryError> {
        let json = std::fs::read_to_string(path).map_err(|e| MemoryError::IoError {
            path: path.display().to_string(),
            message: "failed to read vector index".into(),
            source: e,
        })?;

        let entries: Vec<VectorEntry> =
            serde_json::from_str(&json).map_err(|e| MemoryError::SerializationError {
                message: "failed to deserialize vector index".into(),
                source: Some(e),
            })?;

        let uri_to_idx: HashMap<String, usize> = entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| (entry.uri.clone(), idx))
            .collect();

        Ok(Self {
            entries,
            uri_to_idx,
        })
    }
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut mag_a = 0.0f32;
    let mut mag_b = 0.0f32;

    for i in 0..len {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }

    let mag = mag_a.sqrt() * mag_b.sqrt();
    if mag == 0.0 {
        0.0
    } else {
        dot / mag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_index_returns_no_results() {
        let index = VectorIndex::new();
        let results = index.search(&[1.0, 0.0, 0.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn add_and_search_single_entry() {
        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);

        let results = index.search(&[1.0, 0.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "doc:1");
        assert!((results[0].1 - 1.0).abs() < 0.001); // Perfect match
    }

    #[test]
    fn search_ranks_by_similarity() {
        let mut index = VectorIndex::new();
        index.add("exact", vec![1.0, 0.0, 0.0]);
        index.add("similar", vec![0.9, 0.1, 0.0]);
        index.add("different", vec![0.0, 1.0, 0.0]);

        let results = index.search(&[1.0, 0.0, 0.0], 3);
        assert_eq!(results[0].0, "exact");
        assert_eq!(results[1].0, "similar");
        assert_eq!(results[2].0, "different");
    }

    #[test]
    fn top_k_limits_results() {
        let mut index = VectorIndex::new();
        for i in 0..10 {
            index.add(&format!("doc:{i}"), vec![i as f32, 0.0, 0.0]);
        }

        let results = index.search(&[9.0, 0.0, 0.0], 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn upsert_updates_existing_entry() {
        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);
        index.add("doc:1", vec![0.0, 1.0, 0.0]);

        assert_eq!(index.len(), 1);

        let results = index.search(&[0.0, 1.0, 0.0], 1);
        assert_eq!(results[0].0, "doc:1");
        assert!((results[0].1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn remove_entry() {
        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);
        index.add("doc:2", vec![0.0, 1.0, 0.0]);

        index.remove("doc:1");
        assert_eq!(index.len(), 1);
        assert!(!index.contains("doc:1"));
        assert!(index.contains("doc:2"));
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);
        index.remove("doc:99");
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn cosine_similarity_orthogonal_is_zero() {
        let score = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(score.abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_identical_is_one() {
        let score = cosine_similarity(&[3.0, 4.0], &[3.0, 4.0]);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_opposite_is_negative_one() {
        let score = cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]);
        assert!((score + 1.0).abs() < 0.001);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vectors.json");

        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);
        index.add("doc:2", vec![0.0, 1.0, 0.5]);
        index.save(&path).unwrap();

        let loaded = VectorIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains("doc:1"));
        assert!(loaded.contains("doc:2"));

        // Verify search still works after load
        let results = loaded.search(&[1.0, 0.0, 0.0], 1);
        assert_eq!(results[0].0, "doc:1");
    }

    #[test]
    fn search_with_empty_query_returns_nothing() {
        let mut index = VectorIndex::new();
        index.add("doc:1", vec![1.0, 0.0, 0.0]);
        let results = index.search(&[], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn contains_check() {
        let mut index = VectorIndex::new();
        assert!(!index.contains("doc:1"));
        index.add("doc:1", vec![1.0]);
        assert!(index.contains("doc:1"));
    }
}
