//! [`ContextInjector`] — builds budget-constrained context from memory for agent prompts.

use crate::keyword::KeywordIndex;
use crate::store::VikingStore;
use crate::uri::VikingUri;
use crate::vector::VectorIndex;

/// Per-section token budget configuration for context injection.
///
/// Defaults match the spec (§10): total=4000, project_identity=200,
/// semantic_results=2500, agent_notes=300, recent_run=500.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct InjectionConfig {
    /// Maximum total tokens injected per agent call.
    pub total_budget: usize,
    /// Reserved budget for the project identity section.
    pub project_identity: usize,
    /// Budget for keyword-search-based context.
    pub semantic_results: usize,
    /// Reserved budget for agent-specific notes.
    pub agent_notes: usize,
    /// Budget for last run summary context.
    pub recent_run: usize,
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            total_budget: 4000,
            project_identity: 200,
            semantic_results: 2500,
            agent_notes: 300,
            recent_run: 500,
        }
    }
}

/// The assembled context ready for injection into an agent prompt.
#[derive(Debug, Clone)]
pub struct InjectedContext {
    /// The formatted context string (XML-wrapped sections).
    pub text: String,
    /// Estimated token count of the assembled context.
    pub estimated_tokens: usize,
    /// Names of sections that were included (non-empty).
    pub sections_included: Vec<String>,
}

impl InjectedContext {
    /// Returns `true` if no sections were included (empty context).
    pub fn is_empty(&self) -> bool {
        self.sections_included.is_empty()
    }
}

/// Reads from [`VikingStore`] and [`KeywordIndex`] to build prompt context.
///
/// All operations are fail-soft — store read failures and missing URIs
/// produce empty sections, never errors.
pub struct ContextInjector<'a> {
    store: &'a VikingStore,
    keyword_index: &'a KeywordIndex,
    /// Optional vector index for embedding-based search.
    /// When present with a query embedding, vector search takes priority.
    vector_index: Option<&'a VectorIndex>,
    /// Optional query embedding for vector search.
    /// Provided by the caller (who handles the embedding API call).
    query_embedding: Option<Vec<f32>>,
}

impl<'a> ContextInjector<'a> {
    /// Creates a new injector backed by the given store and keyword index.
    pub fn new(store: &'a VikingStore, keyword_index: &'a KeywordIndex) -> Self {
        Self {
            store,
            keyword_index,
            vector_index: None,
            query_embedding: None,
        }
    }

    /// Attach a vector index and pre-computed query embedding for semantic search.
    ///
    /// When both are set, `build_context` prefers vector search results over
    /// keyword search. Falls back to keywords if vector search returns nothing.
    pub fn with_vector_search(
        mut self,
        vector_index: &'a VectorIndex,
        query_embedding: Vec<f32>,
    ) -> Self {
        self.vector_index = Some(vector_index);
        self.query_embedding = Some(query_embedding);
        self
    }

    /// Build a budget-constrained context string for an agent prompt.
    ///
    /// Assembles sections in order: project identity, semantic search results,
    /// agent notes, recent run summary. Each section is truncated to its
    /// per-section budget, and the total is capped at `config.total_budget`.
    ///
    /// Never returns an error — all failures produce empty sections with
    /// a `tracing::warn`.
    pub fn build_context(&self, query: &str, config: &InjectionConfig) -> InjectedContext {
        let mut sections: Vec<(String, String)> = Vec::new();
        let mut total_tokens: usize = 0;

        // Section 1: Project identity from viking://project/identity
        if let Some(text) = self.read_project_identity(config.project_identity) {
            let tokens = estimate_tokens(&text);
            let capped = Self::cap_section_to_remaining(
                &text,
                tokens,
                config.project_identity,
                config.total_budget,
                total_tokens,
            );
            if let Some((final_text, final_tokens)) = capped {
                sections.push(("project".to_string(), final_text));
                total_tokens += final_tokens;
            }
        }

        // Section 2: Semantic search results via keyword index
        if total_tokens < config.total_budget {
            if let Some(text) = self.read_semantic_results(
                query,
                config.semantic_results,
                config.total_budget - total_tokens,
            ) {
                let tokens = estimate_tokens(&text);
                let capped = Self::cap_section_to_remaining(
                    &text,
                    tokens,
                    config.semantic_results,
                    config.total_budget,
                    total_tokens,
                );
                if let Some((final_text, final_tokens)) = capped {
                    sections.push(("relevant_context".to_string(), final_text));
                    total_tokens += final_tokens;
                }
            }
        }

        // Section 3: Agent notes from viking://agents/*/profile
        if total_tokens < config.total_budget {
            if let Some(text) = self.read_agent_notes(query, config.agent_notes) {
                let tokens = estimate_tokens(&text);
                let capped = Self::cap_section_to_remaining(
                    &text,
                    tokens,
                    config.agent_notes,
                    config.total_budget,
                    total_tokens,
                );
                if let Some((final_text, final_tokens)) = capped {
                    sections.push(("agent_notes".to_string(), final_text));
                    total_tokens += final_tokens;
                }
            }
        }

        // Section 4: Recent run summary from viking://runs/*/summary
        if total_tokens < config.total_budget {
            if let Some(text) = self.read_recent_run(config.recent_run) {
                let tokens = estimate_tokens(&text);
                let capped = Self::cap_section_to_remaining(
                    &text,
                    tokens,
                    config.recent_run,
                    config.total_budget,
                    total_tokens,
                );
                if let Some((final_text, _final_tokens)) = capped {
                    sections.push(("recent_run".to_string(), final_text));
                    // total_tokens not updated — this is the last section.
                }
            }
        }

        // Build the XML-wrapped output
        let sections_included: Vec<String> =
            sections.iter().map(|(name, _)| name.clone()).collect();

        let text = if sections.is_empty() {
            String::new()
        } else {
            let mut out = String::from("<athena_context>\n");
            for (name, content) in &sections {
                out.push_str(&format!("  <{}>\n", name));
                // Indent content lines
                for line in content.lines() {
                    out.push_str(&format!("    {}\n", line));
                }
                out.push_str(&format!("  </{}>\n", name));
            }
            out.push_str("</athena_context>");
            out
        };

        let estimated_tokens = estimate_tokens(&text);

        tracing::info!(
            tokens = estimated_tokens,
            sections = ?sections_included,
            "Context built"
        );

        InjectedContext {
            text,
            estimated_tokens,
            sections_included,
        }
    }

    /// Cap a section's text to fit within both its own budget and the remaining total budget.
    /// Returns `None` if no room remains.
    fn cap_section_to_remaining(
        text: &str,
        tokens: usize,
        section_budget: usize,
        total_budget: usize,
        total_used: usize,
    ) -> Option<(String, usize)> {
        let remaining = total_budget.saturating_sub(total_used);
        if remaining == 0 {
            return None;
        }

        let effective_budget = section_budget.min(remaining);

        if tokens <= effective_budget {
            Some((text.to_string(), tokens))
        } else {
            let truncated = truncate_to_budget(text, effective_budget);
            let final_tokens = estimate_tokens(&truncated);
            if final_tokens == 0 {
                None
            } else {
                Some((truncated, final_tokens))
            }
        }
    }

    /// Read project identity from `viking://project/identity`. Fail-soft.
    fn read_project_identity(&self, budget: usize) -> Option<String> {
        let uri: VikingUri = match "viking://project/identity".parse() {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, "Context section skipped: failed to parse project identity URI");
                return None;
            }
        };

        match self.store.read(&uri) {
            Ok(Some(content)) => {
                let text = content.abstract_text.clone();
                if text.is_empty() {
                    None
                } else {
                    Some(truncate_to_budget(&text, budget))
                }
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "Context section skipped: project identity read failed");
                None
            }
        }
    }

    /// Search keyword index and join hits with store reads. Fail-soft.
    fn read_semantic_results(
        &self,
        query: &str,
        section_budget: usize,
        remaining_total: usize,
    ) -> Option<String> {
        if query.trim().is_empty() {
            return None;
        }

        // Prefer vector search when available, fall back to keyword
        let hit_uris: Vec<String> =
            if let (Some(vi), Some(qe)) = (&self.vector_index, &self.query_embedding) {
                let vector_hits = vi.search(qe, 10);
                if vector_hits.is_empty() {
                    // Fall back to keyword search
                    self.keyword_index
                        .search(query, 10)
                        .into_iter()
                        .map(|h| h.uri_str)
                        .collect()
                } else {
                    vector_hits.into_iter().map(|(uri, _score)| uri).collect()
                }
            } else {
                self.keyword_index
                    .search(query, 10)
                    .into_iter()
                    .map(|h| h.uri_str)
                    .collect()
            };

        if hit_uris.is_empty() {
            return None;
        }

        let effective_budget = section_budget.min(remaining_total);
        let mut parts: Vec<String> = Vec::new();
        let mut tokens_used: usize = 0;

        for uri_str in &hit_uris {
            if tokens_used >= effective_budget {
                break;
            }

            let uri: VikingUri = match uri_str.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };

            match self.store.read(&uri) {
                Ok(Some(content)) => {
                    // Use abstract for brevity, fall back to overview
                    let entry_text = if !content.abstract_text.is_empty() {
                        content.abstract_text.clone()
                    } else {
                        content.overview_text.clone()
                    };

                    if entry_text.is_empty() {
                        continue;
                    }

                    let remaining = effective_budget.saturating_sub(tokens_used);
                    let entry_truncated = truncate_to_budget(&entry_text, remaining);
                    let entry_tokens = estimate_tokens(&entry_truncated);

                    if entry_tokens == 0 {
                        continue;
                    }

                    parts.push(format!("[{}]: {}", uri_str, entry_truncated));
                    tokens_used += entry_tokens;
                }
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(
                        uri = %uri_str,
                        error = %e,
                        "Context section skipped: semantic result read failed"
                    );
                    continue;
                }
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    /// Read agent notes — find the best match from keyword search under agents/. Fail-soft.
    fn read_agent_notes(&self, query: &str, budget: usize) -> Option<String> {
        // Search for agent-related entries
        let hits = self.keyword_index.search(query, 5);

        // Find the best hit that looks like an agent profile
        for hit in &hits {
            if hit.uri_str.contains("agents/") && hit.uri_str.contains("profile") {
                let uri: VikingUri = match hit.uri_str.parse() {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                match self.store.read(&uri) {
                    Ok(Some(content)) => {
                        let text = if !content.abstract_text.is_empty() {
                            content.abstract_text.clone()
                        } else {
                            content.overview_text.clone()
                        };
                        if text.is_empty() {
                            continue;
                        }
                        return Some(truncate_to_budget(&text, budget));
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        tracing::warn!(
                            uri = %hit.uri_str,
                            error = %e,
                            "Context section skipped: agent notes read failed"
                        );
                        continue;
                    }
                }
            }
        }

        None
    }

    /// Read the most recent run summary. Walks `viking://runs/*/summary` URIs. Fail-soft.
    fn read_recent_run(&self, budget: usize) -> Option<String> {
        // List all URIs, filter for runs/*/summary, take the last one (sorted).
        let uris = match self.store.list() {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, "Context section skipped: failed to list store for recent run");
                return None;
            }
        };

        let mut run_summaries: Vec<VikingUri> = uris
            .into_iter()
            .filter(|u| {
                let s = u.to_string();
                s.starts_with("viking://runs/") && s.ends_with("/summary")
            })
            .collect();

        // Sort lexicographically — last one is "most recent" by naming convention.
        run_summaries.sort_by_key(|uri| uri.to_string());

        if let Some(uri) = run_summaries.last() {
            match self.store.read(uri) {
                Ok(Some(content)) => {
                    let text = if !content.abstract_text.is_empty() {
                        content.abstract_text.clone()
                    } else {
                        content.overview_text.clone()
                    };
                    if text.is_empty() {
                        return None;
                    }
                    Some(truncate_to_budget(&text, budget))
                }
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(
                        uri = %uri,
                        error = %e,
                        "Context section skipped: recent run summary read failed"
                    );
                    None
                }
            }
        } else {
            None
        }
    }
}

/// Estimate token count from text using a word-count heuristic.
///
/// Formula: `words * 4 / 3` — approximately 0.75 words per token.
/// Good enough for M002, swappable later.
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count() * 4 / 3
}

/// Truncate text to fit within a token budget, preserving word boundaries.
///
/// Takes words from the front until the budget is reached. Returns the
/// truncated string (may be shorter than the budget).
pub fn truncate_to_budget(text: &str, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return String::new();
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    let total_tokens = words.len() * 4 / 3;

    if total_tokens <= max_tokens {
        return text.to_string();
    }

    // target_words * 4 / 3 <= max_tokens  →  target_words <= max_tokens * 3 / 4
    let target_words = max_tokens * 3 / 4;
    let take = target_words.min(words.len());

    if take == 0 {
        return String::new();
    }

    words[..take].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyword::KeywordIndex;
    use crate::store::VikingStore;
    use crate::types::LayeredContent;
    use crate::uri::VikingUri;
    use tempfile::TempDir;

    /// Helper: set up an empty store + index in a temp dir.
    fn setup_empty() -> (TempDir, VikingStore, KeywordIndex) {
        let dir = TempDir::new().unwrap();
        let store = VikingStore::new(dir.path().join("store")).unwrap();
        let index = KeywordIndex::new();
        (dir, store, index)
    }

    /// Helper: write a LayeredContent entry to the store and index.
    fn write_entry(
        store: &VikingStore,
        index: &mut KeywordIndex,
        uri_str: &str,
        abstract_text: &str,
        overview_text: &str,
    ) {
        let uri: VikingUri = uri_str.parse().unwrap();
        let content =
            LayeredContent::new(uri, abstract_text.to_string(), overview_text.to_string());
        store.write(&content).unwrap();
        // Index abstract + overview for keyword search
        let combined = format!("{} {}", abstract_text, overview_text);
        index.add(uri_str, &combined);
    }

    #[test]
    fn empty_store_produces_empty_context() {
        let (_dir, store, index) = setup_empty();
        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();

        let result = injector.build_context("test query", &config);

        assert!(result.is_empty());
        assert_eq!(result.estimated_tokens, 0);
        assert!(result.sections_included.is_empty());
        assert!(result.text.is_empty());
    }

    #[test]
    fn project_identity_section_populated() {
        let (_dir, store, mut index) = setup_empty();
        write_entry(
            &store,
            &mut index,
            "viking://project/identity",
            "A Rust CLI tool for managing deployments.",
            "Manages cloud deployments with rollback support.",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("deploy", &config);

        assert!(!result.is_empty());
        assert!(result.sections_included.contains(&"project".to_string()));
        assert!(result.text.contains("<project>"));
        assert!(result.text.contains("Rust CLI tool"));
        assert!(result.text.contains("</project>"));
    }

    #[test]
    fn keyword_search_results_in_semantic_section() {
        let (_dir, store, mut index) = setup_empty();
        write_entry(
            &store,
            &mut index,
            "viking://project/conventions",
            "Uses hexagonal architecture with error hints.",
            "Hexagonal architecture pattern with ports and adapters.",
        );
        write_entry(
            &store,
            &mut index,
            "viking://project/testing",
            "Integration tests cover all API endpoints.",
            "Tests use mock backends and temp directories.",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("hexagonal architecture", &config);

        assert!(result
            .sections_included
            .contains(&"relevant_context".to_string()));
        assert!(result.text.contains("<relevant_context>"));
        assert!(result.text.contains("hexagonal"));
    }

    #[test]
    fn total_budget_respected() {
        let (_dir, store, mut index) = setup_empty();

        // Write a big project identity
        let big_text = "word ".repeat(5000);
        write_entry(
            &store,
            &mut index,
            "viking://project/identity",
            &big_text,
            "overview",
        );

        // Write several searchable entries
        for i in 0..10 {
            let text = format!("searchable content number {} with lots of words to fill space and consume tokens quickly in the budget", i);
            write_entry(
                &store,
                &mut index,
                &format!("viking://data/entry{}", i),
                &text,
                &text,
            );
        }

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig {
            total_budget: 500,
            project_identity: 100,
            semantic_results: 300,
            agent_notes: 50,
            recent_run: 50,
        };
        let result = injector.build_context("searchable content", &config);

        // Total tokens should not exceed the budget.
        // The actual text includes XML wrapper overhead, but the raw content
        // per-section is individually capped.
        assert!(
            result.estimated_tokens <= config.total_budget + 50, // small overhead for XML tags
            "estimated_tokens {} should be near total_budget {}",
            result.estimated_tokens,
            config.total_budget,
        );
    }

    #[test]
    fn per_section_budget_enforced_oversized_content_truncated() {
        let (_dir, store, mut index) = setup_empty();

        // Write 2000-word project identity (way over the 200-token default budget)
        let big_abstract = "important ".repeat(2000);
        write_entry(
            &store,
            &mut index,
            "viking://project/identity",
            &big_abstract,
            "overview",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig {
            total_budget: 4000,
            project_identity: 200,
            semantic_results: 2500,
            agent_notes: 300,
            recent_run: 500,
        };
        let result = injector.build_context("anything", &config);

        assert!(result.sections_included.contains(&"project".to_string()));

        // Extract the project section content and verify it was truncated
        let project_start = result.text.find("<project>").unwrap();
        let project_end = result.text.find("</project>").unwrap();
        let project_content = &result.text[project_start..project_end];
        let project_tokens = estimate_tokens(project_content);

        // Should be well under the 2000-word original (~2666 tokens)
        // and within the 200-token section budget (plus a bit for the tag)
        assert!(
            project_tokens <= 250,
            "project section tokens {} should be <= 250 (budget 200 + tag overhead)",
            project_tokens,
        );
    }

    #[test]
    fn missing_uris_produce_empty_sections_fail_soft() {
        let (_dir, store, index) = setup_empty();
        // No entries in store at all — every section should be empty.

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("nonexistent query", &config);

        assert!(result.is_empty());
        // Should not panic or error
    }

    #[test]
    fn estimate_tokens_accuracy() {
        // Empty text
        assert_eq!(estimate_tokens(""), 0);

        // Single word: 1 * 4 / 3 = 1 (integer division)
        assert_eq!(estimate_tokens("hello"), 1);

        // 3 words: 3 * 4 / 3 = 4
        assert_eq!(estimate_tokens("one two three"), 4);

        // 10 words: 10 * 4 / 3 = 13
        assert_eq!(estimate_tokens("a b c d e f g h i j"), 13);

        // 100 words: 100 * 4 / 3 = 133
        let hundred_words = "word ".repeat(100);
        assert_eq!(estimate_tokens(hundred_words.trim()), 133);
    }

    #[test]
    fn truncate_to_budget_preserves_word_boundaries() {
        let text = "one two three four five six seven eight nine ten";

        // Budget of 4 tokens: target_words = 4 * 3 / 4 = 3
        let truncated = truncate_to_budget(text, 4);
        assert_eq!(truncated, "one two three");

        // Verify no partial words
        assert!(!truncated.ends_with(' '));
        for word in truncated.split_whitespace() {
            assert!(!word.is_empty());
        }
    }

    #[test]
    fn truncate_to_budget_returns_full_text_within_budget() {
        let text = "short text";
        let truncated = truncate_to_budget(text, 1000);
        assert_eq!(truncated, text);
    }

    #[test]
    fn truncate_to_budget_zero_budget() {
        let truncated = truncate_to_budget("some text", 0);
        assert!(truncated.is_empty());
    }

    #[test]
    fn multiple_sections_format_with_xml_wrapper() {
        let (_dir, store, mut index) = setup_empty();

        write_entry(
            &store,
            &mut index,
            "viking://project/identity",
            "A Rust project.",
            "Rust project overview.",
        );
        write_entry(
            &store,
            &mut index,
            "viking://project/conventions",
            "Uses hexagonal architecture.",
            "Hexagonal pattern details.",
        );
        write_entry(
            &store,
            &mut index,
            "viking://agents/claude/profile",
            "Claude prefers concise code.",
            "Agent profile for Claude.",
        );
        write_entry(
            &store,
            &mut index,
            "viking://runs/run001/summary",
            "Last run completed 5 tasks successfully.",
            "Run summary details.",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("rust architecture claude", &config);

        // Outer wrapper
        assert!(result.text.starts_with("<athena_context>"));
        assert!(result.text.ends_with("</athena_context>"));

        // At least project and semantic sections present
        assert!(result.sections_included.contains(&"project".to_string()));

        // Token estimate is positive
        assert!(result.estimated_tokens > 0);
    }

    #[test]
    fn agent_notes_found_via_keyword_search() {
        let (_dir, store, mut index) = setup_empty();

        write_entry(
            &store,
            &mut index,
            "viking://agents/claude/profile",
            "Claude agent prefers explicit error handling.",
            "Claude works best with typed errors and clear hints.",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("claude error handling", &config);

        assert!(result
            .sections_included
            .contains(&"agent_notes".to_string()));
        assert!(result.text.contains("<agent_notes>"));
        assert!(result.text.contains("explicit error handling"));
    }

    #[test]
    fn recent_run_section_from_store() {
        let (_dir, store, mut index) = setup_empty();

        write_entry(
            &store,
            &mut index,
            "viking://runs/run001/summary",
            "First run: set up project structure.",
            "Created initial files and config.",
        );
        write_entry(
            &store,
            &mut index,
            "viking://runs/run002/summary",
            "Second run: implemented API endpoints.",
            "Added REST endpoints for CRUD operations.",
        );

        let injector = ContextInjector::new(&store, &index);
        let config = InjectionConfig::default();
        let result = injector.build_context("anything", &config);

        assert!(result.sections_included.contains(&"recent_run".to_string()));
        // Should pick run002 (last sorted)
        assert!(result.text.contains("Second run"));
    }

    #[test]
    fn vector_search_preferred_over_keyword_when_available() {
        let (_dir, store, mut keyword_index) = setup_empty();
        let mut vector_index = VectorIndex::new();

        // Write two entries — keyword matches "auth", vector matches "login"
        write_entry(
            &store,
            &mut keyword_index,
            "viking://project/decisions/auth",
            "Authentication uses JWT tokens.",
            "JWT-based auth flow.",
        );
        write_entry(
            &store,
            &mut keyword_index,
            "viking://project/decisions/logging",
            "Structured logging with tracing.",
            "Uses tracing crate.",
        );

        // Vector index associates "auth" entry with embedding [1,0,0]
        // and "logging" entry with embedding [0,1,0]
        vector_index.add("viking://project/decisions/auth", vec![1.0, 0.0, 0.0]);
        vector_index.add("viking://project/decisions/logging", vec![0.0, 1.0, 0.0]);

        // Query embedding close to "auth" entry
        let query_embedding = vec![0.9, 0.1, 0.0];

        let injector = ContextInjector::new(&store, &keyword_index)
            .with_vector_search(&vector_index, query_embedding);

        let config = InjectionConfig::default();
        let result = injector.build_context("login flow", &config);

        // Vector search should find "auth" (close embedding) even though
        // keyword "login" doesn't match
        assert!(
            result.text.contains("JWT"),
            "vector search should find auth entry by embedding similarity"
        );
    }

    #[test]
    fn vector_search_falls_back_to_keyword_when_empty() {
        let (_dir, store, mut keyword_index) = setup_empty();
        let vector_index = VectorIndex::new(); // Empty vector index

        write_entry(
            &store,
            &mut keyword_index,
            "viking://project/decisions/auth",
            "Authentication uses JWT tokens.",
            "JWT-based auth flow.",
        );

        let query_embedding = vec![1.0, 0.0, 0.0];
        let injector = ContextInjector::new(&store, &keyword_index)
            .with_vector_search(&vector_index, query_embedding);

        let config = InjectionConfig::default();
        let result = injector.build_context("auth tokens", &config);

        // Should fall back to keyword search
        assert!(
            result.text.contains("JWT"),
            "should fall back to keyword search when vector index is empty"
        );
    }
}
