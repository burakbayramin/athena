---
id: M001
provides:
  - Rust CLI binary (`ath`) with 7 workspace crates and full multi-agent orchestration pipeline
  - Three AI agent backends (Claude, Gemini, Codex) with circuit breaker and retry infrastructure
  - DAG-based phase decomposition with topological sort, parallel group detection, and critical path computation
  - Module isolation with file ownership validation and cross-agent review gates
  - Parallel execution via tokio JoinSet with pre-dispatch isolation checks
  - Durable run-report artifacts with token accounting and cost estimation
key_decisions:
  - Rust workspace with 7 crates (ath-types at root, ath-cli as binary) for strict dependency layering
  - Typestate pattern for PhaseRunner enforces valid state transitions at compile time
  - Arc<AgentRegistry> for safe concurrent dispatch across tokio JoinSet tasks
  - CircuitBreaker per provider prevents retry storms on unhealthy backends
  - Cross-agent review (never self-review) using discriminant-based exclusion
  - Static skill taxonomy routing 15 tags across 3 providers with priority tiebreaking
  - Module isolation via exact file path ownership — no merge-based conflict resolution
  - genai 0.5 unified client with AuthResolver closure for multi-provider key injection
  - Kahn's algorithm for topological sort with DFS cycle detection
  - User-owned API keys only — no SaaS key management
patterns_established:
  - thiserror 2.0 field naming — rename `source` fields to `message` to avoid #[source] conflicts
  - Typed errors with hint() methods providing actionable user guidance
  - serde(default, skip_serializing_if) for backward-compatible schema evolution
  - TDD red/green commit pairs for test-first development
  - MockBackend with sequenced/always_ok/failing modes for test orchestration
  - spawn_blocking wrapper pattern for sync git2 operations in async context
  - Request builder closure pattern for retry-with-feedback loops
  - ProgressObserver seam separating execution from presentation
  - Discriminant-based vote counting for agent routing tiebreaks
observability_surfaces:
  - `ath report` renders saved run reports with per-phase token usage, agent assignments, review outcomes, and cost estimates
  - `ath run --verbose` shows grouped executor/reviewer/retry transcripts with secret redaction
  - `ath run --dry-run` previews cached routed execution plans without API calls
  - TerminalProgressReporter with interactive board updates and plain-text fallback
  - Persisted run artifacts at `.ath/runs/<run-id>/report.json` with `latest.txt` pointer
requirement_outcomes:
  - id: R01
    from_status: active
    to_status: validated
    proof: "InputMode enum with NaturalLanguage/SpecFile/Codebase variants in ath-planner/src/input/mod.rs, parse_to_project_spec with 3-attempt retry, wired through CLI (S04 summaries, 33 planner tests)"
  - id: R02
    from_status: active
    to_status: validated
    proof: "topological_sort, compute_parallel_groups, critical_path_length in ath-planner/src/decompose/dag.rs with validate_plan catching circular deps, missing targets, orphaned goals (S05 summaries, 28 decompose tests)"
  - id: R03
    from_status: active
    to_status: validated
    proof: "15-tag routing table in ath-orchestrator/src/taxonomy.rs, route_task majority vote in router.rs, assign_all_tasks batch routing (S06 summaries, 14 router tests)"
  - id: R04
    from_status: active
    to_status: validated
    proof: "check_isolation file ownership validation and audit_outputs in ath-orchestrator/src/isolation.rs, pre-dispatch IsolationViolation in coordinator (S06/S10 summaries, 13 isolation tests)"
  - id: R05
    from_status: active
    to_status: validated
    proof: "ClaudeHandle, GeminiHandle, CodexHandle in ath-agents/src/actor/ implementing AgentBackend trait with genai 0.5 client (S02 summaries, 59 agent tests, dyn dispatch proven)"
  - id: R06
    from_status: active
    to_status: validated
    proof: "GitLayer.stage_and_commit with trailer metadata, AsyncGitLayer wrapper in ath-git, CommitResult with committed flag (S03 summaries, 35 git tests)"
  - id: R07
    from_status: active
    to_status: validated
    proof: "select_reviewer with cross-agent exclusion, build_review_prompt, parse_review_verdict in ath-orchestrator/src/review.rs, review gate in PhaseState typestate (S07 summaries, 17 review tests)"
  - id: R08
    from_status: active
    to_status: validated
    proof: "run_phase retry loop with feedback injection in ath-orchestrator/src/phase_runner.rs, max 3 attempts, build_retry_prompt with 500-char truncation (S07 summaries, 14 phase runner tests)"
  - id: R09
    from_status: active
    to_status: validated
    proof: "RunReport/RunTotals/PhaseSummary in ath-types/src/report.rs, persistence at .ath/runs/, cost.rs pricing, ath report rendering (S09 summaries)"
  - id: R10
    from_status: active
    to_status: validated
    proof: "ConfigStore with layered merge (env > project > global > defaults) in ath-config/src/store.rs, available_providers() discovery (S01 summaries, 23 config tests)"
  - id: R11
    from_status: active
    to_status: validated
    proof: "ProgressEvent/ProgressObserver in ath-orchestrator/src/progress.rs, TerminalProgressReporter in ath-cli/src/progress.rs (S08 summaries, 8 progress tests)"
  - id: R12
    from_status: active
    to_status: validated
    proof: "ath run --dry-run loads cached plan from .ath/last-plan.json in ath-cli/src/dry_run.rs, shows assigned agents without API calls (S08 summaries)"
  - id: R13
    from_status: active
    to_status: validated
    proof: "JoinSet parallel dispatch in coordinator.rs, parallel_phases_overlap test proves concurrent execution via peak concurrency counter, isolation-gate pre-check (S10 summaries, 4 parallel tests)"
  - id: R14
    from_status: active
    to_status: validated
    proof: "AgentError.hint() with provider-specific guidance, PhaseRunnerError with reviewer identity, render_error_output centralized in ath-cli/src/main.rs (S09 summaries)"
duration: "2 days (2026-03-12 to 2026-03-13)"
verification_result: passed
completed_at: 2026-03-13
---

# M001: Migration

**Full v1.0 Athena CLI — 7-crate Rust workspace delivering multi-agent orchestration with DAG-based phase decomposition, parallel execution, cross-agent review gates, and durable run reporting across 16,315 lines and 415 passing tests.**

## What Happened

Athena v1.0 was built across 10 sequential slices, each adding a distinct layer of the orchestration pipeline.

**S01 (Foundation)** established the Cargo workspace with 7 crates, workspace-level dependency management, inter-agent type schemas (ProjectSpec, AgentRequest/Response, ReviewVerdict, PhaseRecord) with 24 round-trip serialization tests, the layered ConfigStore (env > project TOML > global TOML > defaults), and the clap-based CLI entry point with provider status reporting.

**S02 (Agent Clients)** defined the agent contract layer: AgentError with retryable classification, CircuitBreaker state machine with deterministic time testing, the async AgentBackend trait, and MockBackend test double. Then it implemented three provider actors (Claude, Gemini, Codex) using genai 0.5 with tokio actor pattern, exponential backoff with Retry-After honor, and proved dyn dispatch via Box<dyn AgentBackend>.

**S03 (Git Layer)** built the git integration: GitError types, CommitMetadata with trailer-based commit messages, GitLayer with stage_and_commit supporting explicit file staging, empty diff detection, and initial commit support, plus AsyncGitLayer for tokio callers via spawn_blocking.

**S04 (Input Parsing)** created the three-mode input pipeline: InputMode enum (NaturalLanguage/SpecFile/Codebase), spec file reader with 50KB cap, codebase scanner with gitignore-aware traversal and key file extraction, parse_to_project_spec with 3-attempt retry loop, and full CLI wiring through ClaudeHandle.

**S05 (Phase Decomposition)** added ExecutionPlan/PhaseSpec/TaskSpec types, topological sort (Kahn's algorithm), parallel group detection, critical path computation, validate_plan with 5 hard error types including transitive contract checking, LLM decomposition prompt with JSON schema, and CLI display of execution plans.

**S06 (Module Isolation)** introduced the skill taxonomy routing 15 tags to 3 agents, majority-vote task router with priority tiebreaking and circuit-breaker fallback, and pre-dispatch file ownership validation with post-execution audit.

**S07 (Phase Runner and Review)** delivered the PhaseState typestate machine (6 states, compile-time review gate), ReviewEngine with cross-agent reviewer selection, structured review prompts and verdict parsing, the run_phase orchestration loop with retry/feedback, and AgentCoordinator driving all phases through the pipeline.

**S08 (CLI and Progress)** modularized the CLI surface (run/init/report commands), built the TerminalProgressReporter with interactive board and plain-text fallback, added verbose transcript rendering with secret redaction, and implemented honest dry-run previews from cached routed plans.

**S09 (Reporting and Error Quality)** created the durable RunReport artifact persisted at `.ath/runs/<run-id>/report.json`, added full token accounting across executors/reviewers/retries, cost estimation with real model pricing, and centralized actionable error rendering with provider-specific guidance.

**S10 (Parallel Execution)** refactored the coordinator from sequential to parallel group dispatch via tokio JoinSet, added pre-dispatch isolation validation, and proved concurrent execution through 4 integration tests measuring overlap, timing, isolation enforcement, and metadata ordering.

## Cross-Slice Verification

The milestone roadmap defines no explicit success criteria in its `## Success Criteria` section. Verification was performed against the 14 validated requirements from PROJECT.md:

1. **Input parsing (3 modes)**: InputMode enum with NaturalLanguage/SpecFile/Codebase in `ath-planner/src/input/mod.rs`. CLI flags `--spec` and `--codebase` wired. 33 planner tests pass.
2. **Dependency analysis**: `topological_sort`, `compute_parallel_groups`, `critical_path_length` in `decompose/dag.rs`. `validate_plan` catches circular deps, missing targets, orphaned goals. 28 decompose tests pass.
3. **Skill-to-agent mapping**: 15-tag routing table in `taxonomy.rs`, majority vote in `router.rs`. 14 router tests pass.
4. **Module isolation**: `check_isolation` validates file disjointness across parallel phases. Pre-dispatch `IsolationViolation` error blocks conflicting plans. 13 isolation tests pass.
5. **Three agent backends**: `ClaudeHandle`, `GeminiHandle`, `CodexHandle` implement `AgentBackend` with genai 0.5. `Box<dyn AgentBackend>` dispatch proven. 59 agent tests pass.
6. **Git commits with metadata**: `stage_and_commit` writes athena:-prefixed subjects with git trailers. `AsyncGitLayer` wraps for tokio. 35 git tests pass.
7. **Cross-review gate**: `select_reviewer` enforces never-same-as-author. PhaseState typestate requires AwaitingReview→Complete transition. 17 review tests pass.
8. **Auto-retry with feedback**: `run_phase` retries up to 3 attempts with `build_retry_prompt` injecting reviewer feedback. 14 phase runner tests pass.
9. **Structured report**: `RunReport`/`RunTotals`/`PhaseSummary` persisted to `.ath/runs/`. `ath report` renders phase table, agents, review outcomes, and costs.
10. **API key configuration**: `ConfigStore` loads from env vars, project `.ath.toml`, and global config with correct precedence. 23 config tests pass.
11. **Real-time progress**: `ProgressEvent`/`ProgressObserver` seam with `TerminalProgressReporter` showing phase/agent/task status. 8 progress tests pass.
12. **Dry-run mode**: `ath run --dry-run` loads cached plan from `.ath/last-plan.json` and displays without API calls.
13. **Parallel execution**: JoinSet dispatch in `coordinator.rs`. `parallel_phases_overlap` test proves concurrent execution via `AtomicU32` peak counter. 4 parallel tests pass.
14. **Actionable errors**: `hint()` methods on `AgentError`, `PhaseRunnerError`, `IsolationError`, `ConfigError`, `ValidationError`. `render_error_output` centralizes CLI formatting.

**Aggregate verification**: 415 tests pass across the workspace. 0 clippy warnings. `cargo build --workspace` succeeds. `ath --version` prints `ath 0.1.0`. 58 Rust source files totaling 16,315 lines across 7 crates.

## Requirement Changes

All 14 requirements transitioned from Active to Validated during this milestone:

- R01 (Input parsing): Active → Validated — 3 input modes implemented and wired through CLI with 33 tests
- R02 (Dependency analysis): Active → Validated — DAG algorithms with topological sort and parallel groups, 28 tests
- R03 (Skill mapping): Active → Validated — 15-tag taxonomy with majority-vote routing, 14 tests
- R04 (Module isolation): Active → Validated — File ownership checks and audit, 13 tests
- R05 (Agent backends): Active → Validated — Claude/Gemini/Codex actors with dyn dispatch, 59 tests
- R06 (Git commits): Active → Validated — stage_and_commit with trailers and async wrapper, 35 tests
- R07 (Cross-review): Active → Validated — Cross-agent reviewer selection with typestate gate, 17 tests
- R08 (Auto-retry): Active → Validated — 3-attempt retry with feedback injection, 14 tests
- R09 (Structured report): Active → Validated — RunReport persistence with cost estimation
- R10 (API key config): Active → Validated — Layered ConfigStore with env/file/default merge, 23 tests
- R11 (Progress reporting): Active → Validated — ProgressObserver seam with terminal reporter, 8 tests
- R12 (Dry-run): Active → Validated — Cached plan preview without API calls
- R13 (Parallel execution): Active → Validated — JoinSet dispatch with overlap proof, 4 tests
- R14 (Actionable errors): Active → Validated — hint() methods and centralized error rendering

## Forward Intelligence

### What the next milestone should know
- The codebase is fully compilable and tested but has not been exercised against real AI provider APIs in an end-to-end integration test. All provider tests use MockBackend.
- genai 0.5 does not expose Retry-After headers from HTTP responses — the `retry_after` field on `RateLimit` errors is always `None`. Real-world rate limit handling may need custom header parsing.
- The `ath init` command is a placeholder — it prints setup guidance but does not write any files.
- Resumable execution (resume from last completed phase) was explicitly deferred to v2.

### What's fragile
- **thiserror 2.0 field naming** — any struct field named `source` will be interpreted as `#[source]` by thiserror, causing compilation errors. This was hit 3 times (AgentError, GitError, and TaskJoin) and requires renaming to `message`. Future types must avoid `source` as a field name.
- **genai API surface** — genai 0.5 is a thin wrapper; breaking changes in genai or upstream provider APIs could affect `classify_error` mapping and `ChatOptions::JsonSpec` construction.
- **Static pricing table** — `cost.rs` hardcodes per-token prices for 4 models. Any model name change or new model requires manual update.

### Authoritative diagnostics
- `cargo test --workspace` — the single most trustworthy signal; 415 tests cover all layers from types through CLI
- `cargo clippy --workspace -- -D warnings` — zero warnings as of completion
- `ath --version` → `ath 0.1.0` confirms the binary builds and runs

### What assumptions changed
- **Assumed Rust was pre-installed** — it wasn't; S01 had to install rustup as a prerequisite
- **Assumed thiserror would accept `source: String`** — thiserror 2.0 changed behavior; established the `message` field naming pattern early
- **Assumed genai would expose Retry-After headers** — it doesn't; retry_after is always None from classify_error

## Files Created/Modified

- `Cargo.toml` — Workspace root with 7 members, shared dependency management
- `crates/ath-types/src/` (8 files) — Core types: ProjectSpec, AgentRequest/Response, ReviewVerdict, PhaseRecord, ExecutionPlan, RunReport
- `crates/ath-config/src/` (5 files) — Layered config: TOML parsing, env loading, ConfigStore with merge
- `crates/ath-agents/src/` (5 files + actor/) — AgentBackend trait, CircuitBreaker, MockBackend, Claude/Gemini/Codex actors
- `crates/ath-git/src/` (5 files) — GitLayer, CommitMetadata, stage_and_commit, AsyncGitLayer
- `crates/ath-planner/src/` (input/ + decompose/) — Input parsing pipeline, DAG decomposition, validation, display
- `crates/ath-orchestrator/src/` (9 files) — PhaseRunner typestate, ReviewEngine, AgentCoordinator, router, taxonomy, isolation, progress
- `crates/ath-cli/src/` (8 files) — CLI commands (run/init/report), progress reporter, verbose transcripts, dry-run cache, cost estimation, error rendering
