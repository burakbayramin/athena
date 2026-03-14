# M007: Embedding-Based Memory Recall

**Vision:** Memory search uses vector embeddings for semantic similarity, falling back to keywords when embeddings are unavailable.

## Success Criteria

- Memory search returns semantically relevant results via vector similarity
- Keyword fallback continues working when no embedding API is configured
- Embedding vectors are persisted between runs
- All existing tests pass

## Key Risks / Unknowns

- **genai embed API shape** — need to verify return type and batch support
- **Persistence format** — JSON for vectors is large but simple

## Proof Strategy

- genai API → retire in S01 by building VectorIndex with real embed types
- Persistence → retire in S01 by proving round-trip save/load

## Verification Classes

- Contract verification: unit tests for VectorIndex, cosine similarity, persistence
- Integration verification: ContextInjector uses VectorIndex when available
- Operational verification: none
- UAT: none

## Milestone Definition of Done

- VectorIndex implemented with cosine similarity search
- ContextInjector prefers vector search, falls back to keyword
- Embeddings persisted to disk
- All tests pass

## Requirement Coverage

- Covers: REQ-MEM-KEYWORD (upgraded with vector search primary)
- Leaves for later: REQ-LOCAL-EMBED

## Slices

- [x] **S01: VectorIndex with Cosine Similarity** `risk:high` `depends:[]`
  > After this: VectorIndex type stores embeddings, searches by cosine similarity, persists to JSON — proven by unit tests
- [x] **S02: ContextInjector Integration** `risk:medium` `depends:[S01]`
  > After this: ContextInjector prefers VectorIndex over KeywordIndex when embeddings available — proven by integration tests

## Boundary Map

### S01 → S02

Produces:
- VectorIndex type with add/search/persist/load API
- Cosine similarity scoring
- JSON persistence format for embedding vectors

Consumes:
- nothing (first slice)
