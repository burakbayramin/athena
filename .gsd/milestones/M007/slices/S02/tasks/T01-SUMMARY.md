---
id: T01
result: passed
---

# T01: Wire VectorIndex into ContextInjector

Added optional `vector_index` and `query_embedding` fields to `ContextInjector`. `with_vector_search()` builder method attaches both. `read_semantic_results()` prefers vector search when available, falls back to keyword. Hit URIs unified to `Vec<String>` regardless of source. 2 new tests: vector preferred over keyword, fallback to keyword when empty.
