# Roadmap: Athena

## Overview

Athena is built infrastructure-first, following the 6-tier dependency order from architecture research. The foundation (shared types and config) must exist before any agent client can compile. Agent clients must be proven reliable before the orchestration coordinator can delegate to them. The phase decomposition engine — Athena's core value — is validated in isolation before it drives the full execution pipeline. Sequential execution ships first; parallel execution is layered on once isolation is proven at scale. Every phase delivers a coherent, independently testable capability that the next phase builds on.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation** - Shared types, config loading, and unified error system
- [ ] **Phase 2: Agent Clients** - Claude, Gemini, and Codex API actors with reliability primitives
- [ ] **Phase 3: Git Layer** - In-process git commits with phase and agent metadata
- [ ] **Phase 4: Input Parsing** - Natural language, spec file, and codebase analysis into ProjectSpec
- [ ] **Phase 5: Phase Decomposition** - ProjectAnalyzer + PhasePlanner producing validated dependency DAG
- [ ] **Phase 6: Module Isolation** - IsolationManager with file ownership, agent routing, and skill taxonomy
- [ ] **Phase 7: Phase Runner and Review** - PhaseRunner state machine + ReviewEngine with cross-agent review gates
- [ ] **Phase 8: CLI and Progress** - clap shell, terminal progress reporting, and dry-run mode
- [ ] **Phase 9: Reporting and Error Quality** - Structured final report, token cost tracking, actionable error messages
- [ ] **Phase 10: Parallel Execution** - Parallel independent phase dispatch via tokio JoinSet

## Phase Details

### Phase 1: Foundation
**Goal**: The shared type system, config loading, and error hierarchy are in place — every downstream crate can import stable, validated interfaces without circular dependencies
**Depends on**: Nothing (first phase)
**Requirements**: INPT-04, PLAN-04
**Success Criteria** (what must be TRUE):
  1. Running `ath --version` succeeds and config is loaded from env vars and config file without panicking
  2. Providing an invalid or missing API key produces a typed error (not a panic) with a clear message identifying which key is missing
  3. The typed inter-agent schemas (AgentRequest, AgentResponse, ReviewVerdict, ProjectSpec) can be serialized and deserialized round-trip without data loss
  4. All internal modules import from the shared types crate — no duplicated type definitions exist in the project
**Plans**: 4 plans

Plans:
- [x] 01-01-PLAN.md — Cargo workspace scaffold, all 7 ath-* crate stubs, workspace-level dependency management
- [ ] 01-02-PLAN.md — ath-types: error hierarchy + all inter-agent schemas (ProjectSpec, AgentRequest, AgentResponse, ReviewVerdict, PhaseRecord) with validation and round-trip tests
- [ ] 01-03-PLAN.md — ath-config: ConfigStore with layered TOML/env loading, precedence merge, graceful degradation
- [ ] 01-04-PLAN.md — ath-cli: binary wiring with config load, provider status, error display formatting

### Phase 2: Agent Clients
**Goal**: Athena can call Claude, Gemini, and Codex APIs reliably — with retry, backoff, and circuit-breaker behavior — through a uniform trait interface
**Depends on**: Phase 1
**Requirements**: ORCH-05
**Success Criteria** (what must be TRUE):
  1. Athena can send a prompt to Claude and receive a structured response without manual intervention
  2. Athena can send a prompt to Gemini and receive a structured response without manual intervention
  3. Athena can send a prompt to Codex and receive a structured response without manual intervention
  4. A 429 or 500 error triggers exponential backoff (1s to 60s cap) and retries automatically — it does not surface to the user as an unhandled error
  5. After 3 consecutive failures on one provider, the circuit breaker trips and reports which provider is unavailable
**Plans**: TBD

Plans:
- [ ] 02-01: AgentBackend trait definition and mock implementation for testing
- [ ] 02-02: ClaudeActor — tokio actor with mpsc channels, genai integration, backoff, circuit breaker
- [ ] 02-03: GeminiActor — tokio actor with provider-specific JSON mode validation
- [ ] 02-04: CodexActor — tokio actor for OpenAI/Codex endpoint
- [ ] 02-05: Actor integration tests — round-trip prompt/response for all three providers

### Phase 3: Git Layer
**Goal**: Athena can commit generated code to a local git repository after each phase, with per-phase metadata in commit messages, using in-process git2 without a system dependency
**Depends on**: Phase 1
**Requirements**: OUTP-01
**Success Criteria** (what must be TRUE):
  1. After a phase completes, a git commit appears in the local repo containing only the files produced by that phase
  2. The commit message includes phase name, agent responsible, and task identifier
  3. Running `git log` on the target repo shows one commit per executed phase — no extra or missing commits
  4. GitLayer works in a repository with no prior commits (initial commit case) without panicking
**Plans**: TBD

Plans:
- [ ] 03-01: GitLayer struct — git2 Repository management with spawn_blocking guard
- [ ] 03-02: Stage, commit, and metadata embedding for per-phase commits
- [ ] 03-03: Edge case handling — initial commit, empty diff, merge conflict detection

### Phase 4: Input Parsing
**Goal**: Athena accepts a project description in natural language, as a spec file, or as a pointer to an existing codebase, and produces a normalized ProjectSpec in all three cases
**Depends on**: Phase 2
**Requirements**: INPT-01, INPT-02, INPT-03
**Success Criteria** (what must be TRUE):
  1. User can run `ath run "build me a REST API for a todo app"` and Athena produces a ProjectSpec with named goals and constraints — no crash, no empty output
  2. User can run `ath run --spec ./spec.md` and Athena parses the markdown file into the same ProjectSpec structure
  3. User can run `ath run --codebase ./my-project` and Athena analyzes the existing files and produces a ProjectSpec describing next-step goals
  4. All three input modes produce a ProjectSpec that passes serde deserialization — malformed LLM output is caught with an actionable error, not a panic
**Plans**: TBD

Plans:
- [ ] 04-01: CLI argument surface for run subcommand (input mode flags)
- [ ] 04-02: Natural language input normalization — LLM call to extract ProjectSpec
- [ ] 04-03: Spec file parser — markdown/structured document to ProjectSpec
- [ ] 04-04: Codebase analyzer — file tree traversal, summarization, next-step extraction
- [ ] 04-05: ProjectSpec validation — serde deserialization with structured error on malformed output

### Phase 5: Phase Decomposition
**Goal**: Given a ProjectSpec, Athena decomposes it into an ordered, dependency-validated phase plan with parallelism flags — and can catch structural errors (circular deps, missing contracts) before any API call is made
**Depends on**: Phase 4
**Requirements**: PLAN-01, PLAN-02, PLAN-03
**Success Criteria** (what must be TRUE):
  1. For a given ProjectSpec, Athena produces a phase list with named tasks, explicit dependency edges, and a parallelism flag per phase
  2. The dependency graph passes structural validation — circular dependencies are detected and reported with the cycle path, not silently ignored
  3. Phases that have no dependency on each other are flagged as parallel-eligible in the plan output
  4. Running `ath run --dry-run` on any project prints the full phase plan with dependency table — without making any LLM or git call
**Plans**: TBD

Plans:
- [ ] 05-01: PhasePlanner — DAG construction via LLM call with structured output enforcement
- [ ] 05-02: Topological sort and parallelism flag assignment
- [ ] 05-03: Structural validator — circular dependency detection, contracts-before-consumers check
- [ ] 05-04: Dry-run projection — plan display without execution (wired to PLAN-05)

### Phase 6: Module Isolation
**Goal**: Athena enforces strict file ownership per agent — no two agents can be assigned overlapping files in the same phase — and routes tasks to agents using a skill taxonomy
**Depends on**: Phase 5
**Requirements**: ORCH-01, ORCH-02, ORCH-03
**Success Criteria** (what must be TRUE):
  1. Each task in a phase is assigned to exactly one agent (Claude, Gemini, or Codex) based on skill taxonomy match — no unassigned tasks
  2. If the planner attempts to assign overlapping files to two agents in the same phase, IsolationManager blocks the dispatch and reports the conflict before any LLM call is made
  3. After a phase executes, each agent's output files are confirmed to be disjoint — no file appears in two agents' output sets
  4. The agent assignment rationale (which skill matched which agent) is visible in --verbose output
**Plans**: TBD

Plans:
- [ ] 06-01: Skill taxonomy definition — task type to agent capability mapping
- [ ] 06-02: Agent router — skill-based task-to-agent assignment logic
- [ ] 06-03: IsolationManager — file ownership registry with pre-dispatch conflict check
- [ ] 06-04: Post-phase isolation audit — verify disjoint output sets

### Phase 7: Phase Runner and Review
**Goal**: Athena executes a full phase plan end-to-end — running agent tasks, routing output to a cross-agent reviewer, enforcing the review gate, and retrying on failure — using an enum state machine that makes invalid transitions impossible
**Depends on**: Phase 6, Phase 3
**Requirements**: QUAL-01, QUAL-02, QUAL-03
**Success Criteria** (what must be TRUE):
  1. A phase executes from Pending through Running to AwaitingReview to Complete without human intervention when the agent output is valid
  2. When a reviewer rejects agent output, phase progression is blocked and Athena retries automatically with the reviewer's feedback injected into the next attempt
  3. After 3 failed review attempts, Athena halts phase execution and reports the final reviewer verdict — it does not enter a fourth retry or loop indefinitely
  4. Cross-agent review pairing is enforced — the author agent's output is never reviewed by the same agent that produced it
  5. A complete sequential run (all phases in order, each reviewed) completes successfully on a real project without manual intervention
**Plans**: TBD

Plans:
- [ ] 07-01: PhaseState enum machine — Pending, Running, AwaitingReview, Complete, ReviewFailed transitions
- [ ] 07-02: ReviewEngine — cross-vendor pairing table, structured pass/fail verdict extraction
- [ ] 07-03: Retry loop with convergence guard — max 3 attempts, reviewer feedback injection
- [ ] 07-04: AgentCoordinator — sequential phase dispatch, JoinSet task fan-out within a phase
- [ ] 07-05: End-to-end integration test — full pipeline on a synthetic project

### Phase 8: CLI and Progress
**Goal**: Athena has a complete CLI surface with subcommands, and the terminal shows real-time phase and agent status during execution — so users are never looking at a silent, hung process
**Depends on**: Phase 7
**Requirements**: PLAN-05, OUTP-02
**Success Criteria** (what must be TRUE):
  1. `ath run`, `ath init`, and `ath report` are distinct subcommands with --help output describing their arguments
  2. During execution, the terminal updates in real time showing: current phase name, active agent, task status (running/complete/failed)
  3. `ath run --dry-run` prints the full phase plan and exits without executing any LLM or git call
  4. `ath run --verbose` shows full agent prompt/response transcripts in addition to normal progress output
**Plans**: TBD

Plans:
- [ ] 08-01: clap subcommand definitions — run, init, report with full argument surface
- [ ] 08-02: Progress reporter — indicatif + tracing-indicatif integration for per-phase status display
- [ ] 08-03: Verbose mode — full agent transcript output behind --verbose flag
- [ ] 08-04: Dry-run mode wiring — plan output path without execution (integrates Phase 5 dry-run)

### Phase 9: Reporting and Error Quality
**Goal**: Athena produces a structured final report after every run, tracks token usage and estimated cost per phase per agent, and surfaces errors with enough context to act on them without reading source code
**Depends on**: Phase 7
**Requirements**: QUAL-04, OUTP-03, OUTP-04
**Success Criteria** (what must be TRUE):
  1. After a completed run, Athena writes a report to disk containing: phase table (phase/task/agent/dependency/parallelism), review outcomes per phase, and overall status
  2. The report includes token counts and estimated cost per phase per agent — not just totals
  3. An API authentication error message names the provider and tells the user which env var or config key to check
  4. A review failure error message includes the phase name, which agent reviewed, which attempt failed, and the reviewer's verdict text
  5. A schema validation error identifies the field that failed deserialization and the raw value that was received
**Plans**: TBD

Plans:
- [ ] 09-01: ReportWriter — JSON + Markdown report generation from run state
- [ ] 09-02: Token usage accumulator — per-phase, per-agent tracking across the full run
- [ ] 09-03: Cost estimator — token counts to dollar estimates using known provider pricing
- [ ] 09-04: Actionable error messages — context-rich formatting for API, review, and schema error types

### Phase 10: Parallel Execution
**Goal**: Athena executes independent phases in parallel simultaneously across agents — with isolation verified before any concurrent dispatch — reducing total run time for projects with parallelizable work
**Depends on**: Phase 7
**Requirements**: ORCH-04
**Success Criteria** (what must be TRUE):
  1. Phases flagged as parallel-eligible in the plan execute concurrently — their start times overlap, not sequential
  2. Running a project with two independent parallel phases completes faster than running the same project with those phases forced sequential
  3. When two parallel phases attempt to write to the same file, IsolationManager blocks the second dispatch before any LLM call — the conflict is reported, not silently skipped
  4. After parallel phases complete, all output files are committed to git with correct per-phase metadata — no interleaved or dropped commits
**Plans**: TBD

Plans:
- [ ] 10-01: Parallel dispatch enablement in AgentCoordinator — activate JoinSet fan-out for independent phases
- [ ] 10-02: Cross-phase isolation enforcement — IsolationManager extended to concurrent phase boundaries
- [ ] 10-03: Git commit sequencing — serialize concurrent phase commits to prevent race conditions
- [ ] 10-04: Parallel execution integration test — verified faster-than-sequential on synthetic parallelizable project

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 1/4 | In progress | - |
| 2. Agent Clients | 0/5 | Not started | - |
| 3. Git Layer | 0/3 | Not started | - |
| 4. Input Parsing | 0/5 | Not started | - |
| 5. Phase Decomposition | 0/4 | Not started | - |
| 6. Module Isolation | 0/4 | Not started | - |
| 7. Phase Runner and Review | 0/5 | Not started | - |
| 8. CLI and Progress | 0/4 | Not started | - |
| 9. Reporting and Error Quality | 0/4 | Not started | - |
| 10. Parallel Execution | 0/4 | Not started | - |
