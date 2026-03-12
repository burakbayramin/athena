# Architecture Research

**Domain:** Multi-agent AI orchestration CLI (Rust)
**Researched:** 2026-03-12
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CLI Entry Layer                              │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  main.rs  →  cli/mod.rs (clap)  →  commands/{run,init,...}   │   │
│  └──────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                      Orchestration Layer                             │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────────────┐  │
│  │ ProjectAnalyzer │  │  PhasePlanner    │  │ AgentCoordinator   │  │
│  │                 │→ │                  │→ │                    │  │
│  │ Input → Struct  │  │ Graph → Phases   │  │ Dispatch + Monitor │  │
│  └─────────────────┘  └──────────────────┘  └────────────────────┘  │
│                                                     │               │
│  ┌──────────────────────────────────────────────────┼────────────┐  │
│  │                  Phase Execution Loop             │            │  │
│  │   ┌────────────┐   ┌──────────────┐   ┌──────────▼────────┐  │  │
│  │   │ Isolation  │   │  Review      │   │   State Machine   │  │  │
│  │   │ Manager    │←──│  Engine      │←──│   PhaseRunner     │  │  │
│  │   └────────────┘   └──────────────┘   └───────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                         Agent Layer                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ ClaudeClient │  │ GeminiClient │  │  CodexClient │              │
│  │  (Anthropic) │  │  (Google)    │  │  (OpenAI)    │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│            All behind trait: AgentBackend                            │
├─────────────────────────────────────────────────────────────────────┤
│                     Infrastructure Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  GitLayer    │  │ ConfigStore  │  │   ReportWriter           │  │
│  │  (git2-rs)   │  │ (env/toml)   │  │   (JSON/Markdown)        │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `CLI Entry` | Parse args, route to subcommand, bootstrap config | `clap` derive macros, `main.rs` calls into `commands/` |
| `ProjectAnalyzer` | Accept raw input (text/spec/code), extract structured project description | LLM call (Claude) → typed `ProjectSpec` struct |
| `PhasePlanner` | Build dependency graph, emit ordered phase list with parallelism map | DAG traversal, LLM-assisted decomposition → `Vec<Phase>` |
| `AgentCoordinator` | Assign agents to tasks, dispatch concurrently, collect results | Tokio `JoinSet` or actor handles, routes on skill affinity |
| `IsolationManager` | Track file ownership per agent, block double-write, enforce boundaries | In-memory `HashMap<PathBuf, AgentId>` + write-time assertion |
| `PhaseRunner` | Execute one phase as a state machine (Pending→Running→Reviewing→Done/Retry) | Enum-based state, async loop |
| `ReviewEngine` | Route completed agent output to a different agent for cross-review | Agent A output → Agent B prompt → structured pass/fail verdict |
| `AgentBackend` (trait) | Uniform interface: send prompt, receive structured response | `async_trait`, implementations per provider |
| `GitLayer` | Stage files, create per-phase commits, maintain repo cleanliness | `git2` crate (libgit2 bindings) |
| `ConfigStore` | Load API keys, project config from env vars and TOML file | `config` or `figment` crate, typed struct |
| `ReportWriter` | Emit final structured report (phases, verdicts, timing) | Serialize to JSON + optional Markdown render |

## Recommended Project Structure

```
src/
├── main.rs                    # Entry point, tokio::main, delegate to cli
├── cli/
│   ├── mod.rs                 # Clap App definition, subcommand enum
│   └── commands/
│       ├── run.rs             # `athena run <input>` — main orchestration flow
│       ├── init.rs            # `athena init` — scaffold config file
│       └── report.rs         # `athena report` — display last run report
├── analyzer/
│   ├── mod.rs                 # ProjectAnalyzer entry
│   ├── input.rs               # Input normalization (text / file / directory)
│   └── spec.rs                # ProjectSpec type, LLM extraction prompt
├── planner/
│   ├── mod.rs                 # PhasePlanner entry
│   ├── dag.rs                 # Dependency graph, topological sort
│   ├── phase.rs               # Phase / Task types
│   └── assignment.rs          # Agent-to-task skill affinity logic
├── coordinator/
│   ├── mod.rs                 # AgentCoordinator — dispatch + monitor
│   ├── runner.rs              # PhaseRunner state machine
│   ├── isolation.rs           # IsolationManager — file ownership registry
│   └── review.rs              # ReviewEngine — cross-agent review routing
├── agents/
│   ├── mod.rs                 # AgentBackend trait
│   ├── claude.rs              # Anthropic API client
│   ├── gemini.rs              # Google Gemini API client
│   ├── codex.rs               # OpenAI Codex/GPT API client
│   └── types.rs               # Shared: AgentRequest, AgentResponse, ReviewVerdict
├── git/
│   ├── mod.rs                 # GitLayer — high-level operations
│   └── ops.rs                 # Stage, commit, status primitives using git2
├── config/
│   ├── mod.rs                 # ConfigStore — load and validate
│   └── schema.rs              # Typed config schema (API keys, model names)
├── report/
│   ├── mod.rs                 # ReportWriter
│   └── types.rs               # RunReport, PhaseRecord, ReviewRecord
└── error.rs                   # Unified error type (thiserror)
```

### Structure Rationale

- **`analyzer/`:** Isolated because it may grow to support multiple input modes (URL, GitHub repo, raw text). Keeping it separate avoids polluting the planner with parsing concerns.
- **`planner/`:** The DAG logic is algorithmically distinct from execution. Separating it allows unit testing the phase decomposition without touching any async or agent code.
- **`coordinator/`:** The core loop. Isolation and review are subfunctions of coordination — co-located so the PhaseRunner can access both without message-passing overhead.
- **`agents/`:** All three providers implement one trait. The coordinator never touches a concrete client directly — this makes swapping or adding providers trivial.
- **`git/`:** Kept entirely separate because git state is global to the workspace, not scoped to any phase. A dedicated layer prevents coordinator logic from holding Repository handles.

## Architectural Patterns

### Pattern 1: Supervisor / Worker with Concurrent Fan-Out

**What:** The `AgentCoordinator` acts as the supervisor. For tasks within a phase that have no mutual file dependencies, it fans them out to agents concurrently using `tokio::JoinSet`. Results fan back in, the coordinator collects and validates them.

**When to use:** Any phase where tasks target different modules with no shared files. This is the common case for parallel module development.

**Trade-offs:** Maximizes throughput; requires IsolationManager to catch misconfigured overlapping assignments before dispatch (fail fast, not after write).

**Example:**
```rust
// coordinator/mod.rs
let mut set = JoinSet::new();
for task in parallelizable_tasks {
    let agent = self.assign_agent(&task);
    set.spawn(agent.execute(task));
}
while let Some(result) = set.join_next().await {
    self.collect(result?);
}
```

### Pattern 2: Actor Model for Agent Clients

**What:** Each `AgentBackend` implementation runs as a Tokio actor — a spawned task with an `mpsc` receiver and a `Handle` struct on the caller side. This provides natural backpressure, avoids shared mutable state, and allows rate-limit budgets to be enforced inside the actor without locks.

**When to use:** All three provider clients (Claude, Gemini, Codex). Rate limits, retry logic, and token tracking belong inside the actor, not in the coordinator.

**Trade-offs:** Adds a layer of indirection vs. direct async function calls. The tradeoff is worth it: rate-limit state is encapsulated, and the coordinator never blocks waiting on one provider's queue.

**Example:**
```rust
// agents/claude.rs
enum ClaudeMsg { Execute(AgentRequest, oneshot::Sender<AgentResponse>) }

struct ClaudeActor { receiver: mpsc::Receiver<ClaudeMsg>, client: reqwest::Client }

impl ClaudeActor {
    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                ClaudeMsg::Execute(req, reply) => {
                    let resp = self.call_api(req).await;
                    let _ = reply.send(resp);
                }
            }
        }
    }
}
```

### Pattern 3: Enum State Machine for Phase Execution

**What:** Each phase progresses through explicit states encoded as a Rust enum. Transitions are forced through a match statement — invalid transitions are compile-time impossible.

**When to use:** `PhaseRunner` — the review-gate logic. A phase cannot jump from `Running` to `Done` without passing through `Reviewing`. The enum makes this invariant structural, not procedural.

**Trade-offs:** More verbose than a boolean flag system, but eliminates the class of bugs where a phase is incorrectly marked complete without review. Worth it for a correctness-critical system.

**Example:**
```rust
enum PhaseState {
    Pending,
    Running { tasks_remaining: usize },
    AwaitingReview { outputs: Vec<AgentOutput> },
    ReviewFailed { attempt: u32, feedback: String },
    Complete,
}
```

### Pattern 4: Plan-and-Execute with Cheap Executors

**What:** Use the most capable model (Claude) to create the project plan (decomposition, dependency analysis, agent assignments). Use specialized/cheaper models for execution tasks aligned with their strengths. Reserve the capable model for architecture decisions and review synthesis.

**When to use:** Phase planning and cross-review synthesis (Claude), API research and documentation generation (Gemini), boilerplate and repetitive code generation (Codex).

**Trade-offs:** Reduces API cost significantly. Risk: the planner's quality ceiling determines outcome quality — investing in the planning prompt is high leverage.

## Data Flow

### Primary Execution Flow

```
User Input (text / file path)
    │
    ▼
ProjectAnalyzer
    │  LLM call (Claude) → structured ProjectSpec
    ▼
PhasePlanner
    │  DAG construction → topological sort → Vec<Phase> with parallelism flags
    ▼
AgentCoordinator
    │  For each phase (in dependency order):
    │    ├── IsolationManager.register(phase.file_assignments)
    │    ├── fan-out tasks to agents (JoinSet)
    │    │     ├── ClaudeActor.execute(task_a) → AgentOutput
    │    │     ├── GeminiActor.execute(task_b) → AgentOutput
    │    │     └── CodexActor.execute(task_c) → AgentOutput
    │    ├── PhaseRunner transitions: Running → AwaitingReview
    │    ├── ReviewEngine: route outputs to cross-reviewer agent
    │    │     └── ReviewVerdict { passed: bool, feedback: String }
    │    ├── if passed → GitLayer.commit(phase_outputs)
    │    │               PhaseRunner transitions: AwaitingReview → Complete
    │    └── if failed → PhaseRunner transitions: ReviewFailed (retry loop)
    │                    (up to MAX_RETRIES, then halt with error report)
    ▼
ReportWriter
    │  Serialize RunReport (all phases, verdicts, timings) → JSON + Markdown
    ▼
Terminal output + report file written to disk
```

### Config and Credential Flow

```
Environment Variables (ANTHROPIC_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY)
    + athena.toml (model preferences, retry limits, output path)
    │
    ▼
ConfigStore (validated at startup — fail fast if keys missing)
    │
    ▼
Injected into AgentActor constructors at startup
    (agents never re-read env at runtime — stable references only)
```

### Key Data Flows

1. **Analysis → Planning:** `ProjectAnalyzer` emits a `ProjectSpec` (project name, tech domain, list of logical modules, overall goal). `PhasePlanner` ingests this — it does not re-call the LLM for the spec, it uses the typed struct directly.
2. **Phase outputs → Git:** `GitLayer` receives a `Vec<(PathBuf, String)>` (file path + content). It writes files, stages them, and commits with a structured message. It never receives raw `AgentOutput` structs — the coordinator normalizes to file writes first.
3. **Review routing:** `ReviewEngine` maps `AgentId::Claude → AgentId::Gemini` for review (cross-agent, never self-review). The assignment is configured, not dynamic — consistent reviewer pairing matters for audit trails.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single project / local use | Current design is appropriate. All state in-process. No persistence layer needed. |
| Multiple concurrent projects | Promote ConfigStore to a process-global registry; add project namespacing to IsolationManager; each project gets its own AgentCoordinator instance |
| Rate-limit-heavy workloads | Add token-bucket rate limiter inside each AgentActor; expose per-provider concurrency caps in config |
| Very large codebases (>50 phases) | Persist phase state to disk (SQLite via rusqlite) so runs can resume after interruption |

### Scaling Priorities

1. **First bottleneck:** API rate limits. Claude, Gemini, and OpenAI all impose per-minute token caps. The actor model handles this well, but explicit rate budgets need to be added before hammering large projects.
2. **Second bottleneck:** Long-running phases with no progress feedback. Add a progress channel (tokio `watch`) from PhaseRunner to CLI layer so the terminal can show live status without polling.

## Anti-Patterns

### Anti-Pattern 1: Shared Mutable File State Between Agents

**What people do:** Let multiple agents write to a shared output buffer or the same file path, then merge at the end.

**Why it's wrong:** Merge conflicts in generated code are non-deterministic and often unresolvable without human intervention. The whole value of module isolation is eliminating this.

**Do this instead:** IsolationManager assigns file ownership at phase planning time. A task that would write to an already-owned file is rejected at dispatch — before any LLM calls are made.

### Anti-Pattern 2: Self-Review

**What people do:** Ask the same model that generated output to review its own output.

**Why it's wrong:** Models exhibit systematic blind spots. Self-review catches surface errors but misses the same conceptual gaps the original generation made. This is the entire motivation for cross-agent review.

**Do this instead:** ReviewEngine always routes to a different agent. If Claude wrote it, Gemini or Codex reviews it. Configure the review routing table explicitly in config — don't make it dynamic or probabilistic.

### Anti-Pattern 3: Holding Git Repository Handle Across Await Points

**What people do:** Open a `git2::Repository` in a coordinator struct field and hold it across multiple async function calls and `.await` points.

**Why it's wrong:** `git2::Repository` is not `Send`. Holding it across `.await` points in a Tokio multi-thread runtime causes a compile error. Even if worked around, it creates a bottleneck.

**Do this instead:** Open the `Repository` inside the `GitLayer` methods — open, operate, drop within a single synchronous block. Run git operations on a `tokio::task::spawn_blocking` thread if they need to coexist with async code.

### Anti-Pattern 4: Treating Phase Planning as Stateless

**What people do:** Re-run phase planning on every retry when a review fails, allowing the plan to drift.

**Why it's wrong:** Plan instability makes review feedback irrelevant — the next attempt may not even attempt the same task. The coordinator loses the ability to track what was tried.

**Do this instead:** Phase plan is immutable once generated. Only the outputs change on retry. The planner runs exactly once per `athena run` invocation. Retry means re-executing the same task with the review feedback appended to the prompt.

### Anti-Pattern 5: Parsing LLM Output with String Matching

**What people do:** Use regex or `contains()` to extract structured data from free-text LLM responses.

**Why it's wrong:** LLM output format varies between models and versions. String matching is fragile and degrades silently (wrong parse, not an error).

**Do this instead:** All LLM calls that need structured output must use JSON mode (available in Claude, Gemini, and OpenAI APIs). Define response schemas as Rust structs, derive `Deserialize`, and `serde_json::from_str` the response. Fail loudly on parse failure — it surfaces prompt engineering issues early.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Anthropic Claude API | Actor wrapping `reqwest` async client, JSON mode enabled | Use `claude-3-5-sonnet` or newer for planning/review; `claude-3-haiku` for fast tasks |
| Google Gemini API | Same actor pattern, `generativelanguage.googleapis.com` REST endpoint | Gemini 1.5 Flash is cost-effective for research/doc tasks |
| OpenAI Codex / GPT API | Same actor pattern, `api.openai.com/v1/chat/completions` | GPT-4o for code gen; `response_format: json_object` for structured output |
| Local Git Repository | Synchronous `git2` calls in `spawn_blocking` | `git2` requires libgit2; bundled via `libgit2-sys` — no system dep needed |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| CLI → Coordinator | Direct async fn call (not actor) | CLI holds a `Coordinator` value; no message passing needed at this level |
| Coordinator → AgentActor | `mpsc` channel + `oneshot` reply | Enables concurrent dispatch to multiple agents without shared mutable state |
| Coordinator → IsolationManager | Direct method call (sync) | Isolation checks are fast in-memory lookups; no async needed |
| Coordinator → GitLayer | Direct async call in `spawn_blocking` | git2 is sync; must be offloaded from async context |
| PhaseRunner → ReviewEngine | Direct method call (same module) | They are co-located in `coordinator/`; no IPC boundary |
| Coordinator → ReportWriter | Pass `RunReport` by value at end | Fire-and-forget; report is final, not updated incrementally |

## Build Order Implications

The component dependency graph determines safe build order. Components lower in the list depend on those above them.

```
Tier 1 (no deps — build first):
  error.rs          ← unified error type, everything else imports this
  config/           ← keys and settings, injected everywhere
  agents/types.rs   ← shared request/response types

Tier 2 (depend only on Tier 1):
  agents/           ← claude, gemini, codex clients behind AgentBackend trait
  git/              ← GitLayer, depends on config for repo path

Tier 3 (depend on Tier 1 + 2):
  report/           ← ReportWriter, depends on agents/types for verdicts
  analyzer/         ← ProjectAnalyzer, calls AgentBackend (Claude)

Tier 4 (depend on Tier 1-3):
  planner/          ← PhasePlanner, depends on analyzer output types

Tier 5 (depend on Tier 1-4):
  coordinator/      ← PhaseRunner, IsolationManager, ReviewEngine, AgentCoordinator
                       This is the integration tier — touches everything

Tier 6 (outermost shell):
  cli/              ← Commands wire together coordinator + config + report
  main.rs           ← Bootstrap, runtime setup
```

**Implication for phased delivery:** A working slice through tiers 1-3 (config → one agent client → basic git commit) can be built and manually tested without any orchestration logic. This is the recommended Milestone 1 scope. The coordinator (Tier 5) should not be started until all its dependencies are solid.

## Sources

- [AI Agent Orchestration Patterns — Azure Architecture Center (2026)](https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/ai-agent-design-patterns)
- [Actors with Tokio — Alice Ryhl](https://ryhl.io/blog/actors-with-tokio/)
- [Multi-Agent Supervisor Architecture — Databricks (2025)](https://www.databricks.com/blog/multi-agent-supervisor-architecture-orchestrating-enterprise-ai-scale)
- [git2-rs — libgit2 bindings for Rust](https://docs.rs/git2)
- [Rust Concurrency Patterns — OneSignal](https://onesignal.com/blog/rust-concurrency-patterns/)
- [Choosing the Right Multi-Agent Architecture — LangChain Blog](https://blog.langchain.com/choosing-the-right-multi-agent-architecture/)
- [Building a Resilient Type-Safe Rust API Client with reqwest and serde — Leapcell](https://leapcell.io/blog/building-a-resilient-and-type-safe-rust-api-client-with-reqwest-and-serde)
- [Design Patterns for Agentic AI and Multi-Agent Systems — AppsTek Corp](https://appstekcorp.com/staging/8353/blog/design-patterns-for-agentic-ai-and-multi-agent-systems/)

---
*Architecture research for: Multi-agent AI orchestration CLI (Athena)*
*Researched: 2026-03-12*
