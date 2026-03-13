# Project Research Summary

**Project:** Athena — Multi-agent AI Orchestration CLI
**Domain:** Multi-agent AI orchestration CLI (Rust, heterogeneous model coordination)
**Researched:** 2026-03-12
**Confidence:** HIGH

## Executive Summary

Athena is a Rust CLI that orchestrates three heterogeneous AI agents (Claude, Gemini, Codex) to decompose and autonomously execute software development projects through a structured phase pipeline with cross-agent review gates. The research confirms this is a well-precedented problem domain — tools like CrewAI, MetaGPT, LangGraph, and AutoGen have established the core patterns — but Athena's value proposition is differentiated by three specific choices no current competitor combines: skill-based routing across vendors, strict module isolation with file ownership enforcement, and typed inter-agent schemas validated at every boundary. The Rust stack (tokio + genai + git2 + clap) is an unusually strong fit for this domain: async fan-out maps directly to parallel agent dispatch, the type system enforces the schema contracts that multi-agent reliability depends on, and single-binary distribution eliminates the Python packaging friction that plagues every existing tool.

The recommended build order is infrastructure-first. The architecture research identifies a clear 6-tier dependency graph where config, error types, and shared agent message types must exist before any agent client can be implemented, and the coordinator cannot be started until all its dependencies are solid. This maps well to a phased roadmap: get one agent calling an API and committing to git before writing any orchestration logic. The phase decomposition engine and typed schema layer are the highest-leverage investments — everything else depends on getting those right.

The critical risk cluster is well-documented by production research: untyped cross-agent handoffs cause 42% of multi-agent failures; agent drift degrades quality measurably after ~73 interactions; infinite review loops are a common budget-burning failure mode; and API reliability assumptions kill long-running runs. All four of these risks have clear prevention strategies that must be implemented early — they cannot be retrofitted. The good news: the Rust type system, combined with serde deserialization and enum state machines, makes several of these structural safeguards compile-time guarantees rather than runtime policies.

## Key Findings

### Recommended Stack

The Rust async ecosystem is a near-perfect fit for multi-agent orchestration. Tokio (1.50, current) provides the work-stealing scheduler needed for concurrent I/O-bound agent calls; `genai` 0.5 provides a unified client for all three required providers (Claude, Gemini, OpenAI/Codex) behind a single API, eliminating the cost of maintaining three separate SDK integrations. The `git2` crate gives in-process git operations without a system dependency. The `anyhow`/`thiserror` split (anyhow at binary boundary, thiserror in internal modules) is the correct Rust error handling pattern for this architecture.

No serious alternatives exist for the core stack choices. The only notable risk area is `genai`'s relative youth (v0.5.0, Jan 2026) — it is actively maintained but less battle-tested than the rest of the stack. If provider-specific features outpace genai's adapter support, direct `reqwest` calls are the escape hatch.

**Core technologies:**
- `tokio` 1.x: async runtime — de-facto standard; entire ecosystem targets it; `tokio::spawn` for agent fan-out, `JoinSet` for result collection
- `genai` 0.5: unified AI provider client — single crate covering Claude + Gemini + OpenAI; avoids 3 separate SDK integrations
- `clap` 4.5: CLI argument parsing — derive macros, env var integration, shell completions; no serious competitor for user-facing CLIs
- `git2` 0.20: in-process git operations — no system dependency (bundled libgit2); typed errors; battle-tested
- `serde` + `serde_json` 1.x: serialization — required for typed inter-agent schemas, config, phase state, and reports
- `tracing` 0.1: async-aware logging — spans preserve which agent produced which log line in concurrent execution
- `anyhow` + `thiserror` 2.x: error handling — complementary; anyhow at the binary level, thiserror for typed domain errors
- `futures` 0.3: async combinators — `join_all` and `FuturesUnordered` for concurrent agent dispatch

### Expected Features

The feature landscape is well-researched with strong consensus from 15+ sources. The MVP is clearly bounded: 12 P1 features that form the irreducible core of the value proposition. All are load-bearing — removing any one breaks the core loop.

**Must have (table stakes — v1):**
- Structured phase decomposition with dependency ordering — Athena's core value; everything else depends on this
- Cross-agent review gate (blocking, cross-vendor) — quality mechanism; must ship from day one
- Module isolation via file ownership list — prevents parallel write conflicts
- Agent routing (Claude: architecture/logic, Gemini: research/docs, Codex: boilerplate) — the differentiator
- Typed inter-agent message schemas — foundational reliability; the #1 multi-agent failure mode without this
- Natural language and spec file input — the entry point
- Automatic retry loop on review failure (max 3 attempts) — makes execution autonomous
- Per-phase git commits — basic traceability; non-negotiable
- Terminal progress reporting — users abandon tools that appear hung
- API key config via env vars and config file — standard BYO-key UX
- Structured final report — the "receipt" of what was built
- Error reporting with phase-level context — distinguishes API vs review vs schema failures

**Should have (competitive differentiators — v1.x):**
- Dry-run / planning mode — high value once decomposition is stable; prevents surprise API costs
- Token usage and cost reporting per phase — users cited pricing opacity as a top pain point
- True parallel phase execution for independent modules — requires isolation proven reliable first
- Git worktree isolation — upgrade from file-list isolation once parallel execution lands
- Verbose mode / full agent transcript flag (`--verbose`)

**Defer (v2+):**
- Resume interrupted execution — requires persistent checkpoint state; meaningful architecture addition
- Spec-driven input format (requirements.md / design.md) — standardize after usage patterns emerge
- Plugin system for custom agents — premature before the three-agent model is proven
- Local/self-hosted model support — defer until local models reach capability parity

### Architecture Approach

The architecture is a four-layer system: CLI entry → orchestration (ProjectAnalyzer, PhasePlanner, AgentCoordinator) → agent layer (three providers behind a single `AgentBackend` trait) → infrastructure (GitLayer, ConfigStore, ReportWriter). The coordinator is the integration tier — it touches everything and should not be built until all its dependencies are solid. Three key patterns govern the design: Supervisor/Worker with tokio `JoinSet` fan-out for concurrent agent dispatch; Actor Model per agent client (each provider runs as a spawned task with `mpsc` channels, encapsulating rate limits and retry logic); and an enum State Machine for phase execution that makes invalid state transitions compile-time impossible.

The build tier ordering (6 tiers, from shared error types to CLI shell) is the single most important architectural insight: it directly maps to safe phase delivery. A working slice through tiers 1-3 (config → one agent client → basic git commit) can be manually tested before any orchestration logic exists.

**Major components:**
1. `ProjectAnalyzer` — accept raw input, extract typed `ProjectSpec` via LLM call (Claude)
2. `PhasePlanner` — build dependency DAG, emit ordered `Vec<Phase>` with parallelism flags
3. `AgentCoordinator` — dispatch tasks via `JoinSet`, monitor results, own the phase execution loop
4. `IsolationManager` — track file ownership per agent (`HashMap<PathBuf, AgentId>`), block double-write at dispatch time
5. `PhaseRunner` — enum state machine: `Pending → Running → AwaitingReview → Complete/ReviewFailed`
6. `ReviewEngine` — route agent output to cross-reviewer; enforce cross-vendor pairing; structured pass/fail verdict
7. `AgentBackend` (trait) — uniform interface across Claude/Gemini/Codex; actor implementation per provider
8. `GitLayer` — stage files, per-phase commits via `git2` in `spawn_blocking`; never hold `Repository` across await points

### Critical Pitfalls

1. **Untyped cross-agent handoffs** — 42% of multi-agent failures; define Rust structs with serde at every agent boundary; validate on deserialization; never forward unvalidated output
2. **Agent drift across extended runs** — quality degrades measurably after ~73 interactions; pass structured project manifest + task-specific context only; never pass full phase history to subsequent agents
3. **Infinite review loops** — enforce hard maximum iteration count (default: 3) in code, not in prompts; distinguish blocking vs advisory review failures; track convergence delta
4. **Runaway token costs** — implement per-run budget tracking from day one in the API abstraction layer; dry-run cost estimation before execution; hard cap with halt-and-report
5. **Module isolation boundary violations under shared dependencies** — identify shared interface contracts in a dedicated contracts phase before parallel assignment; agents receive contracts file (read-only); validate at merge time
6. **API reliability** — implement exponential backoff (1s to 60s cap) for 429/500/529; circuit breaker per provider after N consecutive failures; never retry 400/401/403 (client errors); respect `Retry-After` headers
7. **Phase decomposition quality** — structural validation rules before execution: no circular dependencies, no phase referencing output from a later phase, all shared contracts appear before their consumers; dry-validation pass before any API calls

## Implications for Roadmap

Based on the 6-tier build order from architecture research and the feature dependency graph from feature research, the natural phase structure is: build the foundation (config, types, errors), then infrastructure (one agent + git), then the core engine (decomposition + phase runner + review), then integration and polish.

### Phase 1: Foundation and Shared Types
**Rationale:** Architecture research identifies tiers 1-2 as zero-dependency; everything else imports from here. Building these first prevents circular dependency issues and allows all later phases to compile against stable interfaces. This is also where the typed inter-agent schemas live — the #1 reliability investment per pitfalls research.
**Delivers:** `error.rs` (unified error type), `config/` (ConfigStore, API key loading via dotenvy + clap), `agents/types.rs` (AgentRequest, AgentResponse, ReviewVerdict, ProjectSpec typed structs), `report/types.rs` (RunReport, PhaseRecord)
**Addresses:** API key config (table stakes), typed inter-agent schemas (P1 feature), error reporting foundation
**Avoids:** Untyped cross-agent handoffs pitfall — schema contracts defined here before any agent integration

### Phase 2: Agent Clients and Git Layer
**Rationale:** Architecture tiers 2-3; these depend only on Phase 1 types. Building agent clients as actors (mpsc pattern) now encapsulates rate limiting and retry logic before the coordinator tries to use them. Git layer is independent and simple — get it right in isolation before it's called from orchestration paths.
**Delivers:** `agents/` (ClaudeActor, GeminiActor, CodexActor implementing AgentBackend trait, actor pattern with mpsc channels), `git/` (GitLayer with stage/commit/status using git2 in spawn_blocking)
**Uses:** tokio actors, genai crate, git2, spawn_blocking pattern
**Implements:** AgentBackend trait, Actor Model pattern
**Avoids:** API reliability pitfall — exponential backoff, circuit breaker, and Retry-After handling built into actors here; git2 Repository handle pitfall (never held across await)

### Phase 3: Project Analyzer and Phase Planner
**Rationale:** Architecture tiers 3-4; first use of agent clients for real orchestration logic. The decomposition engine is Athena's core value — it must be solid and validated before the coordinator tries to execute plans. Structural validation rules (no circular deps, contracts-before-consumers) belong here, not as a later addition.
**Delivers:** `analyzer/` (ProjectAnalyzer: input normalization, LLM extraction to ProjectSpec), `planner/` (PhasePlanner: DAG construction, topological sort, assignment logic, structural validation)
**Addresses:** Structured phase decomposition (P1), dependency analysis (differentiator), agent routing/skill affinity
**Avoids:** Phase decomposition quality pitfall — structural validator catches circular deps and missing contracts before execution; agent drift pitfall (project manifest design happens here)

### Phase 4: Core Orchestration — Phase Runner and Review Engine
**Rationale:** Architecture tier 5 (integration tier) — cannot begin until all of tiers 1-4 are solid. This is the highest-complexity phase. The enum state machine for PhaseRunner, convergence-guarded ReviewEngine, and IsolationManager are all built here. This phase makes the end-to-end pipeline work for the first time.
**Delivers:** `coordinator/` (PhaseRunner enum state machine, IsolationManager with file ownership registry, ReviewEngine with convergence guards and cross-vendor routing, AgentCoordinator with JoinSet fan-out)
**Implements:** Supervisor/Worker pattern, PhaseState enum machine, cross-agent review with max-iteration enforcement
**Avoids:** Infinite review loops pitfall (convergence guards mandatory in initial implementation), module isolation boundary violations pitfall (IsolationManager enforces at dispatch time, before any LLM calls)

### Phase 5: CLI Shell, Progress Reporting, and Final Report
**Rationale:** Architecture tier 6 — the outermost shell. By this point all business logic exists; the CLI wires it together. Progress reporting and the structured final report are built here rather than earlier because they enhance but do not block the core loop. Terminal UX polish (indicatif spinners, per-phase status) lands here.
**Delivers:** `cli/` (clap subcommands: run, init, report), progress reporting via indicatif + tracing-indicatif, ReportWriter (JSON + Markdown), structured final report to disk
**Addresses:** Visible progress reporting (table stakes), structured final report (P1), terminal UX
**Avoids:** UX pitfalls — silent progress, opaque failure messages, raw LLM output dumping

### Phase 6: Token Budget, Cost Reporting, and Dry-Run Mode
**Rationale:** v1.x features with clear dependency on the stable core loop from phases 1-5. Token tracking is retrofitted into the actor layer (phase 2) once the interface is stable. Dry-run is a read-only projection of the Phase 3 decomposition — low risk to add after validation.
**Delivers:** Per-run token budget tracking (accumulated across full run, not per-call), hard budget cap with halt-and-report, dry-run mode with cost estimation, per-phase token and cost summary in report
**Addresses:** Token usage and cost reporting (P2 differentiator), dry-run / planning mode (P2)
**Avoids:** Runaway token costs pitfall — budget enforcement built into API abstraction

### Phase 7: Parallel Execution and Git Worktree Isolation
**Rationale:** v1.x features that require proven isolation from earlier phases. Parallel execution without proven isolation is the module isolation boundary violation pitfall at scale. These are additive enhancements — the system works correctly in sequential mode; these phases add throughput.
**Delivers:** Parallel phase execution for independent modules via tokio JoinSet (already architecturally present, this phase enables it in the runtime path), git worktree isolation upgrade (replaces file-list isolation for parallel agent working directories)
**Addresses:** Parallel phase execution (P2), git worktree isolation (P2)
**Avoids:** Module isolation boundary violations under parallel execution

### Phase Ordering Rationale

- **Foundation before agents:** The typed schema contracts (Phase 1) are the most important single investment. Building them first means every subsequent phase compiles against stable, validated types — not retrofitted after problems emerge.
- **Infrastructure before orchestration:** Agent clients and git layer (Phase 2) are mechanically simple but must be correct. The coordinator (Phase 4) delegates entirely to these — their correctness is assumed, not checked.
- **Planner before coordinator:** The coordinator executes plans it receives; it does not generate them. Testing the planner in isolation (Phase 3) allows structural validation to catch decomposition errors before they propagate into a running pipeline.
- **Sequential before parallel:** Phases 1-5 deliver a working sequential pipeline. Parallel execution (Phase 7) is additive — the system is useful and shippable before it arrives.
- **Token budget as v1.x not v1:** Token tracking is important but not blocking for initial validation. A controlled first run on a known project can be manually bounded. The pitfalls research flags this as a "never acceptable" long-term shortcut but acceptable for MVP if run scope is small and known.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (PhasePlanner):** The LLM-assisted DAG decomposition is the least-documented pattern in the research — most framework comparisons describe DAG execution, not DAG generation via LLM. The prompt engineering for reliable, structurally valid decomposition output needs more investigation. Needs per-phase research.
- **Phase 4 (ReviewEngine routing):** Cross-vendor review pairing strategy (which model reviews which) and how to distinguish blocking vs advisory review failures reliably requires specific model capability research. Needs per-phase research.
- **Phase 7 (Git worktrees):** Git worktree lifecycle management in a Rust async context with git2 has limited documented examples. The ccswarm project (referenced in features research) is the closest precedent but is Claude Code-specific. Needs per-phase research.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** serde schemas, anyhow/thiserror, dotenvy — all well-documented Rust ecosystem patterns with extensive examples
- **Phase 2 (Agent clients):** The Tokio actor pattern is extensively documented (ryhl.io/blog/actors-with-tokio); genai API is straightforward; git2 spawn_blocking pattern is documented
- **Phase 5 (CLI + reporting):** clap 4 derive macros and indicatif + tracing-indicatif integration are well-documented with official examples
- **Phase 6 (Token budget):** Accumulating token counts from API responses and enforcing a cap is straightforward instrumentation with no novel patterns required

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core crates (tokio, clap, serde, git2, anyhow) are industry-standard with stable APIs. genai 0.5 is the one uncertainty — actively maintained but newer. All versions verified on crates.io as of 2026-03-12. |
| Features | MEDIUM-HIGH | 15+ sources including official framework docs, peer-reviewed research (arXiv), and 2025/2026 developer surveys. Ecosystem is rapidly evolving; competitive feature gaps may shift. Table stakes features are stable; differentiator prioritization is based on current landscape. |
| Architecture | HIGH | Component structure follows established Rust async patterns with strong source backing (Azure Architecture Center, ryhl.io actors, official Tokio docs). The 6-tier build order is inferred from component dependencies — logical but not externally validated. |
| Pitfalls | HIGH | Multiple authoritative sources including official Anthropic API error documentation, arXiv quantitative research (agent drift study 2601.04170), and production case studies. The 42% failure rate from untyped handoffs and the 73-interaction drift threshold are from peer-reviewed sources. |

**Overall confidence:** HIGH

### Gaps to Address

- **genai 0.5 API surface for structured output (JSON mode):** The research confirms genai supports structured output, but the exact API for enforcing JSON schema per-provider (especially for Gemini which uses `generativelanguage.googleapis.com` directly) needs validation during Phase 2 implementation. If genai's JSON mode support is incomplete for any provider, direct reqwest calls are the fallback.
- **LLM-assisted DAG decomposition prompt design:** No existing tool publicly documents their phase decomposition prompts. The quality floor for Athena's core value proposition is unknown until tested on real projects. Plan for iteration on decomposition prompts during Phase 3 — treat prompt changes as versioned code artifacts.
- **Cross-vendor review pairing effectiveness:** Research establishes that cross-model review is better than self-review, but does not quantify whether Claude-reviews-Codex vs Gemini-reviews-Codex produces meaningfully different outcomes. The initial routing table (Claude output → Gemini review, Gemini output → Codex review, Codex output → Claude review) is a reasonable default but should be monitored.
- **Anthropic Opus 4.6 prefill restriction:** The pitfalls research notes that Opus 4.6 does not support prefilling (use `output_config.format` instead). This is a concrete integration gotcha to validate during Phase 2 agent client implementation.

## Sources

### Primary (HIGH confidence)
- Anthropic API error documentation (platform.claude.com/docs) — error codes 429/500/529, Opus 4.6 prefill restriction
- tokio.rs official docs (v1.50.0) — task spawning, JoinSet, channels, spawn_blocking
- docs.rs/git2 — git2 Repository API, libgit2-sys bundled build
- docs.rs/genai (v0.5.3) — provider adapters, ClientBuilder, JSON mode support
- arXiv 2601.04170 (Agent Drift: Quantifying Behavioral Degradation) — 73-interaction threshold, 42% success rate reduction
- RUSTSEC-2021-0141 — dotenv security advisory (justifies dotenvy)
- Azure Architecture Center AI Agent Design Patterns (2026) — supervisor/worker, actor patterns

### Secondary (MEDIUM confidence)
- GitHub Blog: "Multi-agent workflows often fail" — 42% failure rate from interface mismatch
- ryhl.io/blog/actors-with-tokio — Tokio actor pattern with mpsc channels
- DataCamp / o-mega.ai / OpenAgents.org — framework comparison (CrewAI, MetaGPT, LangGraph, AutoGen)
- Galileo AI blog (multiple articles) — production failure modes, coordination failures
- crates.io version data — clap 4.5.60, serde_json 1.0.149, dotenvy 0.15.7 verified March 2026

### Tertiary (MEDIUM-LOW confidence)
- corrode.dev/blog/async — Tokio vs alternatives comparison (community analysis, single source)
- ccswarm GitHub (nwiizo/ccswarm) — git worktree multi-agent isolation pattern (implementation reference, not documentation)
- dasroot.net/posts/2026/02/rust-libraries-llm-orchestration-2026 — Rust LLM ecosystem survey (single community source)

---
*Research completed: 2026-03-12*
*Ready for roadmap: yes*

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

# Stack Research

**Domain:** Multi-agent AI orchestration CLI (Rust)
**Researched:** 2026-03-12
**Confidence:** HIGH (core stack) / MEDIUM (AI-specific crates)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tokio | 1.x (1.50 current) | Async runtime for concurrent agent execution | The de-facto standard async runtime. Work-stealing scheduler handles I/O-bound tasks (HTTP calls to 3 AI APIs) efficiently. Entire ecosystem (reqwest, tracing, git2 async) builds on top of it. No serious competitor for multi-task orchestration. |
| clap | 4.5 (4.5.60 current) | CLI argument parsing with subcommands, flags, env var integration | v4 with derive macros is the industry standard. Supports Unix conventions, auto-generated help, shell completions, and type-safe argument structs. The `derive` feature eliminates boilerplate. Only alternative worth considering (lexopt) lacks help generation needed for a user-facing tool. |
| genai | 0.5.x (0.5.3 current) | Unified multi-provider AI client (Claude, Gemini, OpenAI) | Single crate with native adapters for all three required providers. Avoids maintaining three separate SDK integrations. Supports streaming, reasoning/thinking controls, custom auth/endpoint overrides. Uses reqwest 0.13 internally. Actively maintained (v0.5.0 released Jan 2026). |
| serde + serde_json | 1.x (serde_json 1.0.149) | Serialization for AI request/response structs, config, phase plans | Ubiquitous Rust serialization framework. Required for parsing AI API JSON responses, serializing phase plans to disk, and config files. Every alternative is marginal in comparison to ecosystem depth. |
| git2 | 0.20.x (0.20.2 current) | Programmatic git operations: stage, commit, branch per phase | libgit2 bindings — the only mature option for in-process git ops without shelling out. Supports all operations Athena needs: init repo, stage files (index manipulation), create commits, manage branches. Bundled libgit2 means no system dependency. |
| tracing + tracing-subscriber | 0.1.x | Structured async-aware logging and diagnostics | Designed for async Tokio applications. Spans capture task context across await points, so you can trace which agent produced which output. Integrates with indicatif for progress bars without terminal fighting. Superior to the `log` crate for multi-task orchestration. |
| anyhow | 1.x | Application-level error aggregation | CLI application errors should display cleanly, not match on enum variants. `anyhow` provides ergonomic `?` propagation with `.context()` for descriptive error chains. Correct choice at the binary level (vs. `thiserror` which belongs in libraries). |
| thiserror | 2.x | Typed errors for internal library modules | Phase orchestration, agent communication, git operations each need domain-specific error types that calling code can match on. Use `thiserror` in internal modules; `anyhow` at the top-level binary. Both are complementary. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest | 0.13.x | HTTP client (used via genai; may need direct use for custom endpoints) | Use directly only if genai's adapter doesn't cover a provider-specific feature (e.g., custom streaming endpoints, function calling with non-standard schemas). reqwest 0.13 uses rustls by default — no OpenSSL dependency. |
| dotenvy | 0.15.x | Load API keys from `.env` file for development | Always include for dev ergonomics. Athena requires `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `OPENAI_API_KEY` — dotenvy loads these from `.env` before clap processes them. Pass keys through clap's `env()` attribute for clean integration. |
| indicatif | 0.17.x | Terminal progress bars and spinners during agent execution | Use for per-agent progress display during parallel phase execution. Integrate via `tracing-indicatif` to avoid tracing logs and progress bars fighting for terminal. Multi-bar support shows all three agents simultaneously. |
| console | 0.15.x | Terminal color and styling (dependency of indicatif, usable standalone) | Use for colored status output: phase summaries, review pass/fail badges, agent assignment tables. Already pulled in via indicatif — no extra dep cost. |
| tokio-util | 0.7.x | Stream utilities, codec framing for SSE streaming responses | Required for consuming Server-Sent Events (SSE) streams from AI APIs when using streaming completions. `genai` handles this internally, but useful if implementing custom streaming. |
| futures | 0.3.x | `join_all`, `select`, stream combinators for parallel agent tasks | Use `futures::future::join_all` to fan out concurrent API calls to all three agents. `FuturesUnordered` for collecting results as they complete rather than waiting for all. |
| uuid | 1.x | Generate unique identifiers for phases, tasks, agent sessions | Each phase/task needs a stable ID for tracking, logging correlation, and git commit metadata. |
| chrono | 0.4.x | Timestamps for phase execution tracking and report generation | Git commits and structured reports need ISO 8601 timestamps. `chrono` with `serde` feature for serialization. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-watch | Auto-rebuild on file changes during development | `cargo install cargo-watch`, run as `cargo watch -x run`. Essential for tight iteration loop. |
| cargo-nextest | Faster test runner with better output for parallel tests | `cargo install cargo-nextest`. Especially useful when testing async agent orchestration with many unit tests. |
| cargo-dist | Cross-platform binary distribution (Linux, macOS, Windows) | Athena targets single-binary distribution. `cargo-dist` generates GitHub Actions workflows + installers. Use when ready to ship. |
| rust-analyzer | LSP server for IDE support | Standard. Enables inline type hints critical for async code with complex Future types. |

---

## Installation

```toml
# Cargo.toml

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive", "env"] }

# AI providers (all three: Claude, Gemini, OpenAI/Codex)
genai = "0.5"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Git integration
git2 = "0.20"

# Logging / tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

# Error handling
anyhow = "1"
thiserror = "2"

# Environment / config
dotenvy = "0.15"

# Terminal UI
indicatif = "0.17"
console = "0.15"

# Async combinators
futures = "0.3"

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

```bash
# Create new binary crate
cargo new athena --bin
cd athena

# Add all dependencies at once
cargo add tokio --features full
cargo add clap --features derive,env
cargo add genai
cargo add serde --features derive
cargo add serde_json
cargo add git2
cargo add tracing
cargo add tracing-subscriber --features env-filter,fmt
cargo add anyhow
cargo add thiserror
cargo add dotenvy
cargo add indicatif
cargo add console
cargo add futures
cargo add uuid --features v4
cargo add chrono --features serde
```

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Async runtime | tokio | async-std | tokio has significantly larger ecosystem; genai, reqwest, tracing all target tokio. async-std lacks critical ecosystem crates. |
| Async runtime | tokio | smol | smol is minimal/embedded-focused. Not suitable for orchestrating I/O-heavy multi-agent workflows with SSE streaming. |
| CLI parsing | clap 4 | argh | argh follows Fuchsia OS conventions (not Unix). Missing `--help` subcommand delegation, shell completions, env var integration. Wrong for user-facing tool. |
| CLI parsing | clap 4 | lexopt | Zero-dependency but zero-feature. No help generation, no derive macros, no env vars. Would require manual implementation of everything clap provides. |
| AI client | genai | Per-provider SDKs (anthropic-rs, async-openai) | Three separate SDKs with different APIs, auth patterns, and streaming models. genai unifies all three with one ergonomic interface. Use individual SDKs only if provider-specific features outpace genai. |
| AI client | genai | Raw reqwest | Requires hand-rolling JSON schemas for each provider's API, manually handling SSE streaming, implementing rate limiting. High implementation cost for no gain. |
| Git integration | git2 | Shelling out to `git` CLI | Process spawn has overhead, requires git installed on PATH, harder to test, error handling is string-parsing. git2 is in-process, no system dependency, typed errors. |
| Git integration | git2 | gitoxide (gix) | gitoxide is newer and pure-Rust but its API is less stable and less documented. git2 (libgit2 bindings) is battle-tested across thousands of projects. |
| Logging | tracing | log + env_logger | `log` crate has no concept of spans — cannot track which async task (agent) produced a log entry. In concurrent agent execution this makes logs unreadable. `tracing` was designed for async. |
| Error handling | anyhow + thiserror | eyre | eyre is an anyhow fork with richer error reporting. Reasonable alternative. anyhow is more widely used with better ecosystem integration. |
| Config | dotenvy + clap env() | figment | figment is powerful for layered config but overengineered for Athena v1. Three API keys loaded from env vars is the entire config surface. Figment adds complexity without benefit at this scope. |
| Serialization | serde | nanoserde | nanoserde trades ecosystem compatibility for compile speed. Incompatible with genai and reqwest's serde expectations. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `dotenv` crate (original) | Unmaintained since June 2020, known security advisory RUSTSEC-2021-0141 | `dotenvy` — the actively maintained fork |
| `structopt` | Merged into clap v3+, no longer developed separately | `clap 4` with `derive` feature |
| `openssl` feature in reqwest | Requires system OpenSSL installation — breaks cross-compilation and increases binary size | Leave reqwest 0.13 at its default (rustls) — no system dep, ships in binary |
| `async-openai` as sole provider | OpenAI-only; Athena requires Claude + Gemini too. You'll still need separate Anthropic + Gemini integration | `genai` for unified multi-provider access |
| `std::thread::spawn` for agent parallelism | OS threads are wasteful for I/O-bound AI API calls. 3 agents each making HTTP calls blocks threads for seconds | `tokio::spawn` — tasks are lightweight (64 bytes vs ~MB stack), thousands per runtime |
| `panic!` / `unwrap()` in orchestration paths | Silent crashes during autonomous multi-hour execution lose all state and progress | `anyhow::Result` with `.context()` throughout; reserve `expect()` only for provably-safe invariants |
| `log` + `env_logger` | Cannot track which agent produced a log line in concurrent execution — logs from 3 parallel tasks are interleaved without context | `tracing` with task-specific spans; each agent gets a span with `agent = "claude"` etc. |

---

## Stack Patterns by Variant

**For concurrent agent execution (3 agents in parallel):**
- Use `tokio::spawn` per agent task, collect with `futures::future::join_all`
- Each agent task gets its own tracing span: `tracing::info_span!("agent", name = "claude")`
- Use `tokio::sync::mpsc` channels for agent-to-conductor status updates (actor pattern)
- Rate limiting: `tokio::sync::Semaphore` to cap concurrent API calls if needed

**For streaming AI responses:**
- `genai` handles SSE streaming natively
- Stream token output to terminal via `indicatif` progress bar messages as tokens arrive
- Collect full response for cross-review before advancing phase

**For phase-gated execution (blocking on review gate):**
- Use `tokio::sync::watch` or `tokio::sync::Notify` to signal phase completion to waiting tasks
- Gate advancement with a review result enum: `ReviewResult::Pass | ReviewResult::Fail(issues)`
- Loop on `Fail` with retry budget (e.g., max 3 review cycles per phase)

**For git operations (one commit per phase):**
- Run git2 operations synchronously within a `tokio::task::spawn_blocking` block
- Never call blocking git2 code directly in an async context — it blocks the executor thread
- Stage all agent-written files, commit with structured message: `"[Athena] Phase N: <description> (agents: claude, gemini)"`

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| genai 0.5 | reqwest 0.13, tokio 1.x | genai ships reqwest 0.13 internally; if you add reqwest directly, pin to 0.13 to avoid duplicate versions |
| tracing 0.1 | tokio 1.x, tracing-subscriber 0.3 | tracing-subscriber 0.3 is required for the env-filter feature; 0.2 is EOL |
| clap 4.5 | Rust 1.74+ | clap 4 requires MSRV 1.74; verify CI uses stable Rust not older |
| git2 0.20 | libgit2 1.9.0 (bundled) | No system libgit2 required — bundled via libgit2-sys. Adds ~2MB to compile time but zero runtime dep. |
| serde 1.x | serde_json 1.x, chrono 0.4 | Keep serde features consistent: enable `derive` at the workspace level if using a workspace |

---

## Sources

- genai GitHub (jeremychone/rust-genai) — v0.5.3 confirmed, provider support, reqwest 0.13 upgrade verified
- docs.rs/genai — Adapter pattern, ClientBuilder API, provider routing
- tokio.rs — Version 1.50.0 current (March 2026), task spawning, channels, spawn_blocking docs
- crates.io — clap 4.5.60 (Feb 2026), git2 0.20.2, serde_json 1.0.149, dotenvy 0.15.7
- seanmonstar.com/blog/reqwest-v013-rustls-default — reqwest 0.13 TLS change details (MEDIUM confidence — single official source)
- corrode.dev/blog/async — Tokio vs alternatives comparison (MEDIUM confidence — community analysis)
- ryhl.io/blog/actors-with-tokio — Actor pattern with tokio::sync::mpsc (MEDIUM confidence — widely cited community resource)
- RUSTSEC-2021-0141 — dotenv security advisory justifying dotenvy (HIGH confidence — official advisory)

---

*Stack research for: Athena — Multi-agent AI orchestration CLI in Rust*
*Researched: 2026-03-12*

# Feature Research

**Domain:** Multi-agent AI orchestration CLI (code generation / software development automation)
**Researched:** 2026-03-12
**Confidence:** MEDIUM-HIGH (ecosystem rapidly evolving; core feature categories stable, specific tooling details may shift)

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete or untrustworthy.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Natural language project input | Entry point for any AI dev tool — users expect to describe what they want, not write config files | LOW | Parse free-form description into structured intent; the hard part is structured output from the LLM, not the CLI input itself |
| Structured phase/task decomposition | Every competing tool (MetaGPT, CrewAI, LangGraph) breaks work into discrete steps — users know this pattern | HIGH | This is Athena's core value prop; getting dependency ordering right is the hard problem |
| Named agent roles with explicit assignments | CrewAI popularized role-based agents (Product Manager, Architect, Engineer, QA); users now expect this | MEDIUM | Maps naturally to Claude/Gemini/Codex specialization model |
| Per-phase git commits | Non-negotiable for any code generation tool — users need to track what changed and when | LOW | Straightforward `git commit` after each phase; commit message should include phase name and agent |
| API key configuration via env vars and config file | Every CLI tool that touches external APIs supports both; missing either is a friction point | LOW | `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `OPENAI_API_KEY`; support `.env` file and `~/.athena/config.toml` |
| Visible progress reporting | Users firing off a long-running multi-agent task need to know it's working, not hung | MEDIUM | Real-time phase progress, current agent, current task; terminal-friendly output (not just final report) |
| Structured final report | MetaGPT, CrewAI all produce summary output; users expect an artifact they can inspect | LOW | Phase table, agent assignments, review results, outcome per phase |
| Error reporting with actionable context | When an agent fails or a review blocks, users need to know what failed and why | MEDIUM | Distinguish API errors, review failures, and malformed outputs; surface the specific blocking reason |
| Typed/validated inter-agent communication | GitHub Engineering Blog (2025): unstructured data exchange is the #1 cause of multi-agent workflow failure | HIGH | Agents must exchange structured JSON schemas, not raw prose; validated at every boundary |
| Phase-level review/quality gate | The cross-agent review pattern (Generator + Critic) is now a documented standard pattern (Google ADK, LangGraph, MetaGPT) | HIGH | A different agent reviews each phase output before progression; blocking gate, not advisory |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valued when present.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Intelligent dependency analysis | Most tools execute sequentially or manually-specified DAGs; auto-detecting which modules depend on which is novel | HIGH | Static analysis of the project spec + LLM-assisted dependency inference; prevents incorrect parallelization |
| Skill-based agent routing | Athena assigns tasks to Claude vs Gemini vs Codex based on task type (architecture, research, boilerplate) — no competitor does this with heterogeneous models | HIGH | Requires a routing layer with a skill taxonomy and model capability map that must be maintained as models evolve |
| True module isolation per agent | Git worktrees per agent ensure zero file-level merge conflicts during parallel execution — this is emerging (ccswarm, Claude Code) but not standard | MEDIUM | Each agent owns a strict file list; orchestrator enforces boundaries; worktrees or separate working dirs |
| Cross-model peer review | Agent A's output reviewed by Agent B (different vendor, different model) catches blind spots single-model self-review misses | MEDIUM | Already planned in PROJECT.md; the differentiator is cross-vendor not just cross-agent |
| Automatic retry loop on review failure | Phase blocked? Athena automatically re-prompts the original agent with the reviewer's feedback until resolved or max-retries hit | MEDIUM | Most tools require human intervention to unblock; fully autonomous retry is a DX win |
| Dry-run / planning mode | Show the proposed phase plan, agent assignments, and dependency graph without executing — lets users inspect and validate before incurring API cost | LOW | Overstory (open source orchestrator) has this; users cited cost opacity as a top pain point |
| Token usage and cost reporting | Per-phase token counts and estimated cost per agent; surfaces what each phase actually cost | MEDIUM | Users cited pricing opacity as a major frustration (Stack Overflow 2025 survey); BYO-key tools should be maximally transparent |
| Parallelization of independent phases | 2025 research shows ~1.3x speedup from parallel agent execution; users with background execution aspirations value this | HIGH | Requires accurate dependency analysis first; unsafe parallelization causes integration failures |
| Spec-driven input (file-based) | Kiro, Tessl, GitHub Spec Kit trend: `requirements.md`, `design.md`, `tasks.md` as orchestration input; resonates with structured developers | LOW | Accept both free-form text and structured spec files as project input |
| Resume interrupted execution | LangGraph checkpointing popularized this; long runs can be interrupted by API timeouts, rate limits, or user Ctrl-C | HIGH | Requires persistent phase state between runs; SQLite or local checkpoint file |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems for Athena's scope and v1 goals.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Web dashboard / visual workflow UI | Developers want to "see" agent activity; CrewAI's Agent Operations Platform, LangGraph Studio both exist | Violates v1 CLI-only scope; massive scope expansion; adds auth, server, frontend maintenance burden; 83% of multi-agent users are senior+ engineers already comfortable with terminal output | Rich terminal output with structured tables; dry-run mode prints the DAG as ASCII; final report is human-readable |
| Built-in API key management / key rotation | Users ask for managed secrets to simplify setup | Requires a secrets service, encryption at rest, and a trust model; fundamentally a SaaS feature; creates liability for key security | Env vars + config file; document `.env` gitignore best practices; let users use their own secrets manager |
| Real-time agent conversation visibility (full chat transcript) | AutoGen's transparent turn-by-turn logs are appealing for debugging | Generates enormous terminal noise for non-power-users; token-level streaming from three concurrent agents is unreadable; bloats log files | Verbose flag `--verbose` shows full transcripts on demand; default output is phase-level summaries only |
| Human-in-the-loop approval at every step | CrewAI and AutoGen both support HITL; users from those tools expect it | Destroys the "autonomous" value prop; if every step requires approval, users are just doing the work themselves | Reserve HITL for the planning/dry-run phase only; execution is autonomous by design; users review the plan, not every agent action |
| Plugin/extension system for custom agents | Power users always ask for extensibility early | Premature abstraction before the core protocol is stable; plugin APIs become permanent commitments; maintenance burden compounds fast | Hardcode three agents in v1; design the agent trait/interface cleanly in Rust so extension is possible later without a public API commitment now |
| Automatic PR creation / GitHub integration | Feels like the natural completion of a code-generation workflow | Adds OAuth, GitHub API dependency, branch management logic; out of scope per PROJECT.md; direct commits to local repo is the stated design | Commit directly to local repo per phase; user pushes and opens PR themselves with full control |
| Self-hosted LLM support (Ollama, local models) | Privacy-conscious users want no data leaving their machine | Local models are significantly weaker for complex decomposition and code generation; would require model capability benchmarking to route tasks correctly; complicates the agent capability map | Document which model tiers are supported; note local model support as a v2+ consideration once routing is stable |
| Multi-user / team collaboration | Team workflows are a natural next step after single-user validation | Real-time collaboration requires a server, shared state store, conflict resolution between concurrent human users, and auth — an entirely different product | Stay single-user for v1; validate the core orchestration; team features are a post-PMF product decision |

---

## Feature Dependencies

```
[Natural Language Input]
    └──requires──> [Structured Phase Decomposition]
                       └──requires──> [Dependency Analysis]
                                          └──requires──> [Agent Routing / Skill Map]
                                                             └──requires──> [Module Isolation]
                                                                                └──requires──> [Parallel Execution]

[Structured Phase Decomposition]
    └──requires──> [Typed Inter-Agent Communication]
                       └──requires──> [Cross-Agent Review Gate]
                                          └──requires──> [Automatic Retry Loop]

[API Key Configuration]
    └──requires──> [All Agent API Calls]

[Per-Phase Git Commits]
    └──requires──> [Module Isolation] (know what each agent wrote before committing)

[Dry-Run / Planning Mode]
    └──enhances──> [Structured Phase Decomposition] (same decomposition, no execution)

[Token Usage Reporting]
    └──enhances──> [All Agent API Calls] (track per-call)

[Resume Interrupted Execution]
    └──requires──> [Structured Phase Decomposition] (know which phases completed)
    └──requires──> [Per-Phase Git Commits] (durable checkpoint marker)

[Visible Progress Reporting]
    └──enhances──> [All execution phases]

[Verbose Mode / Full Transcript]
    └──enhances──> [Cross-Agent Review Gate] (show reviewer's exact feedback)
    └──conflicts──> [Default Clean Output] (must be opt-in flag, not default)
```

### Dependency Notes

- **Dependency analysis requires structured decomposition first:** You cannot route tasks to agents or isolate modules until the phase structure exists. This is the foundational blocker — everything downstream depends on getting decomposition right.
- **Module isolation is a prerequisite for parallel execution:** Running agents in parallel on the same working directory causes real-time merge conflicts. Isolation (via file ownership list or git worktrees) must be established before parallelization is enabled.
- **Cross-agent review requires typed communication:** If inter-agent messages are unstructured prose, the reviewer cannot reliably parse what it is reviewing. Typed schemas must be enforced before review gates are meaningful.
- **Resume requires phase state persistence:** If Athena crashes mid-execution, it needs a checkpoint file or database to know which phases completed and what was committed. This is a non-trivial addition and should be deferred past v1.
- **Dry-run enhances but does not require anything:** It is a read-only projection of the decomposition output. It can be added at any phase of development with low risk.
- **Token reporting conflicts with minimal-output defaults:** Streaming verbose token data conflicts with the default clean progress output. Must be a flag, not default behavior.

---

## MVP Definition

### Launch With (v1)

Minimum viable to validate the core concept: intelligent orchestration produces better code than a single agent.

- [ ] Natural language and spec file input — the only way users describe their project
- [ ] Structured phase decomposition with dependency ordering — Athena's core value; without this, everything else is just sequential prompting
- [ ] Agent routing (Claude → architecture/logic, Gemini → research/APIs, Codex → boilerplate generation) — the heterogeneous model approach is the differentiator
- [ ] Module isolation via file ownership list — prevents parallel write conflicts; full git worktree isolation can come later
- [ ] Sequential phase execution with cross-agent review gate — the quality gate is load-bearing; ship it from day one even if parallelism comes later
- [ ] Automatic retry loop on review failure (max 3 attempts) — makes the system autonomous; without this, every review failure requires human intervention
- [ ] Per-phase git commits — basic traceability; non-negotiable
- [ ] Terminal progress reporting (phase name, current agent, status) — users will abandon a tool that appears hung
- [ ] Structured final report (phase table, agent assignments, review outcomes) — the "receipt" of what was built
- [ ] API key config via env vars and config file — standard BYO-key UX
- [ ] Typed inter-agent message schemas — foundational reliability; skip this and cross-agent review becomes unreliable
- [ ] Error reporting with phase-level context — distinguishes API errors from review failures from schema violations

### Add After Validation (v1.x)

Add once v1 proves the core loop works and users are retained.

- [ ] Dry-run / planning mode — add once decomposition is stable and trustworthy; users will want to inspect plans before paying API costs
- [ ] Token usage and cost reporting per phase — add once users ask "why did that cost so much?"; high-value, low-complexity addition
- [ ] True parallel phase execution for independent modules — add once isolation is proven reliable; requires dependency graph correctness first
- [ ] Git worktree isolation (vs. file-list isolation) — upgrade isolation mechanism once parallel execution is added; cleaner boundaries
- [ ] Verbose mode / full agent transcript flag — add when users request debugging capability; `--verbose` flag on existing output

### Future Consideration (v2+)

Defer until post-PMF — these require either significant new architecture or a clear user signal.

- [ ] Resume interrupted execution — requires persistent checkpoint state; meaningful complexity addition; defer until users report losing work
- [ ] Spec-driven input format (requirements.md / design.md structured files) — add once the free-form NL input is well-understood; standardize after usage patterns emerge
- [ ] Plugin system for custom agents — defer until the three-agent model is proven; premature extensibility is a maintenance trap
- [ ] Local/self-hosted model support — defer until local models reach capability parity for complex decomposition tasks

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Structured phase decomposition | HIGH | HIGH | P1 |
| Cross-agent review gate | HIGH | HIGH | P1 |
| Module isolation (file ownership) | HIGH | MEDIUM | P1 |
| Agent routing (Claude/Gemini/Codex) | HIGH | MEDIUM | P1 |
| Natural language input parsing | HIGH | MEDIUM | P1 |
| Per-phase git commits | HIGH | LOW | P1 |
| API key config (env + file) | HIGH | LOW | P1 |
| Terminal progress reporting | HIGH | LOW | P1 |
| Typed inter-agent schemas | HIGH | MEDIUM | P1 |
| Automatic retry on review failure | HIGH | MEDIUM | P1 |
| Structured final report | MEDIUM | LOW | P1 |
| Error reporting with context | MEDIUM | LOW | P1 |
| Dry-run / planning mode | HIGH | LOW | P2 |
| Token usage and cost reporting | MEDIUM | MEDIUM | P2 |
| Parallel phase execution | MEDIUM | HIGH | P2 |
| Git worktree isolation | MEDIUM | MEDIUM | P2 |
| Verbose mode / full transcript | LOW | LOW | P2 |
| Resume interrupted execution | MEDIUM | HIGH | P3 |
| Spec-driven input format | MEDIUM | LOW | P3 |
| Plugin system for agents | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Competitor Feature Analysis

| Feature | CrewAI | MetaGPT | LangGraph | AutoGen | Athena (planned) |
|---------|--------|---------|-----------|---------|-----------------|
| Role-based agent assignment | Yes — core pattern | Yes — simulates software company roles (PM, Architect, Engineer, QA) | Implicit via node design | Conversational personas, dynamic | Yes — fixed roles mapped to Claude/Gemini/Codex strengths |
| Task decomposition / planning | Task-oriented, manual or hierarchical manager | SOPs encode a fixed decomposition pattern | Graph nodes, manually specified by developer | Emerges through multi-turn conversation | Yes — LLM-driven decomposition from NL input |
| Dependency analysis | Manual (task dependencies declared by user) | Not automatic — fixed SOP pattern | Manual (graph edges declared by developer) | None — sequential conversation | Yes — automatic, inferred from project spec |
| Parallel execution | Yes — crew coordination | Yes — parallel subtask decomposition | Yes — DAG-based | Limited — sequential turns | v1: sequential; v1.x: parallel independent phases |
| Cross-agent review gate | Yes — hierarchical manager validates | Yes — peer review in SOP | Configurable — decision nodes | Yes — agents can critique each other in conversation | Yes — mandatory blocking gate, cross-vendor |
| Module isolation | No — agents share codebase | No explicit isolation | No — shared state | No | Yes — strict file ownership list per agent |
| Heterogeneous model routing | No — single LLM provider typically | No | No | Partial — different backends per agent | Yes — skill-based routing across Claude, Gemini, Codex |
| Git integration | No built-in | No built-in | No built-in | No built-in | Yes — per-phase commits, local repo |
| Typed inter-agent schemas | Partial — structured task outputs | Partial — structured documents | Partial — state schema | No — conversation prose | Yes — validated JSON schemas at every boundary |
| Dry-run / planning mode | No | No | No | No | v1.x |
| Cost / token reporting | No (enterprise dashboard only) | No | No (LangSmith external) | No | v1.x |
| Resume interrupted execution | No | No | Yes — checkpointing | No | v2+ |
| Single binary distribution | No — Python package | No — Python package | No — Python package | No — Python package | Yes — Rust compiled binary |
| CLI-first / no server required | No — enterprise requires platform | No | No — LangGraph Studio is web | No | Yes |

---

## Sources

- [CrewAI vs LangGraph vs AutoGen: Choosing the Right Multi-Agent AI Framework — DataCamp](https://www.datacamp.com/tutorial/crewai-vs-langgraph-vs-autogen)
- [LangGraph vs CrewAI vs AutoGen: Top 10 AI Agent Frameworks — o-mega.ai](https://o-mega.ai/articles/langgraph-vs-crewai-vs-autogen-top-10-agent-frameworks-2026)
- [CrewAI vs LangGraph vs AutoGen vs OpenAgents (2026) — OpenAgents Blog](https://openagents.org/blog/posts/2026-02-23-open-source-ai-agent-frameworks-compared)
- [AutoGen vs LangGraph vs CrewAI: Which Agent Framework Actually Holds Up in 2026? — DEV Community](https://dev.to/synsun/autogen-vs-langgraph-vs-crewai-which-agent-framework-actually-holds-up-in-2026-3fl8)
- [Multi-agent workflows often fail. Here's how to engineer ones that don't. — GitHub Blog](https://github.blog/ai-and-ml/generative-ai/multi-agent-workflows-often-fail-heres-how-to-engineer-ones-that-dont/)
- [MetaGPT: Meta Programming for a Multi-Agent Collaborative Framework — arXiv](https://arxiv.org/abs/2308.00352)
- [What is MetaGPT? — IBM Think](https://www.ibm.com/think/topics/metagpt)
- [AI Agent Orchestration Patterns — Azure Architecture Center, Microsoft Learn](https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/ai-agent-design-patterns)
- [Anti-Patterns in Multi-Agent Gen AI Solutions — Medium / Arman Kamran](https://medium.com/@armankamran/anti-patterns-in-multi-agent-gen-ai-solutions-enterprise-pitfalls-and-best-practices-ea39118f3b70)
- [Git Worktrees: The Secret Weapon for Running Multiple AI Coding Agents in Parallel — Medium](https://medium.com/@mabd.dev/git-worktrees-the-secret-weapon-for-running-multiple-ai-coding-agents-in-parallel-e9046451eb96)
- [ccswarm: Multi-agent orchestration using Claude Code with Git worktree isolation — GitHub](https://github.com/nwiizo/ccswarm)
- [10 Things Developers Want from their Agentic IDEs in 2025 — RedMonk](https://redmonk.com/kholterhoff/2025/12/22/10-things-developers-want-from-their-agentic-ides-in-2025/)
- [Developer's guide to multi-agent patterns in ADK — Google Developers Blog](https://developers.googleblog.com/developers-guide-to-multi-agent-patterns-in-adk/)
- [The Rise of Agentic Testing: Multi-Agent Systems for Robust Software Quality Assurance — arXiv](https://arxiv.org/abs/2601.02454)
- [5 Key Trends Shaping Agentic Development in 2026 — The New Stack](https://thenewstack.io/5-key-trends-shaping-agentic-development-in-2026/)
- [Best AI Coding Agents for 2026: Real-World Developer Reviews — Faros AI](https://www.faros.ai/blog/best-ai-coding-agents-2026)

---
*Feature research for: Multi-agent AI orchestration CLI (Athena)*
*Researched: 2026-03-12*

# Pitfalls Research

**Domain:** Multi-agent AI orchestration CLI (coordinating Claude, Gemini, Codex for software development)
**Researched:** 2026-03-12
**Confidence:** HIGH (multiple authoritative sources, official API docs verified, peer-reviewed research referenced)

---

## Critical Pitfalls

### Pitfall 1: Untyped Cross-Agent Handoffs (The "Messy JSON" Problem)

**What goes wrong:**
Agents exchange natural language or loosely-typed JSON where field names shift, data types mismatch, and required fields go missing. Athena passes output from Claude's architecture phase to Gemini's documentation phase, but Gemini receives ambiguous text it re-interprets differently than intended. Downstream agents then act on corrupted semantics. This is the #1 source of cascading failures — 42% of multi-agent failures stem from specification issues, most of which are interface mismatch problems.

**Why it happens:**
Developers treat agents like chat sessions rather than distributed system components. The orchestrator "passes the baton" by shoving raw agent output into the next prompt with no validation. Each agent summarizes and re-interprets the previous output, losing precision with every hop.

**How to avoid:**
Define strict typed schemas (Rust structs with serde) at every agent boundary. Agent output is never passed raw — it is deserialized into a validated struct before being forwarded. Invalid output triggers a retry or escalation, not propagation. Treat schema violations as contract failures: retry, repair, or halt — never forward bad state.

**Warning signs:**
- Agent N+1 asks clarifying questions about output from Agent N
- Phase outputs have different module/file naming than the decomposition plan specified
- Review agent flags structure-level issues rather than logic issues (it's reasoning from wrong premises)
- Token costs spike unexpectedly (agents consuming large context to re-orient)

**Phase to address:** Phase implementing agent communication layer (core orchestration); schema contracts must be established before any agent integration work begins.

---

### Pitfall 2: Runaway Token Costs with No Budget Enforcement

**What goes wrong:**
A single misconfigured review loop, an agent that gets stuck re-generating output, or a phase decomposition that spawns more cross-review calls than expected can multiply token costs 3-10x. Because each LLM call in an agentic workflow accumulates context across steps, a 3-agent pipeline with cross-review generates 6+ API calls per phase, each with growing context windows. A real-world case study showed a 300% cost spike — from $1,200 to $4,800/month — from one workflow change. For Athena, if the review loop fails to converge and re-runs 5 times instead of 2, costs multiply silently.

**Why it happens:**
Token pricing seems trivial per call (fractions of a cent) until multiplied across phases, agents, retries, and cross-review passes. Most orchestration systems have no hard budget enforcement — they rely on developers noticing high bills after the fact.

**How to avoid:**
Implement per-run token budget tracking from day one. Before each phase, estimate total tokens needed (input context + expected output per agent + review calls). Enforce a hard cap (configurable via config file). Track cumulative token usage in a run manifest. If a phase is about to exceed its budget, halt and report — never silently continue. Provide dry-run mode that estimates costs without making API calls.

**Warning signs:**
- Review loops run more than 2-3 iterations without convergence
- Context windows growing larger each phase (previous phase outputs accumulating)
- Same module being regenerated multiple times
- No token counter visible in run output

**Phase to address:** Phase implementing API client layer — budget tracking must be built into the API abstraction, not bolted on later.

---

### Pitfall 3: Agent Drift — Quality Degradation Across Extended Runs

**What goes wrong:**
Research (arxiv:2601.04170) quantified this precisely: multi-agent systems exhibit "progressive degradation of agent behavior, decision quality, and inter-agent coherence over extended interaction sequences." After a median of 73 interactions, systems show: 42% reduction in task success rate, 3.2x increase in required interventions, 63% longer completion times. For Athena, this means early phases produce quality code, but later phases (operating with accumulated context pollution) produce code that is inconsistent with earlier architecture decisions, uses different naming conventions, or contradicts established interfaces.

**Why it happens:**
Three mechanisms compound each other: (1) context pollution — accumulated irrelevant information dilutes signal; (2) distributional shift — later phases encounter edge cases diverging from the agent's training distribution; (3) autoregressive reinforcement — agents' outputs become future inputs, compounding small errors through feedback loops. Anthropic's internal testing showed quality drop-off begins at approximately 70% context utilization.

**How to avoid:**
Never pass full phase history to subsequent phases — pass structured summaries only. Maintain an explicit "project manifest" (Rust struct, serialized to disk) that captures architectural decisions, file ownership, interface contracts, and module names. This manifest is the agent's ground truth, not the conversation history. Agents receive the manifest + their specific task, not the entire prior dialogue. Reset agent context between phases; preserve decisions via the manifest, not via context window accumulation.

**Warning signs:**
- Later phases introduce naming inconsistencies (different variable/function names than earlier phases)
- Review agents flag increasing numbers of issues in later phases
- Generated code imports modules with different names than those created earlier
- Token input sizes growing linearly with phase count

**Phase to address:** Phase implementing phase state management and the project manifest structure — must be designed before multi-phase execution is implemented.

---

### Pitfall 4: Infinite Review Loops Without Convergence Guards

**What goes wrong:**
The cross-review model (Agent A reviews Agent B's output) can deadlock when: the reviewing agent has different quality standards than the producing agent, the producing agent cannot satisfy the reviewer's criteria within the project constraints, or the reviewer is hallucinating issues that do not exist. Without a convergence limit, Athena will loop indefinitely, consuming tokens and time. In the worst case, a 3-agent review with no convergence guard burns through API budget and never produces output.

**Why it happens:**
Cross-review is designed to catch blind spots, but "different model perspectives" can mean genuinely incompatible quality judgments. Claude might architect something that Gemini flags as underdocumented; Gemini's suggestions might be rejected by Claude as over-engineered — creating a genuine oscillation. Without a maximum iteration count and an escalation path, the loop is unbounded.

**How to avoid:**
Enforce a maximum review iteration count (default: 3, configurable). After max iterations without pass: (1) log the review disagreement with full context, (2) produce the best available output with a flagged warning, (3) continue to next phase. Distinguish between "blocking" review failures (security issues, API contract violations) and "advisory" failures (style, documentation gaps). Only block on blocking failures. Track the delta between review iterations — if issues are not decreasing, declare convergence failure early.

**Warning signs:**
- Any review loop exceeding 2 iterations
- Review comments addressing the same issue repeatedly
- Review output becoming longer and more detailed (not shorter — a sign of escalating disagreement)
- Phase wall-clock time significantly exceeding estimate

**Phase to address:** Phase implementing cross-review gate logic — convergence guards are not optional; they must be in the initial implementation.

---

### Pitfall 5: Module Isolation Boundary Violations Under Shared Dependencies

**What goes wrong:**
Athena's module isolation strategy (each agent owns specific files) breaks down when agents need to reference shared interfaces, utility functions, or type definitions. Agent A (Claude) generating backend logic needs to define an interface. Agent B (Codex) generating the API layer needs to implement that interface. If both agents are running in parallel, Agent B may generate code against a different interface assumption than what Agent A produces. The result: compilation errors, runtime type mismatches, or silently incompatible implementations that pass review but fail integration.

**Why it happens:**
Parallel execution assumes true independence, but software modules are not truly independent — they share type definitions, error codes, configuration schemas, and utility functions. The "module isolation" architecture prevents file-level conflicts but does not prevent semantic conflicts at interface boundaries.

**How to avoid:**
Phase decomposition must identify shared contracts before parallel assignment. Any interface or type that crosses agent boundaries must be defined in a "contracts phase" before parallel work begins. Store interface contracts in the project manifest (serialized Rust types or language-specific interface files). Agents receive their assigned module + the contracts file; they may not modify the contracts file. Run a contracts validation step at phase merge time before commit. For parallel agents: generate contracts → validate contracts → assign parallel work → merge → integration test.

**Warning signs:**
- Any two agents assigned modules that share imports or function calls
- Phase decomposition assigns "API layer" and "service layer" to different agents without a contracts step
- Compilation errors at phase merge time
- Review agents approving code that calls functions with different signatures

**Phase to address:** Phase implementing phase decomposition and dependency analysis — the contracts-first pattern must be enforced at decomposition time, not discovered at integration.

---

### Pitfall 6: API Reliability Assumptions (No Retry/Circuit Breaker Architecture)

**What goes wrong:**
Anthropic's API has documented 500 (internal error) and 529 (overloaded) transient error codes. OpenAI and Google have equivalent patterns. An autonomous multi-agent run that hits a 529 during phase 4 of 8 has lost all prior work if there is no retry layer. Worse: naive retry logic (immediate retry, no backoff) can trigger 429 rate-limit errors on top of the original error, creating a cascading failure that kills the entire run.

**Why it happens:**
CLI tools are often built with happy-path assumptions. Developers test on lightly-loaded APIs and assume reliability. Production runs on real projects hit API overload at exactly the worst time — when large context payloads are being sent for complex phases.

**How to avoid:**
Implement a proper retry layer in the Rust HTTP client: exponential backoff (start at 1s, cap at 60s) for 429/500/529; no retry for 400/401/403/404 (these are client errors). Respect `Retry-After` response headers when present. Implement a circuit breaker per API provider: after N consecutive failures, fail fast with a clear error rather than continuing to retry. Persist phase state to disk before each API call so a failed run can be resumed rather than restarted. For long phases, use streaming APIs to avoid idle connection timeouts.

**Warning signs:**
- No explicit retry configuration in API client code
- Error handling that catches all errors and retries regardless of type
- No phase state persistence between API calls
- Run configuration has no timeout or max-retry settings

**Phase to address:** Phase implementing API client abstraction — retry logic, circuit breaking, and phase state persistence must be in the initial API client implementation.

---

### Pitfall 7: Prompt Engineering That Produces Inconsistent Agent Roles

**What goes wrong:**
Athena assigns agents based on strengths: Claude for architecture/logic, Gemini for research/docs/APIs, Codex for code generation/boilerplate. But without tightly constrained role prompts, agents override their assigned roles: Claude starts generating boilerplate instead of architecture, Gemini starts modifying code instead of documenting it. This produces overlapping and conflicting output. Worse: an agent assigned "documentation" that actually modifies source files violates the module isolation guarantee.

**Why it happens:**
LLMs are trained to be helpful. If an agent sees a bug while writing documentation, it will fix the bug — even if its role is documentation only. Without explicit constraints on what the agent is and is not allowed to do, agents "helpfully" exceed their scope, undermining the isolation architecture.

**How to avoid:**
Role prompts must be prescriptive about both what the agent does and what it explicitly must not do. For each agent assignment: define the deliverable (what to produce), the boundary (what files/modules are in scope), the prohibition (what files/actions are out of scope), and the output format (typed schema, not freeform). Test role prompts independently before integration. Use structured output enforcement (JSON schema) to prevent agents from returning freeform text that the orchestrator cannot parse. Treat prompt changes as code changes — version them.

**Warning signs:**
- Agent output includes files outside its assigned module list
- Agents adding "helpful" fixes or improvements to out-of-scope areas
- Output schema violations (agent returning structured text instead of JSON)
- Different runs of the same phase producing structurally different outputs

**Phase to address:** Phase implementing agent prompt templates — role constraints must be designed alongside the module isolation strategy, not independently.

---

### Pitfall 8: Phase Decomposition Quality — Garbage In, Garbage Out

**What goes wrong:**
Athena's core value is intelligent phase decomposition. If the decomposition itself is wrong — phases ordered incorrectly, dependencies mis-identified, granularity too coarse or too fine — every downstream phase inherits the error. A phase assigned to "implement auth" before "define user model" is structurally unsound. A phase granularity that assigns one agent "the entire backend" prevents parallelization and forces the agent to exceed context limits.

**Why it happens:**
Phase decomposition is itself an LLM call, subject to the same hallucination and reasoning errors as any other call. The orchestrator's own reasoning about the project may be wrong. Because decomposition happens first and errors propagate forward, decomposition errors have the highest impact of any failure mode.

**How to avoid:**
Validate decomposition output against a set of structural rules before execution begins: no phase may reference output from a later phase; no single agent task may exceed estimated context limits; all phases must have explicitly listed inputs and outputs; shared contracts must appear before any phase that references them. Run a dry-validation pass that checks these rules without making any other API calls. Surface decomposition output to the user for confirmation on the first run of a project (with option to auto-approve on subsequent runs). Implement decomposition scoring: flag phases with circular dependencies, unbounded scope, or missing input specifications.

**Warning signs:**
- Phase list contains circular references in dependency graph
- A single agent task encompasses multiple architectural layers
- Phase inputs reference artifacts that no prior phase produces
- Decomposition output has no explicit module boundary list

**Phase to address:** Phase implementing core decomposition engine — structural validation rules must be part of the initial decomposition, not a later addition.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Pass raw agent output between phases (no schema) | Faster initial implementation | Cascading failures as system scales; impossible to debug which agent introduced bad state | Never — schema contracts are foundational |
| Single global retry count (no per-error-type logic) | Simpler error handling | Retrying 401 auth errors forever; not retrying 529 overload errors that would recover | Never — error types have different retry semantics |
| No token budget tracking | Faster to ship | Silent cost spikes; impossible to cost-estimate runs; accidental $100+ charges | MVP only if run scope is small and known |
| Hardcoded agent role strings in prompt templates | Fast iteration | Prompt changes are code changes with no version history; behavior regressions are invisible | Never — prompts must be versioned artifacts |
| No phase state persistence | Simpler implementation | Any API failure restarts the entire run from scratch; unusable for long projects | Never — state persistence protects user investment |
| Infinite review loop (no convergence guard) | "Thorough" review feeling | Deadlocks, runaway token costs, stuck runs | Never — convergence guards are safety mechanisms |
| Module isolation via naming convention only (no enforcement) | Easier agent prompting | Agents write to out-of-scope files; silent isolation violations | Never — enforcement must be mechanical, not honor-based |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Anthropic API | Not handling 529 (overloaded) distinctly from 500 (internal error) | Both are transient and should retry, but 529 warrants longer backoff since it indicates system-wide load |
| Anthropic API | Using prefill (assistant message prefix) with Opus 4.6 | Opus 4.6 explicitly does not support prefilling — use structured outputs or `output_config.format` instead |
| Anthropic API | Making large non-streaming requests on flaky networks | Use streaming API for any request expected to take >30 seconds; set TCP keepalive |
| OpenAI/Codex API | Assuming same error code semantics as Anthropic | OpenAI 429 includes both rate limit and quota exhausted — these require different responses |
| All three APIs | Sharing a single retry budget across all providers | Provider-specific circuit breakers prevent one failing provider from exhausting the shared retry budget |
| Git integration | Agents committing directly to main branch | All agent work should go to isolated branches/worktrees, merged only after review gate passes |
| Git integration | Using `git add .` in agent-generated commits | Only stage files within the agent's assigned module boundaries; staging everything breaks isolation guarantees |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Growing context per phase (accumulating all prior phase output) | Token cost grows quadratically with phase count; later phases slower and worse quality | Pass structured manifest + task-specific context only; never full history | By phase 4-5 on a medium project (7+ phases total) |
| Sequential agent calls where parallel is safe | Run time = sum of all agent times; underutilizes parallel execution | Athena's dependency analysis must identify truly independent tasks and run them concurrently via Tokio tasks | Immediately visible on any project with 5+ phases |
| Blocking on synchronous HTTP in async Rust | Tokio executor thread starvation; apparent deadlock under load | Use `reqwest` with async features; never use blocking HTTP clients inside async contexts | At 2+ concurrent agent calls |
| No streaming for large code generation tasks | Network idle timeouts kill long-running requests silently | Enable streaming for any request with `max_tokens > 4000`; set TCP keepalive | On slow networks or requests that take >60s |
| Retrying immediately after 429 without reading `Retry-After` | Triggers acceleration limits per Anthropic docs; compounds the problem | Read `Retry-After` header; if not present, use exponential backoff starting at 5s | Any run with high concurrency or large context payloads |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Writing API keys to disk in plaintext (e.g., in run logs or state files) | Key exposure in log files, which may be shared in bug reports or stored in version control | Never log API keys; redact keys from all output; read only from env vars or config file with mode 600 |
| Agent-generated code containing injected instructions (prompt injection via codebase) | If Athena analyzes an existing codebase, adversarial comments/docs could hijack agent behavior | Treat all codebase input as untrusted user data; do not pass raw file content directly into system prompt; use structured extraction |
| Passing user API keys between agents (e.g., Agent B using Agent A's credentials) | Key misuse, unexpected charges on wrong account | Each provider has one client instance initialized once from env config; keys never appear in inter-agent messages |
| No output sandboxing — agents writing arbitrary files | An agent that "helpfully" writes outside its module directory could corrupt the project | Enforce file write restrictions at the orchestrator level; validate every file path before write against allowed module list |
| Trusting agent-generated git commit messages verbatim | Agents may include sensitive context (API responses, internal paths) in commit messages | Review and sanitize commit messages; strip content that looks like credentials, API responses, or file system paths |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent progress during long multi-phase runs | User cannot tell if the tool is working or stuck; abandons run | Stream phase progress to stdout in real time: current phase, agent assigned, tokens used, estimated remaining |
| No cost estimate before run starts | User is surprised by $10-50 API charges on large projects | Implement dry-run mode that estimates token count and cost per phase without calling APIs; show estimate and prompt for confirmation |
| Opaque failure messages ("Agent returned error") | User cannot diagnose which agent failed, why, or how to fix it | Include: which agent, which phase, error type, retry count, whether retryable, suggested action |
| No resume capability after partial failure | User must restart entire run after phase 6 of 8 fails | Persist phase state to disk; support `--resume` flag that picks up from last successful phase |
| Dumping raw LLM output to terminal | Walls of unstructured text; impossible to audit | Structured phase report: what was built, which files, what the reviewer said, any warnings |

---

## "Looks Done But Isn't" Checklist

- [ ] **Retry logic:** Verify that 400/401/403 errors are NOT retried (they are client errors that will never resolve), and 429/500/529 ARE retried with backoff
- [ ] **Cross-review:** Verify that review output is parsed and validated — not just trusting the reviewer said "PASS" in natural language
- [ ] **Module isolation:** Verify that the orchestrator checks file paths in agent output against the allowed module list before writing — not relying on agent compliance
- [ ] **Token tracking:** Verify that token counts are accumulated across the full run, not just per-call — per-call tracking misses the compounding effect
- [ ] **Convergence guards:** Verify that review loops have a hard maximum iteration count enforced in code, not just documented in prompts
- [ ] **Phase state:** Verify that a simulated mid-run crash can be recovered with `--resume` without re-running prior phases
- [ ] **Cost estimate:** Verify that dry-run mode produces an estimate within 20% of actual run cost (test on a small project)
- [ ] **Decomposition validation:** Verify that the decomposition validator catches circular phase dependencies before execution begins
- [ ] **API key safety:** Verify that API keys never appear in log output, state files, or generated commit messages

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Cascading failure from bad cross-agent handoff | HIGH | Identify first phase where schema violation occurred; re-run from that phase with fixed prompt; requires phase state persistence |
| Runaway token cost from infinite review loop | MEDIUM | Kill the process; inspect review log to identify oscillation cause; adjust review prompt or lower review strictness for affected phase; re-run from that phase |
| Agent drift in later phases producing inconsistent code | HIGH | Manual review of all phases after drift onset; update project manifest with explicit decisions that drift erased; re-run affected phases with manifest as ground truth |
| API key exposure in logs | HIGH | Rotate all exposed API keys immediately; audit log files for distribution; sanitize logs going forward |
| Module isolation violation (agent wrote outside boundary) | MEDIUM | Identify out-of-scope files; determine if they conflict with the owning agent's output; either discard or integrate with explicit merge review |
| Decomposition produced circular dependencies | LOW | Re-run decomposition with explicit instruction to resolve circular dependencies; validate before proceeding |
| Phase state lost (no persistence) | HIGH | Full re-run from start; no shortcut; build state persistence before this happens |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Untyped cross-agent handoffs | Agent communication layer (core orchestration) | Integration test: inject malformed output from Agent A; verify Agent B rejects and retries rather than propagating |
| Runaway token costs | API client abstraction | Cost test: run a known project in dry-run mode; verify estimate matches actual within 20%; verify budget cap halts the run |
| Agent drift across phases | Phase state management / project manifest design | Regression test: run a 6+ phase project; verify module names are consistent between phase 1 and phase 6 output |
| Infinite review loops | Cross-review gate implementation | Loop test: configure a review prompt guaranteed to fail; verify run halts at max iterations, not earlier and not later |
| Module isolation boundary violations | Phase decomposition + agent prompt templates | Boundary test: simulate agent writing to out-of-scope file; verify orchestrator rejects the write |
| API reliability / no retry architecture | API client abstraction (must be earliest implementation phase) | Fault injection: simulate 529 errors; verify exponential backoff; verify correct phases resume after recovery |
| Inconsistent agent role prompts | Agent prompt template design | Prompt test: run same prompt 5 times; verify structural output consistency; verify agent does not modify out-of-scope files |
| Bad phase decomposition | Core decomposition engine | Validation test: provide a project with circular dependencies; verify validator catches it before any API call is made |

---

## Sources

- [Multi-agent workflows often fail. Here's how to engineer ones that don't. — GitHub Blog](https://github.blog/ai-and-ml/generative-ai/multi-agent-workflows-often-fail-heres-how-to-engineer-ones-that-dont/)
- [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap — Towards Data Science](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)
- [Why Multi-Agent AI Systems Fail and How to Fix Them — Galileo](https://galileo.ai/blog/multi-agent-ai-failures-prevention)
- [Why Multi-Agent LLM Systems Fail — Galileo (second article)](https://galileo.ai/blog/multi-agent-llm-systems-fail)
- [Agent Drift: Quantifying Behavioral Degradation in Multi-Agent LLM Systems — arXiv 2601.04170](https://arxiv.org/html/2601.04170)
- [Multi-Agent AI Gone Wrong: How Coordination Failure Creates Hallucinations — Galileo](https://galileo.ai/blog/multi-agent-coordination-failure-mitigation)
- [7 Ways Multi-Agent AI Fails in Production — TechAhead](https://www.techaheadcorp.com/blog/ways-multi-agent-ai-fails-in-production/)
- [Why 40% of Multi-Agent AI Projects Fail — SoftwareSeni](https://www.softwareseni.com/why-forty-percent-of-multi-agent-ai-projects-fail-and-how-to-avoid-the-same-mistakes/)
- [How AI Agents Handle Stalled Tasks and Timeouts — DEV Community](https://dev.to/bobrenze/how-ai-agents-handle-stalled-tasks-and-timeouts-lessons-from-my-production-failure-1jj9)
- [Why Multi-Agent Orchestration Collapses: Deadlocks, Infinite Loops, Memory Overwrites — DEV Community](https://dev.to/onestardao/-ep-6-why-multi-agent-orchestration-collapses-deadlocks-infinite-loops-and-memory-overwrites-1e52)
- [Solving Parallel Workflow Conflicts Between AI Agents in Shared Codebases — Medium](https://medium.com/@raminmammadzada/solving-parallel-workflow-conflicts-between-ai-agents-and-developers-in-shared-codebases-286504422125)
- [Git Worktrees for Parallel AI Coding Agents — Upsun Developer Center](https://devcenter.upsun.com/posts/git-worktrees-for-parallel-ai-coding-agents/)
- [Anthropic API Errors — Official Documentation](https://platform.claude.com/docs/en/api/errors)
- [Control Costs in LLM Agents Effectively — ProsperaSoft](https://prosperasoft.com/blog/artificial-intelligence/ai-agent/llm-agent-api-costs/)
- [LLM Economics: How to Avoid Costly Pitfalls — AI Accelerator Institute](https://www.aiacceleratorinstitute.com/llm-economics-how-to-avoid-costly-pitfalls/)
- [Tackling Rate Limiting for LLM Apps — Portkey](https://portkey.ai/blog/tackling-rate-limiting-for-llm-apps/)
- [AI Agent Security Cheat Sheet — OWASP](https://cheatsheetseries.owasp.org/cheatsheets/AI_Agent_Security_Cheat_Sheet.html)
- [Prompt Injection Attacks: The Most Common AI Exploit in 2025 — Obsidian Security](https://www.obsidiansecurity.com/blog/prompt-injection)
- [Rust Libraries for LLM Orchestration 2026 — dasroot.net](https://dasroot.net/posts/2026/02/rust-libraries-llm-orchestration-2026/)

---

*Pitfalls research for: Athena — multi-agent AI orchestration CLI*
*Researched: 2026-03-12*