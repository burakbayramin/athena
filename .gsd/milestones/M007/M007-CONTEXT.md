# M007: Embedding-Based Memory Recall — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Replace the keyword-based fallback memory search with embedding-based vector search for better semantic context retrieval. Uses genai's `client.embed()` for embedding generation and cosine similarity for search.

## Why This Milestone

Keyword-based TF-IDF search misses semantic matches — "authentication" won't match "login flow" or "auth middleware." Vector embeddings capture semantic similarity, dramatically improving context injection quality for agent prompts.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Get semantically relevant memory recall (not just keyword matches)
- Configure embedding model via `.ath/agents.toml` or env var
- Fall back to keyword search when no embedding API key is available

### Entry point / environment

- Entry point: `ath run` (context injection happens automatically)
- Environment: local dev — CLI binary
- Live dependencies involved: Embedding API (OpenAI, Google, etc.)

## Completion Class

- Contract complete means: VectorIndex stores/searches embeddings, cosine similarity ranks results
- Integration complete means: ContextInjector prefers vector search over keyword when available
- Operational complete means: graceful fallback to keyword when embedding API unavailable

## Final Integrated Acceptance

- Memory search with embeddings returns semantically relevant results
- Keyword fallback still works when no embedding API is configured
- All existing tests pass

## Risks and Unknowns

- **Embedding model availability**: Not all genai providers support embeddings. Need to detect and fall back.
- **Embedding persistence**: Vectors are large — need efficient storage format.
- **API cost**: Embedding calls cost money. Need to batch and cache.

## Existing Codebase / Prior Art

- `crates/ath-memory/src/keyword.rs` — KeywordIndex with TF-IDF
- `crates/ath-memory/src/inject/injector.rs` — ContextInjector uses KeywordIndex
- genai `client.embed()` and `client.embed_batch()` — embedding API

## Relevant Requirements

- REQ-MEM-KEYWORD — This milestone upgrades keyword fallback with vector search primary

## Scope

### In Scope

- VectorIndex type with cosine similarity search
- Embedding generation via genai client.embed()
- Embedding persistence (JSON or binary)
- ContextInjector integration (prefer vectors, fall back to keywords)
- Configurable embedding model

### Out of Scope

- External vector database (Qdrant, Pinecone, etc.)
- Local embedding models
- Real-time index updates during execution

## Technical Constraints

- genai's embed API returns `Vec<f32>` per input
- Cosine similarity is O(n) per query — fine for <10K entries
- Embedding vectors need persistence between runs
