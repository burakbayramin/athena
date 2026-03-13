# Phase 6: Module Isolation - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Enforce strict file ownership per agent — no two agents can be assigned overlapping files in the same phase — and route tasks to agents using a skill taxonomy. Delivers: IsolationManager with file ownership validation, skill-based agent routing (SkillTag → AgentKind), conflict detection across parallel phases, and post-execution file audit. No execution logic (Phase 7), no progress display (Phase 8), no parallel dispatch (Phase 10).

</domain>

<decisions>
## Implementation Decisions

### Skill Taxonomy Design
- Static routing table: hardcoded map from SkillTag to preferred AgentKind
- Mapping: rust/architecture/logic → Claude, docs/research/api → Gemini, codegen/boilerplate → Codex
- Multi-tag tiebreaker: majority vote across all skill tags on a task, ties broken by fixed priority (Claude > Gemini > Codex)
- Default fallback for unrecognized tags: Claude (strongest general-purpose model)
- Hardcoded for v1 — no user-configurable routing table
- Routing rationale visible in --verbose mode only; default output shows agent assignment per task

### File Conflict Resolution
- Hard block before dispatch: reject the plan with a clear IsolationError listing conflicting files, which tasks claim them, and a fix hint ("split into separate phases or consolidate into one task")
- Ownership checked within phase AND across parallel phases (phases with no dependency that could run concurrently must have disjoint file sets)
- Exact file path matching only — no directory-level overlap detection
- Post-execution audit: after a task runs, compare actual output files against declared expected_output_files. Log unexpected files as warnings, don't block

### Routing Fallback Behavior
- No strong match (all tags unrecognized): silently default to Claude — consistent with taxonomy default
- Best-match agent unavailable (circuit breaker tripped): fallback to next-best agent from routing table
- ALL agents unavailable: fail with clear error listing which providers are down, suggest checking API keys/quotas
- No waiting or retry loops for tripped providers — circuit breaker recovery probing (Phase 2) handles that independently

### Assignment Granularity
- Per-task assignment: each task within a phase can go to a different agent based on its skill tags
- Sequential overlap within phase allowed: later tasks can modify files produced by earlier tasks in the same phase (tasks execute sequentially per Phase 5 decision)
- Cross-phase conflict checking only applies to parallel-eligible phases (those with no mutual dependency)
- Agent assignment stored on TaskSpec: add `assigned_agent: Option<AgentKind>` field — None before routing, Some after
- IsolationManager lives in ath-orchestrator crate (not ath-planner)

### Claude's Discretion
- Internal module structure within ath-orchestrator
- Exact routing table entries for edge-case skill tags
- IsolationError variant design and error message formatting
- Post-execution audit implementation details
- How to collect actual output files for the audit step

</decisions>

<specifics>
## Specific Ideas

- The routing table mirrors the project's agent positioning: Claude for architecture/logic, Gemini for research/docs, Codex for code generation — this should be the mental model
- File ownership validation across parallel phases prevents git conflicts that would only surface during Phase 10 parallel execution — catch it early
- The `assigned_agent: Option<AgentKind>` pattern on TaskSpec follows the "enrich in place" approach — downstream phases (7, 8) read assignments directly from the plan without carrying a separate map

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `TaskSpec` (ath-types/src/plan.rs): Has skill_tags and expected_output_files — direct inputs for routing and ownership. Will gain `assigned_agent` field
- `AgentKind` enum (ath-types/src/agent.rs): Claude/Gemini/Codex with provider_name() and model() — routing target
- `SkillTag` (ath-types/src/project.rs): Newtype wrapper around String — routing table matches on inner value
- `ExecutionPlan` (ath-types/src/plan.rs): Has parallel_groups — needed for cross-phase conflict checking
- `ValidationError` (ath-types/src/error.rs): Error-with-hint pattern — extend for IsolationError

### Established Patterns
- `thiserror` for domain errors, `anyhow` at binary boundary
- All types derive Debug, Clone, Serialize, Deserialize, PartialEq
- `validate()` returns `Result<(), ValidationError>` with fix hints
- Tasks within a phase are sequential (Phase 5 decision)
- Circuit breaker per provider (Phase 2) — IsolationManager queries breaker state for routing fallback

### Integration Points
- ath-orchestrator crate (stub exists) — IsolationManager goes here
- ath-orchestrator depends on ath-types (for TaskSpec, AgentKind, ExecutionPlan) and ath-agents (for circuit breaker state queries)
- Downstream: Phase 7 (PhaseRunner) reads assigned_agent from TaskSpec to dispatch, Phase 10 (Parallel Execution) relies on cross-phase file disjointness validated here

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-module-isolation*
*Context gathered: 2026-03-13*
