# Phase 5: Phase Decomposition - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Given a ProjectSpec, decompose it into an ordered, dependency-validated phase plan with parallelism flags — and catch structural errors (circular deps, missing contracts) before any API call is made. Delivers: ProjectAnalyzer that calls Claude to decompose goals into phases, PhasePlan/TaskSpec types, dependency DAG with topological validation, produces/consumes contract checking, and critical path computation. No agent assignment (Phase 6), no execution logic (Phase 7), no CLI progress display (Phase 8).

</domain>

<decisions>
## Implementation Decisions

### Decomposition Strategy
- Pure LLM decomposition — send ProjectSpec to Claude with a structured prompt, no heuristic pre-processing
- Soft bounds on phase count: prompt guides toward 3-10 phases, validation warns (doesn't block) outside that range
- LLM merges overlapping goals and clarifies vague goals — the decomposition IS the clarification step
- Skill tags attached to each task (e.g., "rust", "api-design", "testing") — actual agent assignment deferred to Phase 6 (Module Isolation)

### Task Granularity
- Each task is an actionable spec: name, description, skill tags, expected output files, and acceptance criteria
- Target 2-5 tasks per phase — enough to be focused without being monolithic
- Tasks within a phase are explicitly ordered (sequential execution, not parallel within phase)
- Each task maps back to the ProjectSpec GoalSpec(s) it fulfills — enables traceability and orphan detection

### Parallelism Detection
- Phase-level parallelism only — tasks within a phase are always sequential
- Dependency edges with numeric IDs: each phase has `depends_on: Vec<u32>` referencing other phase IDs
- Parallelism is implicit: phases with no mutual dependency relationship CAN run concurrently (Phase 10 handles actual concurrent execution)
- Critical path computed from the DAG — longest dependency chain stored for progress estimation (used by Phase 8)

### Validation & Error Catching
- **Hard errors (block execution):** circular dependencies, orphaned goals (no task maps to a ProjectSpec goal), missing dependency targets (depends_on references non-existent phase), empty phases (zero tasks)
- **Contract completeness:** each phase declares what it produces and what it consumes from dependencies. Validation checks that consumed items are actually produced by a listed dependency
- **On validation failure:** retry with feedback — append specific validation errors to the LLM prompt, max 3 attempts (matches Phase 2/4 retry-with-context pattern)
- **Warnings (non-blocking):** unusually large phases, identical skill tag distributions, long critical paths. Shown in output but don't block execution

### Claude's Discretion
- Exact LLM prompt design for decomposition
- Internal module structure (new module in ath-planner)
- Specific type names and field organization for PhasePlan/TaskSpec
- Topological sort algorithm choice
- Warning threshold tuning (what counts as "unusually large")
- How to represent produces/consumes contracts (string labels vs typed enum)

</decisions>

<specifics>
## Specific Ideas

- The retry-with-validation-feedback pattern is proven across Phases 2 and 4 — same approach here but with DAG-specific errors in the feedback
- Contract checking (produces/consumes) is the key differentiator — this catches "Phase 3 needs types from Phase 2 but Phase 2 doesn't produce them" BEFORE any API cost is incurred
- Goal traceability (task -> GoalSpec mapping) supports OUTP-03 (structured final report) downstream

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ProjectSpec` (ath-types/src/project.rs): Input to decomposition — name, description, goals with skill tags, constraints, target language/framework, expected files
- `GoalSpec` / `SkillTag` (ath-types/src/project.rs): Goal structure with skill tags — task goal mapping references these
- `AgentBackend` trait (ath-agents/src/backend.rs): Uniform LLM client interface — inject Claude for decomposition calls
- `parse_to_project_spec` pattern (ath-planner/src/input/mod.rs): Retry loop with validation feedback — template for decompose function
- `MockBackend` (ath-agents/src/mock.rs): Test decomposition without real API calls
- `ValidationError` (ath-types/src/error.rs): Error-with-hint pattern — extend for DAG validation errors

### Established Patterns
- `thiserror` for domain errors, `anyhow` at binary boundary
- All types derive Debug, Clone, Serialize, Deserialize, PartialEq
- `validate()` returns `Result<(), ValidationError>` with fix hints
- JSON mode via `json_schema` field on AgentRequest for structured LLM output
- Request builder closure pattern for retry loop (from Phase 4)

### Integration Points
- ath-planner crate: new `decompose` module alongside existing `input` module
- New types needed in ath-types: PhasePlan (or ExecutionPlan), PhaseSpec, TaskSpec — extend phase.rs or new file
- CLI integration: called after `parse_input` returns ProjectSpec, before Phase 7 execution
- Downstream: Phase 6 reads decomposition output to assign agents, Phase 7 reads it to execute, Phase 8 reads critical path for progress display

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-phase-decomposition*
*Context gathered: 2026-03-12*
