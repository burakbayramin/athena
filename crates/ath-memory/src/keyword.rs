//! Keyword-based fallback index for memory search.
//!
//! A simple inverted index using TF-IDF-style scoring. Provides viable
//! search when no embeddings are available. Persists to JSON.

use std::collections::HashMap;
use std::path::Path;

use crate::error::MemoryError;

/// Inverted index mapping terms to documents with TF-IDF-style scores.
///
/// Used as a fallback when vector embeddings are not available.
#[derive(Debug)]
pub struct KeywordIndex {
    /// term → vec of (uri_str, term_frequency_weight)
    postings: HashMap<String, Vec<(String, f32)>>,
    /// Track which URIs are in the index (for removal).
    docs: HashMap<String, Vec<String>>,
}

/// A small set of English stopwords to filter out.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "is", "it", "as", "be", "was", "are", "this", "that", "from", "not", "no", "if", "so",
    "can", "do", "has", "have", "had", "will", "would", "could", "should",
];

impl KeywordIndex {
    /// Creates a new empty keyword index.
    pub fn new() -> Self {
        Self {
            postings: HashMap::new(),
            docs: HashMap::new(),
        }
    }

    /// Add a document to the index.
    ///
    /// Tokenizes the text, computes term frequencies, and updates the
    /// inverted index. If the URI already exists, it is removed first
    /// (upsert semantics).
    pub fn add(&mut self, uri_str: &str, text: &str) {
        // Remove existing entry if present (upsert).
        self.remove(uri_str);

        let tokens = tokenize(text);
        if tokens.is_empty() {
            return;
        }

        // Compute term frequencies.
        let mut term_counts: HashMap<&str, usize> = HashMap::new();
        for token in &tokens {
            *term_counts.entry(token.as_str()).or_insert(0) += 1;
        }

        let total = tokens.len() as f32;
        let mut terms_for_doc = Vec::new();

        for (term, count) in term_counts {
            let tf = count as f32 / total;
            let term_owned = term.to_string();

            self.postings
                .entry(term_owned.clone())
                .or_default()
                .push((uri_str.to_string(), tf));

            terms_for_doc.push(term_owned);
        }

        self.docs.insert(uri_str.to_string(), terms_for_doc);
    }

    /// Search the index for documents matching the query.
    ///
    /// Tokenizes the query, scores documents by sum of matching term
    /// weights, and returns the top_k results ordered by score.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<KeywordHit> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        // Score each document by summing TF weights for matching query terms.
        let mut doc_scores: HashMap<&str, f32> = HashMap::new();

        for token in &query_tokens {
            if let Some(postings) = self.postings.get(token) {
                for (uri, tf) in postings {
                    *doc_scores.entry(uri.as_str()).or_insert(0.0) += tf;
                }
            }
        }

        // Sort by score descending.
        let mut scored: Vec<KeywordHit> = doc_scores
            .into_iter()
            .map(|(uri, score)| KeywordHit {
                uri_str: uri.to_string(),
                score,
            })
            .collect();
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Remove a document from the index.
    pub fn remove(&mut self, uri_str: &str) {
        if let Some(terms) = self.docs.remove(uri_str) {
            for term in &terms {
                if let Some(postings) = self.postings.get_mut(term) {
                    postings.retain(|(uri, _)| uri != uri_str);
                    if postings.is_empty() {
                        self.postings.remove(term);
                    }
                }
            }
        }
    }

    /// Number of documents in the index.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// Returns `true` if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Persist the index to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let data = SerializedKeywordIndex {
            postings: self.postings.clone(),
            docs: self.docs.clone(),
        };
        let json = serde_json::to_string_pretty(&data).map_err(|e| {
            MemoryError::SerializationError {
                message: format!("failed to serialize keyword index: {}", e),
                source: Some(e),
            }
        })?;

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| MemoryError::IoError {
                    path: parent.display().to_string(),
                    message: "failed to create keyword index directory".to_string(),
                    source: e,
                })?;
            }
        }

        std::fs::write(path, json).map_err(|e| MemoryError::IoError {
            path: path.display().to_string(),
            message: "failed to write keyword index".to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Load the index from a JSON file.
    pub fn load(path: &Path) -> Result<Self, MemoryError> {
        let raw = std::fs::read_to_string(path).map_err(|e| MemoryError::IoError {
            path: path.display().to_string(),
            message: "failed to read keyword index".to_string(),
            source: e,
        })?;

        let data: SerializedKeywordIndex =
            serde_json::from_str(&raw).map_err(|e| MemoryError::SerializationError {
                message: format!("failed to parse keyword index: {}", e),
                source: Some(e),
            })?;

        Ok(Self {
            postings: data.postings,
            docs: data.docs,
        })
    }
}

impl Default for KeywordIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// A search result from the keyword index.
#[derive(Debug, Clone)]
pub struct KeywordHit {
    /// The URI string of the matched entry.
    pub uri_str: String,
    /// Relevance score (higher is more relevant).
    pub score: f32,
}

/// Serialization format for the keyword index.
#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedKeywordIndex {
    postings: HashMap<String, Vec<(String, f32)>>,
    docs: HashMap<String, Vec<String>>,
}

/// Tokenize text: lowercase, split on whitespace/punctuation, filter stopwords.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .filter(|s| s.len() > 1) // skip single-character tokens
        .filter(|s| !STOPWORDS.contains(s))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn add_and_search() {
        let mut index = KeywordIndex::new();

        index.add(
            "viking://rust/ownership",
            "Rust ownership rules prevent data races and memory safety issues",
        );
        index.add(
            "viking://rust/traits",
            "Rust traits define shared behavior similar to interfaces",
        );
        index.add(
            "viking://python/typing",
            "Python typing module provides type hints for static analysis",
        );

        let results = index.search("rust ownership", 5);
        assert!(!results.is_empty());
        // "rust/ownership" should be the top hit since it matches both terms.
        assert_eq!(results[0].uri_str, "viking://rust/ownership");
    }

    #[test]
    fn search_empty_index() {
        let index = KeywordIndex::new();
        let results = index.search("anything", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn search_no_matches() {
        let mut index = KeywordIndex::new();
        index.add("viking://test", "hello world");
        let results = index.search("zzzznonexistent", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn remove_entry() {
        let mut index = KeywordIndex::new();
        index.add("viking://to-remove", "important searchable content");
        assert_eq!(index.len(), 1);

        // Should find it.
        let results = index.search("important content", 5);
        assert!(!results.is_empty());

        // Remove and verify gone.
        index.remove("viking://to-remove");
        assert_eq!(index.len(), 0);

        let results = index.search("important content", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn persist_and_reload() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("keyword_index.json");

        let mut index = KeywordIndex::new();
        index.add("viking://doc/architecture", "hexagonal architecture with ports and adapters");
        index.add("viking://doc/testing", "unit testing integration testing end to end");

        index.save(&path).unwrap();
        assert!(path.exists());

        let loaded = KeywordIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);

        // Search should still work.
        let results = loaded.search("architecture", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].uri_str, "viking://doc/architecture");
    }

    #[test]
    fn upsert_semantics() {
        let mut index = KeywordIndex::new();
        index.add("viking://entry", "original content about cats");
        index.add("viking://entry", "updated content about dogs");

        assert_eq!(index.len(), 1);

        // Should find "dogs" but not "cats".
        let results = index.search("dogs", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].uri_str, "viking://entry");

        let results = index.search("cats", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn stopwords_filtered() {
        let tokens = tokenize("the quick brown fox is a test");
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn tokenize_handles_punctuation() {
        let tokens = tokenize("hello, world! rust-lang.org (test)");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"lang".to_string()));
        assert!(tokens.contains(&"org".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }
}
