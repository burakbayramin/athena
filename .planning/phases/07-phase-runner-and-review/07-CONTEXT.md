# Phase 7: Phase Runner and Review - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Execute a full phase plan end-to-end — running agent tasks sequentially, routing output to a cross-agent reviewer, enforcing the review gate, and retrying on failure — using a typestate machine that makes invalid transitions a compile-time error. Delivers: PhaseRunner state machine, ReviewEngine with cross-agent pairing, retry loop with convergence guard (max 3 attempts), and AgentCoordinator for sequential phase dispatch. No CLI progress display (Phase 8), no reporting (Phase 9), no parallel execution (Phase 10).

</domain>

<decisions>
## Implementation Decisions

### Review Pairing Strategy
- Never-same-as-author rule: reviewer must be a different AgentKind than the task author
- Per-phase review (not per-task): aggregate all task outputs, send to one reviewer for holistic review
- Reviewer selection for multi-author phases: exclude the majority author (agent with the most tasks). Ties broken by priority (Claude > Gemini > Codex)
- If chosen reviewer's circuit breaker is tripped: fall back to next eligible non-author agent. If all non-author agents are down, fail the phase with a clear error
- Same reviewer used across all retry attempts for a given phase — consistency over fresh perspective

### State Machine Design
- Typestate pattern: each state is a distinct Rust type (Phase<Pending>, Phase<Running>, etc.). Invalid transitions are compile-time errors
- States: Pending → Running → AwaitingReview → Complete OR ReviewFailed → Retrying → Running (loop)
- Retrying state carries attempt_number inside the state machine. Machine itself knows when max (3) is reached and transitions to ReviewFailed automatically
- ReviewFailed after max attempts is terminal and final — the entire run stops. User must fix and re-run

### Retry Behavior
- Full re-run with feedback: re-execute all tasks in the phase from scratch, with reviewer's feedback injected into the prompt
- Feedback injection: structured "Previous Review Feedback" section added to agent prompt with reviewer's reason + code suggestions. Agent sees original task + what went wrong (most recent attempt only, not full history)
- Always use all 3 attempts — no early bail on repeated failures. Third attempt may succeed with different LLM output
- Same reviewer across all attempts for continuity

### Task Output Handling
- Structured JSON response: agent returns JSON with files_produced (path + content), explanation, issues_encountered. Uses json_schema field on AgentRequest
- Files written to disk only after review passes: hold all task outputs in memory until phase review passes, then write files + git commit atomically. No partial writes on review failure
- Reviewer receives full context: original task spec, all produced files with contents, and agent's explanation of what it did
- No pre-validation of acceptance criteria before review — reviewer handles all quality checks as part of the review

### Claude's Discretion
- Exact typestate generic implementation approach
- Review prompt template design
- AgentCoordinator internal structure
- How PhaseRecord is populated during execution
- Error message formatting for ReviewFailed terminal state

</decisions>

<specifics>
## Specific Ideas

- The typestate pattern should prevent impossible state transitions at compile time — this is the key differentiator from a simple enum
- The Retrying state carrying attempt_number means the state machine is self-aware of retry budget
- Holding outputs in memory until review passes means a failed retry is zero-cost in terms of disk/git state — clean mental model
- The "exclude majority author" rule for reviewer selection mirrors the cross-review philosophy: maximize perspective diversity

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ReviewVerdict` (ath-types/src/review.rs): Has `passed`, `severity`, `reviewer`, `reason`, `suggestions` — direct input for review gate decisions
- `PhaseRecord` (ath-types/src/phase.rs): Has `ReviewAttempt` with attempt_number and verdict — audit trail for retry tracking
- `AgentContribution` (ath-types/src/phase.rs): Tracks files_produced and token usage per agent — populate during task execution
- `AgentRequest`/`AgentResponse` (ath-types/src/agent.rs): Request has json_schema field for structured output — use for task execution and review prompts
- `AgentBackend` trait (ath-agents/src/backend.rs): `call()` and `is_available()` methods — PhaseRunner dispatches via this trait
- `CircuitBreaker` (ath-agents/src/circuit_breaker.rs): Per-provider availability tracking — query for reviewer fallback
- `assign_all_tasks` (ath-orchestrator/src/router.rs): Sets assigned_agent on each TaskSpec — runner reads these to dispatch
- `IsolationManager` (ath-orchestrator/src/isolation.rs): File ownership validation — runner can call audit_outputs post-execution

### Established Patterns
- `thiserror` for domain errors, `anyhow` at binary boundary
- All types derive Debug, Clone, Serialize, Deserialize, PartialEq
- `validate()` returns `Result<(), ValidationError>` with fix hints
- Tasks within a phase are sequential (Phase 5 decision)
- Circuit breaker per provider (Phase 2) — query is_available() before dispatch
- Fail-fast on errors (Phase 6 pattern in assign_all_tasks)

### Integration Points
- PhaseRunner lives in ath-orchestrator crate (extends existing module set: taxonomy, router, isolation)
- Depends on: ath-types (plan types, review types, phase record), ath-agents (AgentBackend trait, circuit breaker), ath-git (commit after review passes)
- Downstream: Phase 8 (CLI) wraps PhaseRunner with progress display, Phase 10 (Parallel) adds JoinSet fan-out around sequential runner

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-phase-runner-and-review*
*Context gathered: 2026-03-13*
