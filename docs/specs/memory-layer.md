# ath-memory: Athena Memory Layer

## Technical Specification v0.1

---

## 1. Problem Statement

Athena is a multi-agent orchestration system that decomposes software projects into phases, routes tasks to specialized AI agents (Claude, Gemini, Codex), executes them with cross-review, and commits results to git. Currently, every run starts from zero:

- **No run history** — Previous runs are forgotten. Agent prompts contain no prior context.
- **No learning** — If an agent discovered a project convention (e.g., "this project uses snake_case for DB columns"), that knowledge is lost after the run ends.
- **No accumulated skill** — Review feedback, common error patterns, and successful architectural decisions are not preserved.
- **No user modeling** — User preferences (preferred frameworks, coding style, naming conventions) must be re-stated each time.

The goal of `ath-memory` is to give Athena a persistent, structured, searchable memory that grows smarter across runs.

---

## 2. Design Philosophy

`ath-memory` combines three proven ideas from the ecosystem:

| Source | What We Take | Why |
|--------|-------------|-----|
| **OpenViking** | Filesystem paradigm for context organization + L0/L1/L2 tiered loading + directory recursive retrieval | Structured hierarchy beats flat vector dumps. Tiered loading controls token cost. |
| **Zvec** | In-process vector database for semantic search | Zero infrastructure. No server. Embeds into the Rust binary. Fast similarity search at scale. |
| **Claude-Mem** | Automatic observation capture + progressive disclosure + session-based memory extraction | Agents should not manually manage memory. The system observes and extracts automatically. |

### Core Principle

> Memory is a local filesystem-like structure, backed by vector embeddings, that grows automatically through agent observations and is loaded into prompts on-demand at the minimum token cost.

---

## 3. Architecture Overview

```
.ath/
├── memory/
│   ├── config.toml                  # Memory system configuration
│   ├── index.zvec                   # Zvec vector index (single file, in-process)
│   │
│   ├── viking://                    # Virtual filesystem (OpenViking paradigm)
│   │   ├── project/                 # Project-level context
│   │   │   ├── .abstract            # L0: "A Rust REST API with PostgreSQL..."
│   │   │   ├── .overview            # L1: Architecture summary, key decisions, tech stack
│   │   │   ├── conventions/         # Discovered coding conventions
│   │   │   ├── architecture/        # Architectural decisions and rationale
│   │   │   └── dependencies/        # Known dependency constraints
│   │   │
│   │   ├── runs/                    # Run history (auto-populated)
│   │   │   ├── <run-uuid-1>/
│   │   │   │   ├── .abstract        # L0: "Built auth module with JWT + refresh tokens"
│   │   │   │   ├── .overview        # L1: Phase summary, agent assignments, outcomes
│   │   │   │   ├── decisions/       # Key decisions made during this run
│   │   │   │   ├── issues/          # Problems encountered and resolutions
│   │   │   │   └── artifacts/       # What files were created/modified
│   │   │   └── <run-uuid-2>/
│   │   │       └── ...
│   │   │
│   │   ├── agents/                  # Agent-specific memory
│   │   │   ├── claude/
│   │   │   │   ├── strengths/       # What Claude does well in this project
│   │   │   │   ├── patterns/        # Recurring patterns in Claude's output
│   │   │   │   └── feedback/        # Review feedback history
│   │   │   ├── gemini/
│   │   │   └── codex/
│   │   │
│   │   └── user/                    # User preferences & habits
│   │       ├── preferences/         # Coding style, framework choices
│   │       ├── corrections/         # Manual corrections user made post-run
│   │       └── instructions/        # Standing instructions ("always use...")
│   │
│   └── observations/                # Raw observation log (append-only)
│       ├── obs_<timestamp>.jsonl    # Rotated observation files
│       └── current.jsonl            # Active observation stream
```

### Data Flow

```
                    ┌─────────────────────────────────────────────┐
                    │                ATH RUN                       │
                    │                                              │
  ath run "..."  ──►│  Input ─► Decompose ─► Route ─► Execute     │
                    │                                    │         │
                    │              ┌──────────────────────┘         │
                    │              ▼                                │
                    │    ┌─────────────────┐                       │
                    │    │   Observers      │ ◄── hooks into every │
                    │    │   (passive)      │     agent call,      │
                    │    └────────┬────────┘     review, and       │
                    │             │               file write        │
                    └─────────────┼────────────────────────────────┘
                                  │
                                  ▼
                    ┌─────────────────────────┐
                    │  Observation Buffer      │  Raw events: agent responses,
                    │  (observations/*.jsonl)  │  review verdicts, file diffs,
                    └────────────┬────────────┘  error messages, timing data
                                 │
                                 ▼
                    ┌─────────────────────────┐
                    │  Memory Extractor        │  Post-run async process:
                    │  (LLM-powered)           │  - Summarize run → L0/L1/L2
                    │                          │  - Extract conventions
                    │                          │  - Update agent profiles
                    │                          │  - Detect user preferences
                    └────────────┬────────────┘
                                 │
                        ┌────────┴────────┐
                        ▼                 ▼
              ┌──────────────┐   ┌──────────────┐
              │ viking://     │   │ index.zvec   │
              │ (structured   │   │ (vector      │
              │  markdown)    │   │  embeddings) │
              └──────────────┘   └──────────────┘
                        │                 │
                        └────────┬────────┘
                                 │
                                 ▼
                    ┌─────────────────────────┐
                    │  Context Injector        │  Before each agent call:
                    │  (reads from memory)     │  - Retrieve relevant context
                    │                          │  - Apply L0 → L1 → L2 loading
                    │                          │  - Inject into agent prompt
                    └─────────────────────────┘
```

---

## 4. Core Components

### 4.1 Viking Context Store (`ath-memory/store/`)

A virtual filesystem implemented as a directory tree of Markdown files. Each node has three layers:

| Layer | File | Token Budget | Purpose |
|-------|------|-------------|---------|
| **L0 (Abstract)** | `.abstract` | ~50-100 tokens | One-line summary for quick relevance checks. Used in initial retrieval. |
| **L1 (Overview)** | `.overview` | ~500-2000 tokens | Structured summary with key facts. Used for agent planning and decision-making. |
| **L2 (Detail)** | `*.md` files | Full content | Complete information. Loaded only when the agent explicitly needs deep context. |

**Why this matters**: A typical Athena run might touch 20+ memory nodes. Loading all at L2 would consume 40k+ tokens. With tiered loading, the Context Injector starts at L0 (~2k tokens for 20 nodes), promotes to L1 only for relevant nodes (~4k tokens for 5 nodes), and loads L2 only when an agent specifically requests deep context.

#### Viking URI Scheme

Every memory entry is addressable:

```
viking://project/.abstract
viking://runs/abc123/decisions/use-jwt-over-session.md
viking://agents/claude/patterns/prefers-builder-pattern.md
viking://user/preferences/naming-conventions.md
```

### 4.2 Vector Index (`ath-memory/index/`)

Uses Zvec as an in-process vector database. No server, no Docker, no background process.

```rust
// Conceptual Rust integration (zvec has Python/Node bindings, 
// we'd use its C API via FFI or a Rust wrapper)

pub struct MemoryIndex {
    collection: ZvecCollection,
}

impl MemoryIndex {
    /// Index a memory entry at all layers
    pub fn upsert(&mut self, uri: &VikingUri, content: &LayeredContent) -> Result<()> {
        // Embed L0 for fast retrieval
        let l0_vec = self.embed(&content.abstract_text)?;
        self.collection.upsert(Doc {
            id: format!("{}::L0", uri),
            vectors: HashMap::from([("embedding", l0_vec)]),
            attributes: HashMap::from([
                ("uri", uri.to_string()),
                ("layer", "L0"),
                ("content", content.abstract_text.clone()),
            ]),
        })?;
        
        // Embed L1 for deeper retrieval
        let l1_vec = self.embed(&content.overview_text)?;
        self.collection.upsert(Doc {
            id: format!("{}::L1", uri),
            vectors: HashMap::from([("embedding", l1_vec)]),
            attributes: HashMap::from([
                ("uri", uri.to_string()),
                ("layer", "L1"),
                ("content", content.overview_text.clone()),
            ]),
        })?;
        
        Ok(())
    }
    
    /// Retrieve relevant context for a query
    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<MemoryHit>> {
        let query_vec = self.embed(query)?;
        let results = self.collection.query(
            VectorQuery("embedding", vector: query_vec),
            topk: top_k,
        )?;
        // Returns URIs + layer + scores
        Ok(results.into_iter().map(MemoryHit::from).collect())
    }
}
```

**Embedding Strategy**:
- L0 abstracts are embedded as-is (short, high-signal)
- L1 overviews are chunked into ~512 token segments, each embedded separately
- L2 content is NOT embedded (too expensive, too noisy). Reached only via URI navigation after L0/L1 hits.

### 4.3 Observation System (`ath-memory/observe/`)

Inspired by Claude-Mem's automatic observation capture. Hooks into the execution pipeline to record events without agent intervention.

#### Observation Types

```rust
pub enum ObservationType {
    // Agent interactions
    AgentRequest { agent: AgentId, task: TaskId, prompt_hash: String },
    AgentResponse { agent: AgentId, task: TaskId, files_produced: Vec<PathBuf>, token_usage: TokenCount },
    
    // Review events
    ReviewVerdict { reviewer: AgentId, author: AgentId, verdict: Verdict, feedback: String },
    ReviewRetry { task: TaskId, attempt: u8, feedback: String },
    
    // File operations
    FileCreated { path: PathBuf, phase: PhaseId },
    FileModified { path: PathBuf, diff_summary: String },
    
    // Errors & recovery
    AgentError { agent: AgentId, error: String, recovered: bool },
    ConflictDetected { paths: Vec<PathBuf>, phases: Vec<PhaseId> },
    
    // Decisions
    RoutingDecision { task: TaskId, chosen_agent: AgentId, reason: String },
    DecompositionResult { phases: usize, parallel_groups: usize, critical_path: usize },
}
```

#### Observation Storage

Observations are written as append-only JSONL files:

```jsonl
{"ts":"2025-03-14T10:00:00Z","run":"abc123","type":"AgentResponse","agent":"claude","task":"t1","files":["src/auth.rs"],"tokens":{"input":2400,"output":1800}}
{"ts":"2025-03-14T10:00:05Z","run":"abc123","type":"ReviewVerdict","reviewer":"gemini","author":"claude","verdict":"pass","feedback":"Clean implementation, good error handling"}
{"ts":"2025-03-14T10:01:00Z","run":"abc123","type":"FileCreated","path":"src/auth.rs","phase":"p1"}
```

### 4.4 Memory Extractor (`ath-memory/extract/`)

Runs asynchronously after each Athena run completes. Uses an LLM to distill raw observations into structured memory.

#### Extraction Pipeline

```
Raw Observations (JSONL)
        │
        ▼
┌───────────────────┐
│ 1. Run Summary    │  Compress all observations into a run-level
│    Generator      │  L0 abstract + L1 overview
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│ 2. Convention     │  Detect patterns: "this project uses X",
│    Detector       │  "the user prefers Y", "agent Z struggles with W"
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│ 3. Decision       │  Extract architectural and design decisions
│    Extractor      │  with rationale ("chose JWT because...")
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│ 4. Agent Profile  │  Update per-agent performance data:
│    Updater        │  review pass rates, common feedback themes,
└───────┬───────────┘  token efficiency
        │
        ▼
┌───────────────────┐
│ 5. Vector Index   │  Embed new/updated entries into Zvec
│    Updater        │
└───────────────────┘
```

#### Extraction Prompts (Example)

**Run Summary Extraction:**

```
You are analyzing an Athena run. Given the following observations 
from a project build run, produce:

1. ABSTRACT (1 sentence, max 20 words): What was accomplished?
2. OVERVIEW (max 300 words): 
   - What phases were executed?
   - Which agents handled what?
   - What key decisions were made?
   - Were there any issues or retries?
   - What files were created/modified?

Observations:
{observations_jsonl}

Respond in JSON:
{
  "abstract": "...",
  "overview": "...",
  "conventions_detected": ["...", "..."],
  "decisions": [{"decision": "...", "rationale": "..."}],
  "issues": [{"issue": "...", "resolution": "..."}]
}
```

### 4.5 Context Injector (`ath-memory/inject/`)

The bridge between memory and agent prompts. Runs BEFORE each agent call.

#### Injection Strategy: Progressive Disclosure

```rust
pub struct ContextInjector {
    store: VikingStore,
    index: MemoryIndex,
}

impl ContextInjector {
    /// Build context for an agent prompt
    pub fn build_context(
        &self,
        task: &TaskSpec,
        agent: &AgentId,
        token_budget: usize,  // e.g., 4000 tokens for context
    ) -> Result<InjectedContext> {
        let mut context = InjectedContext::new(token_budget);
        
        // Layer 1: Always inject (costs ~200 tokens)
        // Project identity — what is this project?
        let project_abstract = self.store.read("viking://project/.abstract")?;
        context.add_section("PROJECT", &project_abstract, Priority::Required);
        
        // Layer 2: User standing instructions (costs ~200-500 tokens)
        let instructions = self.store.read("viking://user/instructions/")?;
        context.add_section("USER_INSTRUCTIONS", &instructions, Priority::Required);
        
        // Layer 3: Semantic search for task-relevant context (costs ~500-2000 tokens)
        let query = format!("{} {}", task.description, task.skill_tags.join(" "));
        let hits = self.index.search(&query, 10)?;
        
        for hit in hits {
            if context.remaining_budget() < 100 { break; }
            
            match hit.layer {
                Layer::L0 => {
                    // Check if worth promoting to L1
                    if hit.score > 0.85 {
                        let l1 = self.store.read(&format!("{}/.overview", hit.uri.parent()))?;
                        context.add_section(&hit.uri.to_string(), &l1, Priority::High);
                    } else {
                        context.add_section(&hit.uri.to_string(), &hit.content, Priority::Medium);
                    }
                }
                Layer::L1 => {
                    context.add_section(&hit.uri.to_string(), &hit.content, Priority::High);
                }
                _ => {} // L2 never auto-loaded
            }
        }
        
        // Layer 4: Agent-specific memory (costs ~200-500 tokens)
        let agent_patterns = self.store.read(
            &format!("viking://agents/{}/patterns/.overview", agent)
        )?;
        context.add_section("AGENT_HISTORY", &agent_patterns, Priority::Medium);
        
        // Layer 5: Recent run context (costs ~200-500 tokens)
        let recent_run = self.store.most_recent_run()?;
        context.add_section("LAST_RUN", &recent_run.abstract_text, Priority::Low);
        
        Ok(context)
    }
}
```

#### Injected Prompt Structure

The injected context is prepended to the agent's task prompt:

```
<athena_context>
  <project>
    A Rust REST API for earthquake monitoring with PostgreSQL backend, 
    deployed on Hostinger VPS.
  </project>
  
  <user_instructions>
    - Always use snake_case for database columns
    - Prefer explicit error types over anyhow
    - Write integration tests for all API endpoints
  </user_instructions>
  
  <relevant_context>
    <entry uri="viking://runs/prev-run/decisions/use-axum.md" relevance="0.92">
      Chose Axum over Actix-web for the HTTP framework. Rationale: better 
      tower middleware ecosystem, simpler handler signatures, active maintenance.
    </entry>
    <entry uri="viking://project/conventions/error-handling.md" relevance="0.88">
      Project uses thiserror for library errors, custom AppError for HTTP 
      responses with consistent JSON error format.
    </entry>
  </relevant_context>
  
  <agent_notes agent="claude">
    Claude tends to over-engineer error types in this project. Keep it 
    simple — max 2 levels of error nesting.
  </agent_notes>
</athena_context>

--- TASK ---
{actual_task_prompt}
```

---

## 5. Retrieval Strategy

Inspired by OpenViking's Directory Recursive Retrieval, adapted for Athena's memory structure.

### 5.1 Multi-Stage Retrieval

```
Query: "implement rate limiting middleware"
                │
                ▼
┌──────────────────────────┐
│ Stage 1: Intent Analysis │  → ["rate limiting", "middleware", "HTTP", "tower"]
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│ Stage 2: Vector Search   │  Search L0 abstracts across all viking:// paths
│ (Zvec, top-20 L0 hits)  │  → Finds: project/conventions, runs/xyz/decisions,
└──────────┬───────────────┘    agents/claude/patterns
           │
           ▼
┌──────────────────────────┐
│ Stage 3: Directory Focus │  Identify top directories from L0 hits
│                          │  → viking://project/conventions/ (3 hits)
│                          │  → viking://runs/xyz/ (2 hits)
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│ Stage 4: L1 Drill-down   │  Load L1 overviews of focused directories
│                          │  Re-rank with L1 content against query
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│ Stage 5: Result Assembly │  Return ordered results with layer info
│                          │  Context Injector uses these for prompt building
└──────────────────────────┘
```

### 5.2 Retrieval Observability

Every retrieval produces a trajectory log (inspired by OpenViking's visualized retrieval):

```json
{
  "query": "implement rate limiting middleware",
  "intents": ["rate limiting", "middleware", "HTTP"],
  "trajectory": [
    {"stage": "vector_search", "hits": 20, "top_uri": "viking://project/conventions/", "top_score": 0.94},
    {"stage": "directory_focus", "directories": ["viking://project/conventions/", "viking://runs/xyz/"]},
    {"stage": "l1_drilldown", "promoted": 3, "demoted": 5},
    {"stage": "result", "final_count": 5, "total_tokens": 1847}
  ],
  "token_cost": {
    "l0_loaded": 12,
    "l1_promoted": 3,
    "l2_loaded": 0,
    "total_tokens": 1847
  }
}
```

This trajectory is stored in `.ath/runs/<uuid>/retrievals/` for debugging and optimization.

---

## 6. Memory Lifecycle

### 6.1 Write Path (Post-Run)

```
Run Completes
     │
     ├──► Flush observation buffer to observations/*.jsonl
     │
     ├──► Run Memory Extractor (async, LLM-powered)
     │    ├── Generate run summary → viking://runs/<uuid>/
     │    ├── Detect conventions → viking://project/conventions/
     │    ├── Extract decisions → viking://runs/<uuid>/decisions/
     │    └── Update agent profiles → viking://agents/*/
     │
     └──► Update vector index (Zvec)
          ├── Embed new L0 abstracts
          └── Embed new L1 overviews
```

### 6.2 Read Path (Pre-Agent-Call)

```
Agent Call Prepared
     │
     ├──► Context Injector activates
     │    ├── Load project identity (L0, always)
     │    ├── Load user instructions (always)
     │    ├── Semantic search for task context (Zvec)
     │    ├── Progressive L0 → L1 promotion
     │    └── Load agent-specific notes
     │
     └──► Inject context into agent prompt
          └── Respect token budget
```

### 6.3 Maintenance

```
ath memory gc          # Remove observation files older than 30 days
ath memory compact     # Re-summarize and merge old run entries
ath memory rebuild     # Rebuild Zvec index from viking:// store
ath memory stats       # Show memory size, entry count, index health
```

---

## 7. CLI Interface

```bash
# View memory tree
ath memory tree
# Output:
# viking://
# ├── project/           (3 entries, 2.1k tokens at L1)
# ├── runs/              (12 runs, latest: 2h ago)
# ├── agents/            (3 agents tracked)
# └── user/              (5 preferences, 2 instructions)

# Search memory
ath memory search "authentication pattern"
# Output:
# [0.94] viking://runs/abc123/decisions/use-jwt.md (L1)
# [0.87] viking://project/conventions/auth-middleware.md (L0)
# [0.72] viking://agents/claude/patterns/auth-impl.md (L1)

# Read specific entry
ath memory read viking://project/conventions/error-handling.md

# Manual memory entry
ath memory add viking://user/instructions/always-use-tokio.md \
  "Always use tokio runtime with multi-thread flavor. Never use async-std."

# View run memory
ath memory run abc123

# Export memory for debugging
ath memory export --format json > memory-dump.json

# View retrieval trajectories
ath memory trajectories --run abc123

# Show token cost analysis
ath memory cost --last 5
# Output:
# Run abc123: 1,847 tokens injected (12 L0, 3 L1, 0 L2)
# Run def456: 2,103 tokens injected (15 L0, 4 L1, 1 L2)
# Average: 1,975 tokens/run (vs ~40k without tiered loading)
```

---

## 8. Crate Structure

```
athena/
├── ath-memory/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                 # Public API
│   │   │
│   │   ├── store/                 # Viking Context Store
│   │   │   ├── mod.rs
│   │   │   ├── viking_uri.rs      # URI parsing and resolution
│   │   │   ├── layered_content.rs # L0/L1/L2 content types
│   │   │   ├── reader.rs          # Read from viking:// paths
│   │   │   └── writer.rs          # Write to viking:// paths
│   │   │
│   │   ├── index/                 # Vector Index (Zvec)
│   │   │   ├── mod.rs
│   │   │   ├── embedder.rs        # Text → vector embedding (API call)
│   │   │   ├── zvec_wrapper.rs    # Zvec FFI wrapper
│   │   │   └── search.rs          # Multi-stage retrieval logic
│   │   │
│   │   ├── observe/               # Observation System
│   │   │   ├── mod.rs
│   │   │   ├── observer.rs        # Event hooks + buffer
│   │   │   ├── types.rs           # ObservationType enum
│   │   │   └── storage.rs         # JSONL append-only writer
│   │   │
│   │   ├── extract/               # Memory Extractor
│   │   │   ├── mod.rs
│   │   │   ├── run_summary.rs     # Run → L0/L1 summary
│   │   │   ├── convention.rs      # Convention detection
│   │   │   ├── decision.rs        # Decision extraction
│   │   │   └── agent_profile.rs   # Agent profile updates
│   │   │
│   │   ├── inject/                # Context Injector
│   │   │   ├── mod.rs
│   │   │   ├── injector.rs        # Main injection logic
│   │   │   ├── budget.rs          # Token budget management
│   │   │   └── template.rs        # Prompt template rendering
│   │   │
│   │   └── cli/                   # CLI subcommands
│   │       ├── mod.rs
│   │       ├── tree.rs
│   │       ├── search.rs
│   │       ├── read.rs
│   │       └── stats.rs
│   │
│   └── tests/
│       ├── store_tests.rs
│       ├── retrieval_tests.rs
│       └── extraction_tests.rs
```

---

## 9. Integration Points with Existing Athena Modules

### 9.1 ath-orchestrator/phase_runner.rs

**Before** (current):
```rust
async fn execute_task(&self, task: &TaskSpec, agent: &dyn Agent) -> Result<TaskResult> {
    let prompt = self.build_prompt(task);
    agent.execute(&prompt).await
}
```

**After** (with memory):
```rust
async fn execute_task(&self, task: &TaskSpec, agent: &dyn Agent) -> Result<TaskResult> {
    // Inject memory context
    let memory_context = self.memory.inject(task, agent.id(), TOKEN_BUDGET)?;
    let prompt = self.build_prompt_with_context(task, &memory_context);
    
    // Execute with observation
    let _guard = self.memory.observe_task(task, agent.id());
    let result = agent.execute(&prompt).await?;
    
    // Record result observation
    self.memory.observe_result(task, agent.id(), &result);
    
    Ok(result)
}
```

### 9.2 ath-orchestrator/coordinator.rs

**After run completion:**
```rust
async fn post_run(&self, run: &CompletedRun) -> Result<()> {
    // Existing: generate report
    self.reporter.generate(run)?;
    
    // Existing: git commit
    self.git.stage_and_commit(run)?;
    
    // NEW: extract and store memory
    self.memory.extract_and_store(run).await?;
    
    Ok(())
}
```

### 9.3 ath-planner/decompose/

Memory-aware decomposition:
```rust
async fn decompose(&self, spec: &ProjectSpec) -> Result<ExecutionPlan> {
    // Load project memory for decomposition context
    let project_context = self.memory.inject_for_planning(spec)?;
    
    // Include in decomposition prompt:
    // - Previous run structures (what phase patterns worked)
    // - Known conventions (avoid re-discovering)
    // - Agent strengths (route more effectively)
    let plan = self.llm.decompose_with_context(spec, &project_context).await?;
    
    Ok(plan)
}
```

---

## 10. Token Economics

### Cost Comparison

| Scenario | Without Memory | With Memory (L0/L1) | Savings |
|----------|---------------|---------------------|---------|
| 5-phase run, 3 tasks/phase | 0 context tokens | ~10k tokens total | N/A (no context before) |
| Re-running similar project | 0 (starts from scratch) | ~2k tokens (conventions + decisions) | Fewer retries, fewer review failures |
| Agent prompt augmentation | Base prompt only | +500-2000 tokens/call | Better first-pass quality |

### Token Budget Defaults

```toml
# .ath/memory/config.toml

[injection]
total_budget = 4000          # Max tokens injected per agent call
project_identity = 200       # Reserved for project abstract
user_instructions = 500      # Reserved for standing instructions
semantic_results = 2500      # Budget for search-based context
agent_notes = 300            # Reserved for agent-specific memory
recent_run = 500             # Budget for last run context

[extraction]
max_observations_per_run = 500   # Truncate if run produces more
summary_model = "claude"         # Which agent summarizes runs
summary_max_tokens = 2000        # Max tokens for summary generation

[index]
embedding_model = "text-embedding-3-small"  # Or local model
embedding_dimension = 1536
l0_chunk_size = 100             # L0 abstracts: embed whole
l1_chunk_size = 512             # L1 overviews: chunk at 512 tokens
```

---

## 11. Implementation Phases

### Phase 1: Foundation (MVP)
- [ ] Implement Viking Store (read/write L0/L1/L2 markdown files)
- [ ] Implement VikingUri parser
- [ ] Basic observation system (append JSONL during runs)
- [ ] Simple post-run summary extraction (LLM call)
- [ ] Basic Context Injector (project identity + last run summary)
- [ ] CLI: `ath memory tree`, `ath memory read`

### Phase 2: Vector Search
- [ ] Integrate Zvec (or fallback: SQLite FTS5 for text search)
- [ ] Implement embedding pipeline (API calls to embedding model)
- [ ] Multi-stage retrieval (L0 search → directory focus → L1 drill-down)
- [ ] Token budget management in Context Injector
- [ ] CLI: `ath memory search`

### Phase 3: Smart Extraction
- [ ] Convention detector
- [ ] Decision extractor
- [ ] Agent profile updater
- [ ] User preference learner
- [ ] Retrieval trajectory logging

### Phase 4: Advanced Features
- [ ] Memory compaction (merge old runs into consolidated summaries)
- [ ] Cross-project memory sharing (common patterns across repos)
- [ ] Memory diffing (`ath memory diff run1 run2`)
- [ ] Interactive memory editor (TUI for manual memory curation)
- [ ] Retrieval trajectory visualization

---

## 12. Open Questions

1. **Zvec vs SQLite FTS5**: Zvec gives true semantic search but adds a C++ build dependency. SQLite FTS5 is simpler but keyword-only. Hybrid approach (FTS5 + optional Zvec)?

2. **Embedding API dependency**: Memory extraction and indexing require an embedding API. Should this be the same API key as the agent LLMs, or separate config?

3. **Memory size limits**: How much memory is too much? Should we auto-compact after N runs? After M megabytes?

4. **Privacy**: Some observations may contain sensitive code. Should there be a `<private>` tag system (like Claude-Mem) to exclude content from memory?

5. **Multi-project memory**: Should `ath-memory` support a global user profile that spans projects, separate from per-project memory?

---

## 13. References

- [OpenViking](https://github.com/volcengine/OpenViking) — Filesystem paradigm for context, L0/L1/L2 tiered loading, directory recursive retrieval
- [Zvec](https://github.com/alibaba/zvec) — In-process vector database, zero-infrastructure semantic search
- [Claude-Mem](https://github.com/thedotmack/claude-mem) — Automatic observation capture, progressive disclosure, session-based memory extraction
- Athena existing modules: `ath-planner`, `ath-orchestrator`, `ath-types`, `ath-git`, `ath-cli`