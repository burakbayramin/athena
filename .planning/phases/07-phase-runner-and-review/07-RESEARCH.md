# Phase 7: Phase Runner and Review - Research

**Researched:** 2026-03-13
**Domain:** Rust typestate state machines, cross-agent review orchestration, retry loops
**Confidence:** HIGH

## Summary

Phase 7 builds the execution engine that runs phase plans end-to-end. The core challenge is implementing a typestate-based state machine where each phase state (Pending, Running, AwaitingReview, Complete, ReviewFailed, Retrying) is a distinct Rust type, making invalid transitions compile-time errors. The phase runner dispatches tasks to agents sequentially, collects structured JSON outputs, holds them in memory until a cross-agent reviewer passes them, then atomically writes files and git commits.

The existing codebase provides strong foundations: `AgentBackend` trait for dispatch, `MockBackend` for testing, `ReviewVerdict`/`PhaseRecord`/`ReviewAttempt` types for audit trails, `AgentRequest` with `json_schema` for structured output, and `assign_all_tasks` for routing. The main new code is the typestate machine, the ReviewEngine (reviewer selection + prompt + verdict parsing), the retry loop with feedback injection, and the AgentCoordinator that sequences phases.

**Primary recommendation:** Use PhantomData-based typestate for the state machine with consuming `self` transitions. Keep the ReviewEngine as a standalone struct that owns reviewer selection logic and prompt construction. The AgentCoordinator takes `&dyn AgentBackend` providers via a registry map and drives the full pipeline.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Never-same-as-author rule: reviewer must be a different AgentKind than the task author
- Per-phase review (not per-task): aggregate all task outputs, send to one reviewer for holistic review
- Reviewer selection for multi-author phases: exclude the majority author (agent with most tasks). Ties broken by priority (Claude > Gemini > Codex)
- If chosen reviewer's circuit breaker is tripped: fall back to next eligible non-author agent. If all non-author agents are down, fail the phase with a clear error
- Same reviewer used across all retry attempts for a given phase -- consistency over fresh perspective
- Typestate pattern: each state is a distinct Rust type (Phase<Pending>, Phase<Running>, etc.). Invalid transitions are compile-time errors
- States: Pending -> Running -> AwaitingReview -> Complete OR ReviewFailed -> Retrying -> Running (loop)
- Retrying state carries attempt_number inside the state machine. Machine itself knows when max (3) is reached and transitions to ReviewFailed automatically
- ReviewFailed after max attempts is terminal and final -- the entire run stops. User must fix and re-run
- Full re-run with feedback: re-execute all tasks in the phase from scratch, with reviewer's feedback injected into the prompt
- Feedback injection: structured "Previous Review Feedback" section added to agent prompt with reviewer's reason + code suggestions. Agent sees original task + what went wrong (most recent attempt only, not full history)
- Always use all 3 attempts -- no early bail on repeated failures
- Same reviewer across all attempts for continuity
- Structured JSON response: agent returns JSON with files_produced (path + content), explanation, issues_encountered. Uses json_schema field on AgentRequest
- Files written to disk only after review passes: hold all task outputs in memory until phase review passes, then write files + git commit atomically. No partial writes on review failure
- Reviewer receives full context: original task spec, all produced files with contents, and agent's explanation of what it did
- No pre-validation of acceptance criteria before review -- reviewer handles all quality checks as part of the review

### Claude's Discretion
- Exact typestate generic implementation approach
- Review prompt template design
- AgentCoordinator internal structure
- How PhaseRecord is populated during execution
- Error message formatting for ReviewFailed terminal state

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| QUAL-01 | Each phase output is cross-reviewed by a different AI agent (not the author) | ReviewEngine with never-same-as-author pairing, majority-author exclusion, circuit-breaker-aware fallback |
| QUAL-02 | Review gate blocks phase progression until review passes | Typestate machine: AwaitingReview can only transition to Complete (on pass) or ReviewFailed/Retrying (on fail) -- no bypass path exists at the type level |
| QUAL-03 | On review failure, Athena auto-retries with reviewer feedback (max 3 attempts) | Retry loop with attempt_number in Retrying state, feedback injection into prompts, convergence guard at attempt 3 |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ath-types | workspace | ReviewVerdict, PhaseRecord, AgentRequest/Response, PhaseSpec, TaskSpec | Already defined, all the audit and schema types needed |
| ath-agents | workspace | AgentBackend trait, MockBackend, CircuitBreaker, AgentError | Dispatch interface and test doubles already exist |
| ath-git | workspace | AsyncGitLayer for atomic commit after review passes | Async git commit via spawn_blocking |
| serde/serde_json | 1.0 | Structured JSON parsing of agent task output and review verdicts | Already in workspace deps |
| tokio | 1 | Async runtime for agent dispatch | Already in workspace deps |
| async-trait | 0.1 | Async trait methods | Already in workspace deps |
| thiserror | 2.0 | Domain error types for PhaseRunner errors | Project pattern: thiserror for internal errors |
| chrono | 0.4 | Timestamps for PhaseRecord and ReviewAttempt | Already in workspace deps |
| uuid | 1.8 | Request IDs for AgentRequest | Already in workspace deps |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::marker::PhantomData | stdlib | Zero-cost type-level state markers | Typestate pattern for phase states |
| std::mem::discriminant | stdlib | Compare AgentKind variants without matching model strings | Reviewer != author check |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| PhantomData typestate | statum crate | Adds macro dependency for limited gain in a 6-state machine -- hand-roll is clearer |
| Manual retry loop | backon crate | backon is already in workspace but doesn't support feedback injection between retries -- custom loop needed |
| Enum wrapper for external API | Pure typestate everywhere | Typestate can't be stored in collections; need an enum PhaseStatus for serialization/logging alongside the typestate for transitions |

**No new dependencies required.** Everything needed is already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/ath-orchestrator/src/
  lib.rs              # add: pub mod phase_runner; pub mod review; pub mod coordinator;
  phase_runner.rs     # PhaseState typestate machine + PhaseRunner orchestration
  review.rs           # ReviewEngine: pairing, prompt, verdict extraction
  coordinator.rs      # AgentCoordinator: sequential phase dispatch
  error.rs            # extend with PhaseRunnerError variants
  router.rs           # existing -- unchanged
  isolation.rs        # existing -- unchanged
  taxonomy.rs         # existing -- unchanged
```

### Pattern 1: Typestate State Machine
**What:** Each phase state is a zero-sized marker struct used as a generic parameter. Transitions consume `self` and return the new state type. Invalid transitions simply do not have method implementations.
**When to use:** The PhaseRunner state machine -- all 6 states.
**Example:**
```rust
use std::marker::PhantomData;

// State markers (zero-sized)
pub struct Pending;
pub struct Running;
pub struct AwaitingReview;
pub struct Complete;
pub struct ReviewFailed;
pub struct Retrying;

// The state machine container
pub struct PhaseState<S> {
    phase_id: u32,
    phase_name: String,
    _state: PhantomData<S>,
    // State-specific data stored in typed wrappers
}

// Only Pending can transition to Running
impl PhaseState<Pending> {
    pub fn start(self) -> PhaseState<Running> {
        PhaseState {
            phase_id: self.phase_id,
            phase_name: self.phase_name,
            _state: PhantomData,
        }
    }
}

// Only Running can transition to AwaitingReview
impl PhaseState<Running> {
    pub fn submit_for_review(self, outputs: TaskOutputs) -> PhaseState<AwaitingReview> {
        // ...
    }
}

// AwaitingReview can go to Complete or needs retry handling
impl PhaseState<AwaitingReview> {
    pub fn approve(self) -> PhaseState<Complete> { /* ... */ }
    pub fn reject(self, attempt: u32, max: u32) -> RejectOutcome { /* ... */ }
}

pub enum RejectOutcome {
    Retry(PhaseState<Retrying>),
    Failed(PhaseState<ReviewFailed>),
}
```

### Pattern 2: Dual Representation (Typestate + Enum)
**What:** Typestate for the transition logic (compile-time safety), plus a serializable enum for logging, persistence, and PhaseRecord status tracking.
**When to use:** Whenever you need to store/log/display current state. The PhaseRecord and any external reporting needs a simple enum, not generic types.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseStatus {
    Pending,
    Running,
    AwaitingReview,
    Complete,
    ReviewFailed { attempts: u32, final_reason: String },
    Retrying { attempt_number: u32 },
}
```

### Pattern 3: Agent Registry
**What:** A HashMap or struct mapping AgentKind discriminants to `Arc<dyn AgentBackend>` instances. The coordinator looks up the backend for each task's assigned_agent.
**When to use:** AgentCoordinator needs to dispatch to the right backend.
**Example:**
```rust
pub struct AgentRegistry {
    backends: HashMap<std::mem::Discriminant<AgentKind>, Arc<dyn AgentBackend>>,
}

impl AgentRegistry {
    pub fn get(&self, agent: &AgentKind) -> Option<&Arc<dyn AgentBackend>> {
        self.backends.get(&std::mem::discriminant(agent))
    }
}
```

### Pattern 4: In-Memory Output Buffer
**What:** All task outputs are collected in a Vec during phase execution, held in memory until review passes, then written atomically to disk + git committed.
**When to use:** Between Running -> AwaitingReview -> Complete. On review failure, the buffer is discarded and tasks re-run from scratch.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    pub task_name: String,
    pub agent: AgentKind,
    pub files_produced: Vec<FileOutput>,
    pub explanation: String,
    pub issues_encountered: Vec<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutput {
    pub path: String,
    pub content: String,
}
```

### Anti-Patterns to Avoid
- **Storing PhaseState<S> in a collection:** Generic types with different S can't go in the same Vec. Use the enum PhaseStatus for collections; use typestate only for the active execution path.
- **Cloning PhaseState for retry:** Typestate transitions consume self. Don't try to clone before transitioning -- the retry path creates a new Running state from Retrying.
- **Reviewing with the same agent:** The discriminant check must happen at reviewer selection time, not after. Never construct a review request without first verifying agent != author.
- **Writing files before review:** The entire point of the in-memory buffer is that failed reviews leave zero disk artifacts. Do not write "draft" files.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Agent dispatch | Custom HTTP clients | `AgentBackend::send()` via ath-agents | Already handles retries, circuit breaking, structured output |
| Git commits | Direct git2 calls | `AsyncGitLayer::commit_phase_async()` | Already handles spawn_blocking, staging, metadata |
| JSON schema validation | Custom validators | `json_schema` field on `AgentRequest` | Provider-native JSON mode already supported |
| Agent routing | Manual agent selection | `assign_all_tasks()` from router.rs | Majority vote + fallback + rationale already done |
| Discriminant comparison | Pattern matching all variants | `std::mem::discriminant()` | Already used in router.rs, project pattern |

**Key insight:** The existing codebase already has the dispatch, routing, git, and type infrastructure. Phase 7 is about connecting these pieces with the state machine, review logic, and retry loop.

## Common Pitfalls

### Pitfall 1: Typestate Ergonomics Collapse
**What goes wrong:** Trying to make the typestate work everywhere leads to massive generic proliferation. Functions that accept "any phase state" require trait bounds on every method.
**Why it happens:** Typestate is great for enforcing transitions but awkward for passing state around generically.
**How to avoid:** Use typestate only in the PhaseRunner's internal execution loop. External interfaces (AgentCoordinator, logging, PhaseRecord) use the PhaseStatus enum. The typestate drives the state machine; the enum represents it.
**Warning signs:** Generic bounds appearing on more than 2 functions. Functions needing `where S: SomeStateTrait`.

### Pitfall 2: Reviewer Selection Race with Circuit Breaker
**What goes wrong:** Reviewer is selected when phase starts, but by the time review happens (after all tasks run), the reviewer's circuit breaker may have tripped.
**Why it happens:** Tasks can take significant time; provider state changes during execution.
**How to avoid:** Check `is_available()` at review dispatch time, not at phase start. If tripped, fall back to next eligible agent per the priority chain. Lock in the reviewer for retries only after the first successful review dispatch.
**Warning signs:** "Circuit breaker open" errors during review that weren't checked before sending.

### Pitfall 3: JSON Schema Mismatch
**What goes wrong:** Agent returns JSON that doesn't match the expected TaskOutput schema -- missing fields, wrong types, extra nesting.
**Why it happens:** Different LLM providers handle json_schema differently. Some are strict, others approximate.
**How to avoid:** Define a robust JSON schema with clear field names. Use `serde_json::from_str` with good error messages. Have a fallback: if structured output fails, try to extract from raw content. Log the raw response on parse failure for debugging.
**Warning signs:** Deserialization panics in tests. Provider returns wrapped JSON (e.g., `{"result": {actual_data}}`).

### Pitfall 4: Retry Attempt Counting Off-By-One
**What goes wrong:** First attempt counted as "retry 1" instead of "attempt 1", leading to only 2 actual tries before hitting the max.
**Why it happens:** Ambiguity between "attempt number" and "retry number".
**How to avoid:** Use 1-based attempt numbering. Attempt 1 = first try (not a retry). Attempt 2 = first retry. Attempt 3 = second retry = max. The Retrying state carries the next attempt number (2 or 3). ReviewFailed is reached when attempt 3 fails.
**Warning signs:** Tests passing with 2 attempts instead of 3. Off-by-one in the convergence guard.

### Pitfall 5: Feedback Injection Bloating Prompts
**What goes wrong:** Including too much review feedback makes the agent prompt exceed context limits or dilute the original task.
**Why it happens:** ReviewVerdict can contain verbose reasons and multiple suggestions.
**How to avoid:** Cap feedback injection to the most recent attempt only (already decided). Limit the feedback section to: reason (first 500 chars), up to 5 suggestions, and a brief "what to fix" summary. Place feedback section clearly separated from the original task prompt.
**Warning signs:** Token usage doubling on retry attempts. Agent output degrading on retries.

### Pitfall 6: Atomic Write Failure Leaves Partial State
**What goes wrong:** Writing multiple files succeeds for some but fails for others, leaving inconsistent state.
**Why it happens:** Filesystem operations can fail mid-batch (permissions, disk space).
**How to avoid:** Write all files first, then stage + commit atomically via AsyncGitLayer. If any file write fails, don't commit. Consider writing to a temp directory first, then moving. Git commit is the atomicity boundary.
**Warning signs:** Tests that mock file writes but don't test partial failure cases.

## Code Examples

### Structured Task Output JSON Schema
```rust
// The JSON schema to pass via AgentRequest.json_schema for task execution
pub fn task_output_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "files_produced": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            },
            "explanation": { "type": "string" },
            "issues_encountered": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["files_produced", "explanation", "issues_encountered"]
    })
}
```

### Review Verdict JSON Schema
```rust
// The JSON schema for review verdict extraction
pub fn review_verdict_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "passed": { "type": "boolean" },
            "severity": { "type": "string", "enum": ["critical", "warning", "info"] },
            "reason": { "type": "string" },
            "suggestions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string" },
                        "line": { "type": ["integer", "null"] },
                        "suggestion": { "type": "string" }
                    },
                    "required": ["file", "suggestion"]
                }
            }
        },
        "required": ["passed", "severity", "reason", "suggestions"]
    })
}
```

### Reviewer Selection Logic
```rust
// Exclude majority author, use priority tiebreaking
pub fn select_reviewer(
    task_agents: &[AgentKind],
    available: impl Fn(&AgentKind) -> bool,
) -> Result<AgentKind, PhaseRunnerError> {
    // Count tasks per agent discriminant
    let mut counts: HashMap<std::mem::Discriminant<AgentKind>, (AgentKind, usize)> = HashMap::new();
    for agent in task_agents {
        let disc = std::mem::discriminant(agent);
        counts.entry(disc)
            .and_modify(|(_, c)| *c += 1)
            .or_insert((agent.clone(), 1));
    }

    // Find majority author
    let majority = counts.values()
        .max_by_key(|(_, count)| *count)
        .map(|(agent, _)| std::mem::discriminant(agent));

    // All three agent kinds as candidates, excluding majority author
    let all_kinds = [
        AgentKind::Claude("opus-4".into()),
        AgentKind::Gemini("2.5-pro".into()),
        AgentKind::Codex("o3".into()),
    ];

    for candidate in &all_kinds {
        let disc = std::mem::discriminant(candidate);
        if Some(disc) == majority {
            continue; // skip majority author
        }
        if available(candidate) {
            return Ok(candidate.clone());
        }
    }

    Err(PhaseRunnerError::NoReviewerAvailable { /* ... */ })
}
```

### Feedback Injection into Prompt
```rust
fn build_retry_prompt(original_task: &TaskSpec, feedback: &ReviewVerdict) -> String {
    let mut prompt = format!(
        "## Task\n{}\n\n## Description\n{}\n",
        original_task.name, original_task.description
    );

    prompt.push_str("\n## Previous Review Feedback\n");
    prompt.push_str(&format!("**Result:** FAILED ({})\n", feedback.severity));
    prompt.push_str(&format!("**Reason:** {}\n", feedback.reason));

    if !feedback.suggestions.is_empty() {
        prompt.push_str("\n**Suggestions:**\n");
        for s in &feedback.suggestions {
            if let Some(line) = s.line {
                prompt.push_str(&format!("- `{}` line {}: {}\n", s.file, line, s.suggestion));
            } else {
                prompt.push_str(&format!("- `{}`: {}\n", s.file, s.suggestion));
            }
        }
    }

    prompt.push_str("\nPlease address the feedback above and produce corrected output.\n");
    prompt
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Enum + match state machine | Typestate with PhantomData | Stable Rust pattern since ~2020 | Invalid transitions are compile errors, not runtime panics |
| Single-agent review | Cross-agent review pairing | Project decision (Phase 7) | Different model perspective catches blind spots |
| Immediate file writes | Buffer-then-write-atomically | Project decision (Phase 7) | Failed reviews leave zero disk artifacts |

**Deprecated/outdated:**
- None applicable -- this is new construction, not migration

## Open Questions

1. **Review Prompt Template Effectiveness**
   - What we know: The reviewer needs original task spec, produced files, and agent explanation
   - What's unclear: Optimal prompt structure for getting reliable structured verdicts from different providers
   - Recommendation: Start with a straightforward template, iterate based on integration test results. The json_schema field should constrain output format.

2. **Large File Content in Review Context**
   - What we know: Reviewer receives full file contents from all tasks
   - What's unclear: Token limits if a phase produces many large files
   - Recommendation: For v1, pass full content. If token limits are hit, truncate to first N lines per file with a note. This is an edge case for the synthetic test project.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace test settings |
| Quick run command | `cargo test -p ath-orchestrator --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| QUAL-01 | Cross-agent review: reviewer != author | unit | `cargo test -p ath-orchestrator review::tests::reviewer_excludes_author -x` | No -- Wave 0 |
| QUAL-01 | Majority author excluded from review | unit | `cargo test -p ath-orchestrator review::tests::majority_author_excluded -x` | No -- Wave 0 |
| QUAL-01 | Circuit breaker fallback for reviewer | unit | `cargo test -p ath-orchestrator review::tests::reviewer_fallback_on_circuit_break -x` | No -- Wave 0 |
| QUAL-02 | Review gate blocks progression | unit | `cargo test -p ath-orchestrator phase_runner::tests::review_gate_blocks -x` | No -- Wave 0 |
| QUAL-02 | Typestate prevents invalid transitions | unit (compile-fail) | `cargo test -p ath-orchestrator phase_runner::tests::valid_transitions -x` | No -- Wave 0 |
| QUAL-03 | Retry with feedback injection | unit | `cargo test -p ath-orchestrator phase_runner::tests::retry_injects_feedback -x` | No -- Wave 0 |
| QUAL-03 | Max 3 attempts then ReviewFailed | unit | `cargo test -p ath-orchestrator phase_runner::tests::max_three_attempts -x` | No -- Wave 0 |
| QUAL-03 | Full pipeline happy path | integration | `cargo test -p ath-orchestrator coordinator::tests::full_pipeline_passes -x` | No -- Wave 0 |
| QUAL-03 | Full pipeline with retry + pass | integration | `cargo test -p ath-orchestrator coordinator::tests::pipeline_retry_then_pass -x` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-orchestrator --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/ath-orchestrator/src/phase_runner.rs` -- typestate machine + tests
- [ ] `crates/ath-orchestrator/src/review.rs` -- ReviewEngine + tests
- [ ] `crates/ath-orchestrator/src/coordinator.rs` -- AgentCoordinator + tests
- [ ] Extend `crates/ath-orchestrator/src/error.rs` -- PhaseRunnerError variants
- [ ] Add `tokio`, `async-trait`, `chrono`, `uuid`, `ath-git` to ath-orchestrator Cargo.toml dependencies

## Sources

### Primary (HIGH confidence)
- Codebase inspection: ath-types/src/review.rs, phase.rs, agent.rs, plan.rs -- all schema types verified
- Codebase inspection: ath-agents/src/backend.rs, mock.rs, circuit_breaker.rs, error.rs -- dispatch interface verified
- Codebase inspection: ath-orchestrator/src/router.rs, isolation.rs, taxonomy.rs -- routing patterns verified
- Codebase inspection: ath-git/src/async_ops.rs -- async commit interface verified
- Codebase inspection: Cargo.toml workspace dependencies -- all needed libs already present

### Secondary (MEDIUM confidence)
- [Cliffle: The Typestate Pattern in Rust](https://cliffle.com/blog/rust-typestate/) -- canonical reference for PhantomData typestate
- [Embedded Rust Book: Typestate Programming](https://docs.rust-embedded.org/book/static-guarantees/typestate-programming.html) -- official embedded Rust guide on typestate
- [Hoverbear: Pretty State Machine Patterns in Rust](https://hoverbear.org/blog/rust-state-machine-pattern/) -- practical enum vs typestate comparison
- [OneUpTime: Rust State Machines (2026-02)](https://oneuptime.com/blog/post/2026-02-01-rust-state-machines/view) -- recent guide on typestate + enum dual representation

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, all types already defined, no new dependencies
- Architecture: HIGH -- typestate pattern is well-documented in Rust, codebase patterns (discriminant comparison, MockBackend, error handling) are established
- Pitfalls: HIGH -- identified from codebase inspection (circuit breaker timing, JSON schema handling) and Rust typestate literature (ergonomics, collection storage)

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- no external dependencies changing)
