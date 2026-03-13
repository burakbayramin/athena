# Phase 6: Module Isolation - Research

**Researched:** 2026-03-13
**Domain:** Multi-agent task routing, file ownership isolation, skill taxonomy
**Confidence:** HIGH

## Summary

Phase 6 adds three capabilities to the ath-orchestrator crate: (1) a static skill taxonomy that maps SkillTag values to preferred AgentKind, (2) an agent router that assigns each task to exactly one agent using majority-vote tiebreaking, and (3) an IsolationManager that validates file ownership is disjoint across parallel-eligible phases before dispatch. A post-execution audit step confirms actual output files match expectations.

The domain is entirely internal logic -- no external libraries, no network calls, no LLM interaction. All inputs (TaskSpec, ExecutionPlan, AgentKind, SkillTag) already exist in ath-types. The circuit breaker availability check exists in ath-agents. The work is pure Rust data structures and validation algorithms operating on the existing type system.

**Primary recommendation:** Implement as three internal modules within ath-orchestrator (taxonomy.rs, router.rs, isolation.rs) plus a thin public facade, keeping each concern testable in isolation with zero external dependencies beyond ath-types and ath-agents.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Static routing table: hardcoded map from SkillTag to preferred AgentKind
- Mapping: rust/architecture/logic -> Claude, docs/research/api -> Gemini, codegen/boilerplate -> Codex
- Multi-tag tiebreaker: majority vote across all skill tags on a task, ties broken by fixed priority (Claude > Gemini > Codex)
- Default fallback for unrecognized tags: Claude (strongest general-purpose model)
- Hardcoded for v1 -- no user-configurable routing table
- Routing rationale visible in --verbose mode only; default output shows agent assignment per task
- Hard block before dispatch: reject the plan with a clear IsolationError listing conflicting files, which tasks claim them, and a fix hint
- Ownership checked within phase AND across parallel phases (phases with no dependency that could run concurrently must have disjoint file sets)
- Exact file path matching only -- no directory-level overlap detection
- Post-execution audit: compare actual vs declared output files, log unexpected files as warnings, don't block
- No strong match: silently default to Claude
- Best-match agent unavailable (circuit breaker tripped): fallback to next-best agent from routing table
- ALL agents unavailable: fail with clear error listing which providers are down
- Per-task assignment: each task within a phase can go to a different agent based on its skill tags
- Sequential overlap within phase allowed: later tasks can modify files produced by earlier tasks in the same phase
- Cross-phase conflict checking only applies to parallel-eligible phases
- Agent assignment stored on TaskSpec: add `assigned_agent: Option<AgentKind>` field -- None before routing, Some after
- IsolationManager lives in ath-orchestrator crate (not ath-planner)

### Claude's Discretion
- Internal module structure within ath-orchestrator
- Exact routing table entries for edge-case skill tags
- IsolationError variant design and error message formatting
- Post-execution audit implementation details
- How to collect actual output files for the audit step

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ORCH-01 | Athena assigns named agent roles (Claude: architecture/logic, Gemini: research/APIs, Codex: code generation) | Skill taxonomy module maps SkillTag strings to AgentKind variants using static routing table |
| ORCH-02 | Athena routes tasks to agents based on a skill taxonomy matching task requirements to model strengths | Agent router reads task.skill_tags, queries taxonomy, applies majority-vote + priority tiebreak, sets assigned_agent |
| ORCH-03 | Each agent operates on isolated modules with strict file ownership -- no shared edits | IsolationManager builds file->task ownership map per parallel group, blocks dispatch on overlap, post-audit confirms disjointness |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ath-types | workspace | TaskSpec, AgentKind, SkillTag, ExecutionPlan, ValidationError | All input/output types already defined here |
| ath-agents | workspace | AgentBackend::is_available(), CircuitBreaker | Needed for routing fallback when provider is down |
| thiserror | 2.0 | IsolationError definition | Project standard for domain errors |
| serde | 1.0 | Derive Serialize/Deserialize on new types | Project standard -- all types serializable |
| std::collections | stdlib | HashMap, HashSet for routing table and file ownership | No external deps needed for this logic |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing/log | -- | Verbose routing rationale output | If project adds logging; for now use simple return of rationale strings |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Static HashMap routing table | Trait-based strategy pattern | Over-engineering for v1 hardcoded table; static is simpler and matches decision |
| String-based SkillTag matching | Enum-based skill tags | SkillTag is already a String newtype; matching on inner string is sufficient for v1 |

**Installation:**
No new dependencies required. ath-orchestrator already depends on ath-types. Add ath-agents dependency to ath-orchestrator/Cargo.toml for circuit breaker availability checks.

```toml
# crates/ath-orchestrator/Cargo.toml additions
[dependencies]
ath-types = { path = "../ath-types" }
ath-agents = { path = "../ath-agents" }
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
```

## Architecture Patterns

### Recommended Module Structure
```
crates/ath-orchestrator/src/
  lib.rs            # Public API: re-exports
  taxonomy.rs       # SkillTag -> AgentKind routing table
  router.rs         # Task-to-agent assignment logic (uses taxonomy + circuit breaker)
  isolation.rs      # File ownership validation + post-execution audit
  error.rs          # IsolationError variants
```

### Pattern 1: Static Routing Table
**What:** A function or lazy-initialized HashMap mapping lowercase skill tag strings to AgentKind variants.
**When to use:** Every time a task needs agent assignment.
**Example:**
```rust
use std::collections::HashMap;
use ath_types::{AgentKind, SkillTag};

/// Fixed priority order for tiebreaking: Claude > Gemini > Codex
const PRIORITY: &[fn(String) -> AgentKind] = &[
    |m| AgentKind::Claude(m),
    |m| AgentKind::Gemini(m),
    |m| AgentKind::Codex(m),
];

pub fn build_routing_table() -> HashMap<String, AgentKind> {
    let mut table = HashMap::new();
    // Claude: architecture, logic, rust
    for tag in ["rust", "architecture", "logic", "systems", "design"] {
        table.insert(tag.to_string(), AgentKind::Claude("opus-4".into()));
    }
    // Gemini: research, docs, api
    for tag in ["docs", "research", "api", "documentation", "analysis"] {
        table.insert(tag.to_string(), AgentKind::Gemini("2.5-pro".into()));
    }
    // Codex: codegen, boilerplate
    for tag in ["codegen", "boilerplate", "scaffolding", "template", "generation"] {
        table.insert(tag.to_string(), AgentKind::Codex("o3".into()));
    }
    table
}
```

### Pattern 2: Majority Vote with Priority Tiebreak
**What:** Count which AgentKind each skill tag maps to, pick the one with the most votes. On tie, use fixed priority (Claude > Gemini > Codex).
**When to use:** Agent assignment for a single task.
**Example:**
```rust
pub struct RoutingDecision {
    pub agent: AgentKind,
    pub rationale: String,  // Human-readable explanation for --verbose
}

pub fn route_task(
    skill_tags: &[SkillTag],
    table: &HashMap<String, AgentKind>,
    available: impl Fn(&AgentKind) -> bool,
) -> Result<RoutingDecision, IsolationError> {
    // 1. Count votes per agent kind (discriminant only)
    // 2. Sort by (vote_count desc, priority asc)
    // 3. Pick first that passes available() check
    // 4. If none available, return AllAgentsUnavailable error
}
```

### Pattern 3: File Ownership Registry
**What:** HashMap<String, (phase_id, task_name)> tracking which task claims each file. Built per parallel group from ExecutionPlan::parallel_groups.
**When to use:** Pre-dispatch validation of an entire execution plan.
**Example:**
```rust
pub struct FileConflict {
    pub file: String,
    pub task_a: String,
    pub phase_a: u32,
    pub task_b: String,
    pub phase_b: u32,
}

pub fn check_isolation(plan: &ExecutionPlan) -> Result<(), IsolationError> {
    for group in &plan.parallel_groups {
        let mut ownership: HashMap<String, (u32, String)> = HashMap::new();
        for phase_id in group {
            let phase = find_phase(plan, *phase_id);
            for task in &phase.tasks {
                for file in &task.expected_output_files {
                    if let Some((existing_phase, existing_task)) = ownership.get(file) {
                        return Err(IsolationError::FileConflict { /* ... */ });
                    }
                    ownership.insert(file.clone(), (*phase_id, task.name.clone()));
                }
            }
        }
    }
    Ok(())
}
```

### Pattern 4: Enrich-in-Place Assignment
**What:** Add `assigned_agent: Option<AgentKind>` to TaskSpec. Router mutates tasks in-place on the plan. Downstream phases (7, 8) read assignments directly.
**When to use:** After routing, before dispatch.
**Key consideration:** Adding a field to TaskSpec requires `#[serde(default)]` for backward compatibility with existing serialized plans that lack the field.

### Anti-Patterns to Avoid
- **Separate assignment map:** Don't maintain a separate HashMap<TaskName, AgentKind> alongside the plan -- enriching TaskSpec directly avoids sync bugs and is the decided approach.
- **Directory-level overlap detection:** Don't build directory hierarchy checking -- exact file path matching only per decision.
- **Dynamic/configurable routing:** Don't build config file parsing or user-facing routing customization -- hardcoded for v1.
- **Blocking on unavailable agents:** Don't add retry loops or wait-for-recovery logic -- fall back immediately to next-best agent.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Agent availability check | Custom health probing | ath-agents::AgentBackend::is_available() | Circuit breaker already tracks provider state |
| Error types with hints | Ad-hoc error strings | thiserror enum with hint field (existing pattern) | Matches ValidationError pattern from ath-types |
| Parallel group identification | Custom DAG analysis | ExecutionPlan::parallel_groups (already computed) | Phase 5 already computes which phases can run concurrently |

**Key insight:** Phase 5 already computed the parallel_groups. Phase 6 consumes that data for conflict checking -- no DAG analysis needed here.

## Common Pitfalls

### Pitfall 1: Forgetting serde(default) on new TaskSpec field
**What goes wrong:** Adding `assigned_agent: Option<AgentKind>` to TaskSpec breaks deserialization of existing JSON that lacks the field.
**Why it happens:** serde requires all fields present by default for struct deserialization.
**How to avoid:** Add `#[serde(default, skip_serializing_if = "Option::is_none")]` on the new field -- same pattern used for AgentRequest.json_schema.
**Warning signs:** Deserialization tests for existing TaskSpec JSON fail after field addition.

### Pitfall 2: Case-sensitive SkillTag matching
**What goes wrong:** SkillTag("Rust") doesn't match routing table entry "rust", causing unexpected Claude-default assignments.
**Why it happens:** SkillTag wraps a raw String; LLM-generated tags may have inconsistent casing.
**How to avoid:** Normalize to lowercase before routing table lookup: `tag.0.to_lowercase()`.
**Warning signs:** Tasks with obvious skill matches getting default-routed to Claude.

### Pitfall 3: Within-phase file overlap vs cross-phase
**What goes wrong:** Blocking tasks within the same phase from sharing files, when the decision explicitly allows sequential overlap within a phase.
**Why it happens:** Applying the same ownership check uniformly to all tasks regardless of phase boundaries.
**How to avoid:** Only check file disjointness across different phases within the same parallel group. Within a single phase, tasks are sequential and CAN share files.
**Warning signs:** Valid plans rejected because Task 2 modifies a file Task 1 created in the same phase.

### Pitfall 4: AgentKind equality comparison with model strings
**What goes wrong:** `AgentKind::Claude("opus-4") != AgentKind::Claude("sonnet-4")` when counting votes by agent "kind."
**Why it happens:** AgentKind variants carry model strings; PartialEq compares the full value.
**How to avoid:** Compare by discriminant only when counting votes. Use `std::mem::discriminant()` or match on variant ignoring the inner string.
**Warning signs:** Vote counts fragmented across model variants of the same provider.

### Pitfall 5: Empty skill_tags on a task
**What goes wrong:** Task has no skill tags, majority vote has zero votes, no clear routing.
**Why it happens:** LLM-generated plans may omit skill tags on simple tasks.
**How to avoid:** Default to Claude when skill_tags is empty (matches "no strong match" decision).
**Warning signs:** Panic or unexpected behavior on division-by-zero in vote counting.

## Code Examples

### Adding assigned_agent to TaskSpec
```rust
// In crates/ath-types/src/plan.rs
use crate::agent::AgentKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSpec {
    pub name: String,
    pub description: String,
    pub skill_tags: Vec<SkillTag>,
    pub expected_output_files: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub goal_indices: Vec<usize>,
    /// Agent assigned by the router. None before routing, Some after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<AgentKind>,
}
```

### IsolationError design (following ValidationError pattern)
```rust
// In crates/ath-orchestrator/src/error.rs
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum IsolationError {
    #[error("File conflict: '{file}' claimed by task '{task_a}' (phase {phase_a}) and task '{task_b}' (phase {phase_b})")]
    FileConflict {
        file: String,
        task_a: String,
        phase_a: u32,
        task_b: String,
        phase_b: u32,
        hint: String,
    },

    #[error("All agents unavailable: {providers}")]
    AllAgentsUnavailable {
        providers: String,
        hint: String,
    },

    #[error("No agent assigned to task '{task_name}'")]
    UnassignedTask {
        task_name: String,
        hint: String,
    },
}

impl IsolationError {
    pub fn hint(&self) -> &str {
        match self {
            IsolationError::FileConflict { hint, .. }
            | IsolationError::AllAgentsUnavailable { hint, .. }
            | IsolationError::UnassignedTask { hint, .. } => hint,
        }
    }
}
```

### Discriminant-based vote counting
```rust
use std::mem::discriminant;

fn count_votes(tags: &[SkillTag], table: &HashMap<String, AgentKind>) -> Vec<(AgentKind, usize)> {
    let mut counts: HashMap<std::mem::Discriminant<AgentKind>, (AgentKind, usize)> = HashMap::new();
    for tag in tags {
        if let Some(agent) = table.get(&tag.0.to_lowercase()) {
            let disc = discriminant(agent);
            counts.entry(disc)
                .and_modify(|(_, count)| *count += 1)
                .or_insert((agent.clone(), 1));
        }
    }
    let mut results: Vec<_> = counts.into_values().collect();
    // Sort by count desc, then by priority (Claude=0, Gemini=1, Codex=2)
    results.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| priority(a.0).cmp(&priority(b.0))));
    results
}

fn priority(agent: &AgentKind) -> u8 {
    match agent {
        AgentKind::Claude(_) => 0,
        AgentKind::Gemini(_) => 1,
        AgentKind::Codex(_) => 2,
    }
}
```

### Post-execution audit
```rust
pub struct AuditWarning {
    pub task_name: String,
    pub unexpected_files: Vec<String>,
    pub missing_files: Vec<String>,
}

pub fn audit_outputs(
    task: &TaskSpec,
    actual_files: &[String],
) -> Vec<AuditWarning> {
    let expected: HashSet<_> = task.expected_output_files.iter().collect();
    let actual: HashSet<_> = actual_files.iter().collect();

    let unexpected: Vec<_> = actual.difference(&expected).map(|s| s.to_string()).collect();
    let missing: Vec<_> = expected.difference(&actual).map(|s| s.to_string()).collect();

    if unexpected.is_empty() && missing.is_empty() {
        vec![]
    } else {
        vec![AuditWarning {
            task_name: task.name.clone(),
            unexpected_files: unexpected,
            missing_files: missing,
        }]
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-agent execution | Multi-agent with skill routing | This phase | Enables agent specialization |
| No file conflict checks | Pre-dispatch isolation validation | This phase | Prevents merge conflicts in Phase 10 parallel execution |
| Manual agent selection | Automatic taxonomy-based routing | This phase | Removes human decision from per-task routing |

## Open Questions

1. **Model variant strings in routing table**
   - What we know: AgentKind carries model strings (e.g., "opus-4", "2.5-pro", "o3")
   - What's unclear: Where do default model strings come from? Should routing table use config-provided model strings?
   - Recommendation: Read default model strings from the existing config system (ath-config) or hardcode reasonable defaults. The routing table maps to provider kind; the specific model is a config concern.

2. **How to collect actual output files for post-audit**
   - What we know: Post-execution audit compares actual vs declared files
   - What's unclear: Phase 7 (PhaseRunner) doesn't exist yet -- how will actual files be reported back?
   - Recommendation: Define the audit function signature now (takes task + actual file list). Phase 7 will call it after execution. For now, the function is a pure utility that Phase 7 integrates.

3. **Circuit breaker access pattern**
   - What we know: CircuitBreaker lives in ath-agents, method is `can_attempt(&mut self)`
   - What's unclear: The AgentBackend trait has `is_available(&self)` -- router should use this trait method, not raw CircuitBreaker
   - Recommendation: Router accepts `&dyn AgentBackend` references (or a trait for availability checks) rather than directly touching CircuitBreaker. This keeps the abstraction clean.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (#[test], #[cfg(test)]) |
| Config file | None needed -- Rust test framework is zero-config |
| Quick run command | `cargo test -p ath-orchestrator` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ORCH-01 | Routing table maps skill tags to correct agent kinds | unit | `cargo test -p ath-orchestrator -- taxonomy` | No -- Wave 0 |
| ORCH-01 | Default fallback routes to Claude for unknown tags | unit | `cargo test -p ath-orchestrator -- taxonomy::fallback` | No -- Wave 0 |
| ORCH-02 | Majority vote picks agent with most tag matches | unit | `cargo test -p ath-orchestrator -- router::majority` | No -- Wave 0 |
| ORCH-02 | Tie broken by fixed priority Claude > Gemini > Codex | unit | `cargo test -p ath-orchestrator -- router::tiebreak` | No -- Wave 0 |
| ORCH-02 | Unavailable agent falls back to next-best | unit | `cargo test -p ath-orchestrator -- router::fallback` | No -- Wave 0 |
| ORCH-02 | All agents unavailable returns AllAgentsUnavailable error | unit | `cargo test -p ath-orchestrator -- router::all_unavailable` | No -- Wave 0 |
| ORCH-02 | Routing rationale string generated for verbose output | unit | `cargo test -p ath-orchestrator -- router::rationale` | No -- Wave 0 |
| ORCH-03 | Parallel-phase file overlap detected and blocked | unit | `cargo test -p ath-orchestrator -- isolation::conflict` | No -- Wave 0 |
| ORCH-03 | Same-phase sequential tasks allowed to share files | unit | `cargo test -p ath-orchestrator -- isolation::sequential_ok` | No -- Wave 0 |
| ORCH-03 | Post-audit detects unexpected and missing files | unit | `cargo test -p ath-orchestrator -- isolation::audit` | No -- Wave 0 |
| ORCH-03 | Clean plan passes isolation check | unit | `cargo test -p ath-orchestrator -- isolation::clean_pass` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-orchestrator`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/ath-orchestrator/src/taxonomy.rs` -- routing table module with tests
- [ ] `crates/ath-orchestrator/src/router.rs` -- agent router with tests
- [ ] `crates/ath-orchestrator/src/isolation.rs` -- file ownership + audit with tests
- [ ] `crates/ath-orchestrator/src/error.rs` -- IsolationError type
- [ ] Update `crates/ath-types/src/plan.rs` -- add assigned_agent field to TaskSpec
- [ ] Update `crates/ath-orchestrator/Cargo.toml` -- add ath-agents, thiserror, serde deps

## Sources

### Primary (HIGH confidence)
- Direct source code inspection of ath-types/src/plan.rs -- TaskSpec, ExecutionPlan, PhaseSpec structure
- Direct source code inspection of ath-types/src/agent.rs -- AgentKind enum with model strings
- Direct source code inspection of ath-types/src/project.rs -- SkillTag newtype
- Direct source code inspection of ath-types/src/error.rs -- ValidationError pattern with hint()
- Direct source code inspection of ath-agents/src/backend.rs -- AgentBackend trait with is_available()
- Direct source code inspection of ath-agents/src/circuit_breaker.rs -- CircuitBreaker API
- 06-CONTEXT.md -- all implementation decisions locked by user

### Secondary (MEDIUM confidence)
- Rust std::mem::discriminant() for variant-only comparison -- standard library, stable API

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no external dependencies; all types already exist in codebase
- Architecture: HIGH -- decisions are locked; module structure follows existing patterns
- Pitfalls: HIGH -- identified from direct code inspection (serde compat, case sensitivity, discriminant equality)

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- internal logic with no external dependency version concerns)