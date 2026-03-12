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
