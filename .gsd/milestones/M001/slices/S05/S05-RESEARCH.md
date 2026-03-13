# Phase 5: Phase Decomposition - Research

**Researched:** 2026-03-12
**Domain:** DAG-based project decomposition via LLM with structural validation (Rust)
**Confidence:** HIGH

## Summary

Phase 5 builds the core value of Athena: taking a `ProjectSpec` and decomposing it into an ordered, dependency-validated phase plan. The implementation follows the proven pattern from Phase 4 (LLM call with structured output, retry-with-feedback on validation failure) but adds DAG construction, topological sorting, cycle detection, produces/consumes contract checking, and critical path computation.

The codebase already provides all integration points: `ProjectSpec` as input, `AgentBackend` trait for LLM calls, `MockBackend` for testing, `json_schema` field on `AgentRequest` for structured output enforcement, and the `parse_to_project_spec` retry loop as a template. New types (`ExecutionPlan`, `PhaseSpec`, `TaskSpec`) go in `ath-types`, and the decomposition logic goes in a new `decompose` module in `ath-planner`.

**Primary recommendation:** Hand-roll Kahn's algorithm for topological sort (graph is 3-10 nodes, no need for petgraph dependency), follow the Phase 4 retry-with-validation pattern exactly, and extend `ValidationError` with new DAG-specific variants for cycle detection, orphaned goals, missing dependency targets, and contract violations.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

- Pure LLM decomposition -- send ProjectSpec to Claude with a structured prompt, no heuristic pre-processing
- Soft bounds on phase count: prompt guides toward 3-10 phases, validation warns (doesn't block) outside that range
- LLM merges overlapping goals and clarifies vague goals -- the decomposition IS the clarification step
- Skill tags attached to each task (e.g., "rust", "api-design", "testing") -- actual agent assignment deferred to Phase 6
- Each task is an actionable spec: name, description, skill tags, expected output files, and acceptance criteria
- Target 2-5 tasks per phase -- enough to be focused without being monolithic
- Tasks within a phase are explicitly ordered (sequential execution, not parallel within phase)
- Each task maps back to the ProjectSpec GoalSpec(s) it fulfills -- enables traceability and orphan detection
- Phase-level parallelism only -- tasks within a phase are always sequential
- Dependency edges with numeric IDs: each phase has `depends_on: Vec<u32>` referencing other phase IDs
- Parallelism is implicit: phases with no mutual dependency relationship CAN run concurrently (Phase 10 handles actual concurrent execution)
- Critical path computed from the DAG -- longest dependency chain stored for progress estimation (used by Phase 8)
- Hard errors (block execution): circular dependencies, orphaned goals, missing dependency targets, empty phases
- Contract completeness: each phase declares what it produces and what it consumes from dependencies. Validation checks consumed items are actually produced by a listed dependency
- On validation failure: retry with feedback -- append specific validation errors to the LLM prompt, max 3 attempts (matches Phase 2/4 retry-with-context pattern)
- Warnings (non-blocking): unusually large phases, identical skill tag distributions, long critical paths

### Claude's Discretion

- Exact LLM prompt design for decomposition
- Internal module structure (new module in ath-planner)
- Specific type names and field organization for PhasePlan/TaskSpec
- Topological sort algorithm choice
- Warning threshold tuning (what counts as "unusually large")
- How to represent produces/consumes contracts (string labels vs typed enum)

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PLAN-01 | Athena decomposes project input into ordered phases with named tasks | LLM decomposition via `decompose_project_spec` function, structured JSON output enforcement via `json_schema`, retry-with-feedback pattern from Phase 4 |
| PLAN-02 | Athena automatically infers dependency DAG between phases and tasks | LLM generates `depends_on: Vec<u32>` per phase, Kahn's algorithm validates DAG structure, topological sort produces execution order |
| PLAN-03 | Athena identifies which phases can run in parallel vs must be sequential | Phases with no mutual dependency edges are flagged `parallel_eligible: true` -- computed from the adjacency list after topological sort |

</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde / serde_json | 1.0 | Serialize/deserialize plan types and LLM JSON output | Already in workspace, all types derive Serialize/Deserialize |
| thiserror | 2.0 | Domain error types for decomposition failures | Established pattern -- ValidationError, InputError both use it |
| tokio | 1 | Async runtime for LLM calls | Already in workspace |
| chrono | 0.4 | Timestamps on plan types | Already in workspace |
| uuid | 1.8 | Unique IDs for phases/tasks | Already in workspace |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| colored | 3 | Terminal output for dry-run display | Phase plan display and warnings |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled Kahn's algorithm | petgraph crate | petgraph is overkill for 3-10 node graphs; adds dependency for ~30 lines of code; hand-rolled gives full control over cycle path reporting |
| String labels for contracts | Typed enum | String labels are simpler, easier for LLM to generate, and sufficient for validation; typed enum adds complexity without clear benefit at this stage |

**Installation:** No new dependencies needed -- everything is already in `workspace.dependencies`.

## Architecture Patterns

### Recommended Module Structure

```
crates/ath-types/src/
  plan.rs              # ExecutionPlan, PhaseSpec, TaskSpec, Contract types
  error.rs             # Extended with DAG validation variants
  lib.rs               # Add pub mod plan; re-export new types

crates/ath-planner/src/
  decompose/
    mod.rs             # decompose_project_spec() - main entry point with retry loop
    prompt.rs          # System prompt, JSON schema, request builder for decomposition
    validate.rs        # DAG validation: cycles, orphans, contracts, empty phases
    dag.rs             # Topological sort, parallelism detection, critical path
    error.rs           # DecomposeError type
  lib.rs               # Add pub mod decompose;
```

### Pattern 1: Retry-with-Validation-Feedback (proven in Phase 4)

**What:** Send LLM request, parse response, validate, retry with error appended to prompt on failure.
**When to use:** Every LLM-structured-output call in this project.
**Example:**

```rust
// Follows parse_to_project_spec pattern exactly
pub async fn decompose_project_spec(
    project: &ProjectSpec,
    backend: &dyn AgentBackend,
) -> Result<ExecutionPlan, DecomposeError> {
    let mut last_error: Option<String> = None;

    for _attempt in 0..MAX_DECOMPOSE_ATTEMPTS {
        let request = build_decompose_request(project, last_error.as_deref());
        let response = backend.send(request).await.map_err(DecomposeError::Agent)?;

        match serde_json::from_str::<ExecutionPlan>(&response.content) {
            Ok(plan) => {
                match validate_plan(&plan, project) {
                    Ok(warnings) => {
                        // Log warnings but don't block
                        return Ok(plan);
                    }
                    Err(errors) => {
                        last_error = Some(format_validation_errors(&errors));
                    }
                }
            }
            Err(e) => {
                last_error = Some(format!("JSON parse error: {}", e));
            }
        }
    }

    Err(DecomposeError::DecomposeFailed {
        attempts: MAX_DECOMPOSE_ATTEMPTS,
        last_error,
    })
}
```

### Pattern 2: Kahn's Algorithm for Topological Sort with Cycle Detection

**What:** BFS-based topological sort that naturally detects cycles (if not all nodes are visited, a cycle exists).
**When to use:** After LLM generates the phase plan, before returning it.
**Example:**

```rust
use std::collections::{HashMap, VecDeque};

/// Topological sort via Kahn's algorithm.
/// Returns sorted phase IDs on success, or the cycle path on failure.
pub fn topological_sort(phases: &[PhaseSpec]) -> Result<Vec<u32>, Vec<u32>> {
    let mut in_degree: HashMap<u32, usize> = HashMap::new();
    let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();

    // Initialize
    for phase in phases {
        in_degree.entry(phase.id).or_insert(0);
        adjacency.entry(phase.id).or_insert_default();
        for &dep in &phase.depends_on {
            adjacency.entry(dep).or_insert_default().push(phase.id);
            *in_degree.entry(phase.id).or_insert(0) += 1;
        }
    }

    // Start with zero in-degree nodes
    let mut queue: VecDeque<u32> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut sorted = Vec::new();

    while let Some(node) = queue.pop_front() {
        sorted.push(node);
        if let Some(neighbors) = adjacency.get(&node) {
            for &neighbor in neighbors {
                let deg = in_degree.get_mut(&neighbor).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(neighbor);
                }
            }
        }
    }

    if sorted.len() != phases.len() {
        // Cycle exists -- find the cycle path via DFS
        let cycle = find_cycle_path(phases);
        Err(cycle)
    } else {
        Ok(sorted)
    }
}
```

### Pattern 3: Parallelism Detection from DAG

**What:** After topological sort, compute which phases can run concurrently by checking for absence of dependency relationship.
**When to use:** As a post-processing step on the sorted plan.
**Example:**

```rust
/// Compute parallel eligibility: a phase is parallel-eligible if it shares
/// no dependency chain with at least one other phase at the same "level".
pub fn compute_parallel_groups(phases: &[PhaseSpec], sorted_order: &[u32]) -> Vec<Vec<u32>> {
    // Assign each phase to the earliest "level" it can execute at
    // level = max(level of all dependencies) + 1
    let mut levels: HashMap<u32, usize> = HashMap::new();

    for &id in sorted_order {
        let phase = phases.iter().find(|p| p.id == id).unwrap();
        let level = if phase.depends_on.is_empty() {
            0
        } else {
            phase.depends_on.iter()
                .map(|dep| levels[dep] + 1)
                .max()
                .unwrap()
        };
        levels.insert(id, level);
    }

    // Group by level -- phases at the same level are parallel-eligible
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut groups = vec![Vec::new(); max_level + 1];
    for (&id, &level) in &levels {
        groups[level].push(id);
    }
    groups
}
```

### Pattern 4: Critical Path Computation

**What:** Longest path through the DAG (by phase count or estimated duration).
**When to use:** After topological sort, stored in `ExecutionPlan` for Phase 8 progress display.
**Example:**

```rust
/// Compute critical path length (longest dependency chain).
pub fn critical_path_length(phases: &[PhaseSpec], sorted_order: &[u32]) -> usize {
    let mut longest: HashMap<u32, usize> = HashMap::new();

    for &id in sorted_order {
        let phase = phases.iter().find(|p| p.id == id).unwrap();
        let path_len = if phase.depends_on.is_empty() {
            1
        } else {
            phase.depends_on.iter()
                .map(|dep| longest[dep])
                .max()
                .unwrap() + 1
        };
        longest.insert(id, path_len);
    }

    longest.values().copied().max().unwrap_or(0)
}
```

### Anti-Patterns to Avoid

- **Validating inside the type constructor:** Keep `validate()` as a separate method (established pattern). Don't put validation logic in `impl Default` or `new()`.
- **Using petgraph for 3-10 nodes:** Adds unnecessary dependency and obscures the algorithm. The entire topological sort + cycle detection is ~40 lines.
- **Mixing hard errors and warnings in the same return type:** Use `Result<Vec<Warning>, Vec<HardError>>` for validate, not a single list. Hard errors must block; warnings must not.
- **Putting graph algorithms in ath-types:** Types crate stays pure data. Graph logic belongs in ath-planner's `decompose/dag.rs`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON serialization | Custom parser | serde_json | Already proven across all phases |
| LLM structured output | Manual JSON extraction | `json_schema` on `AgentRequest` | Provider-level enforcement via genai's `JsonSpec` mode |
| UUID generation | Sequential IDs | uuid v4 | Collision-free, no coordination needed |
| Retry logic with feedback | Custom retry framework | Copy `parse_to_project_spec` pattern | Proven, tested, handles all edge cases |

**Key insight:** The retry-with-validation-feedback pattern is the project's core pattern for all LLM interactions. Do NOT create a new abstraction -- copy the shape exactly from `parse_to_project_spec`.

## Common Pitfalls

### Pitfall 1: LLM Generates Invalid Dependency IDs
**What goes wrong:** LLM outputs `depends_on: [5]` but there is no phase with `id: 5`.
**Why it happens:** LLM hallucination of IDs, especially with longer plans.
**How to avoid:** Validate all `depends_on` references exist in the phase list. Include this in the hard error set (MissingDependencyTarget).
**Warning signs:** Tests with MockBackend should include cases with dangling references.

### Pitfall 2: LLM Generates Circular Dependencies
**What goes wrong:** Phase A depends on Phase B, Phase B depends on Phase A (or longer cycles).
**Why it happens:** LLM reasoning about complex dependency chains can be inconsistent.
**How to avoid:** Kahn's algorithm naturally detects this. Report the cycle path in the error message fed back to the LLM on retry.
**Warning signs:** Topological sort visits fewer nodes than total phases.

### Pitfall 3: Orphaned Goals Not Caught
**What goes wrong:** A GoalSpec from the ProjectSpec has no task mapping to it, meaning the plan silently drops a requirement.
**Why it happens:** LLM focuses on "interesting" goals and forgets mundane ones.
**How to avoid:** Post-validation: for each GoalSpec in the input, verify at least one TaskSpec references it by index. Hard error if any goal is orphaned.
**Warning signs:** Mismatch between `project.goals.len()` and unique goal indices across all tasks.

### Pitfall 4: Contract Validation Misses Transitive Dependencies
**What goes wrong:** Phase C consumes "auth-types" which Phase A produces, but Phase C only depends on Phase B (not Phase A). Phase B happens to depend on Phase A, so transitively it works -- but the contract checker flags it as an error.
**Why it happens:** Strict "direct dependency" checking vs. transitive closure.
**How to avoid:** Check transitive dependency chain, not just direct `depends_on`. Compute the transitive closure of the dependency graph and validate produces/consumes against that.
**Warning signs:** Tests should include both direct and transitive contract satisfaction cases.

### Pitfall 5: Retry Feedback Prompt Grows Too Large
**What goes wrong:** Each retry appends the full error to the prompt, and with DAG-specific errors (cycle paths, multiple orphaned goals), the feedback becomes enormous.
**Why it happens:** Naive concatenation of all validation errors.
**How to avoid:** Summarize errors: "3 validation errors: circular dependency between phases 1->3->1, orphaned goals [2, 4], phase 5 has no tasks." Keep feedback under ~500 tokens.
**Warning signs:** Third retry attempt has a much larger prompt than the first.

### Pitfall 6: serde Deserialization of SkillTag from LLM Output
**What goes wrong:** `SkillTag` is a newtype `SkillTag(pub String)`. LLM outputs `"skill_tags": ["rust", "api"]` but serde expects `{"0": "rust"}` for tuple structs.
**Why it happens:** serde's default for newtype structs is transparent (just the inner value), so `["rust"]` actually works. But if the JSON schema sent to the LLM says `"items": {"type": "string"}`, serde will deserialize correctly. This is fine -- just be aware.
**How to avoid:** Verify in the JSON schema that skill_tags items are plain strings. The existing `project_spec_json_schema` already does this correctly.
**Warning signs:** None -- this works correctly with serde's transparent newtype behavior.

## Code Examples

### Type Definitions for ExecutionPlan

```rust
// crates/ath-types/src/plan.rs

use serde::{Deserialize, Serialize};
use crate::project::SkillTag;

/// A produces/consumes contract label.
/// String labels are simpler for LLM generation and sufficient for validation.
pub type ContractLabel = String;

/// A single task within a phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSpec {
    /// Human-readable task name.
    pub name: String,
    /// What this task should accomplish.
    pub description: String,
    /// Skills needed for this task.
    pub skill_tags: Vec<SkillTag>,
    /// Files this task is expected to produce or modify.
    pub expected_output_files: Vec<String>,
    /// Acceptance criteria -- how to verify the task is done.
    pub acceptance_criteria: Vec<String>,
    /// Indices into ProjectSpec.goals that this task fulfills.
    pub goal_indices: Vec<usize>,
}

/// A phase in the execution plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseSpec {
    /// Unique numeric ID for dependency references.
    pub id: u32,
    /// Human-readable phase name.
    pub name: String,
    /// Phase description.
    pub description: String,
    /// Ordered list of tasks in this phase (sequential execution).
    pub tasks: Vec<TaskSpec>,
    /// IDs of phases this phase depends on.
    pub depends_on: Vec<u32>,
    /// What this phase produces (contract labels).
    pub produces: Vec<ContractLabel>,
    /// What this phase consumes from its dependencies.
    pub consumes: Vec<ContractLabel>,
}

/// The complete execution plan produced by decomposition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionPlan {
    /// Ordered list of phases.
    pub phases: Vec<PhaseSpec>,
    /// Topologically sorted phase IDs (execution order).
    pub execution_order: Vec<u32>,
    /// Phases grouped by parallel-eligible levels.
    pub parallel_groups: Vec<Vec<u32>>,
    /// Length of the critical path (longest dependency chain).
    pub critical_path_length: usize,
}
```

### Validation Error Extensions

```rust
// Extend ValidationError in ath-types/src/error.rs with new variants:

/// Circular dependency detected in the phase DAG.
#[error("Circular dependency detected: {cycle_path}")]
CircularDependency {
    cycle_path: String,
    hint: String,
},

/// A phase depends on a non-existent phase ID.
#[error("Phase {phase_id} depends on non-existent phase {missing_id}")]
MissingDependencyTarget {
    phase_id: u32,
    missing_id: u32,
    hint: String,
},

/// A project goal has no task mapping to it.
#[error("Goal {goal_index} has no task mapping: {goal_description}")]
OrphanedGoal {
    goal_index: usize,
    goal_description: String,
    hint: String,
},

/// A phase has zero tasks.
#[error("Phase {phase_id} '{phase_name}' has no tasks")]
EmptyPhase {
    phase_id: u32,
    phase_name: String,
    hint: String,
},

/// A phase consumes a contract that no dependency produces.
#[error("Phase {phase_id} consumes '{contract}' but no dependency produces it")]
UnsatisfiedContract {
    phase_id: u32,
    contract: String,
    hint: String,
},
```

### DecomposeError Type

```rust
// crates/ath-planner/src/decompose/error.rs

use thiserror::Error;
use ath_types::ValidationError;

#[derive(Debug, Error)]
pub enum DecomposeError {
    #[error("Decomposition failed after {attempts} attempts")]
    DecomposeFailed {
        attempts: usize,
        last_error: Option<String>,
    },

    #[error("Agent error: {0}")]
    Agent(#[from] ath_agents::error::AgentError),

    #[error("Validation errors: {0:?}")]
    Validation(Vec<ValidationError>),
}
```

### JSON Schema for ExecutionPlan (LLM structured output)

```rust
pub fn execution_plan_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "phases": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "tasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "description": { "type": "string" },
                                    "skill_tags": { "type": "array", "items": { "type": "string" } },
                                    "expected_output_files": { "type": "array", "items": { "type": "string" } },
                                    "acceptance_criteria": { "type": "array", "items": { "type": "string" } },
                                    "goal_indices": { "type": "array", "items": { "type": "integer" } }
                                },
                                "required": ["name", "description", "skill_tags", "expected_output_files", "acceptance_criteria", "goal_indices"]
                            }
                        },
                        "depends_on": { "type": "array", "items": { "type": "integer" } },
                        "produces": { "type": "array", "items": { "type": "string" } },
                        "consumes": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["id", "name", "description", "tasks", "depends_on", "produces", "consumes"]
                }
            }
        },
        "required": ["phases"]
    })
}
```

Note: The LLM only generates `phases`. The fields `execution_order`, `parallel_groups`, and `critical_path_length` are computed server-side after validation. The deserialization target for LLM output should be a separate `RawPlanResponse` struct containing only `phases: Vec<PhaseSpec>`, and the full `ExecutionPlan` is assembled after validation.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Heuristic decomposition | LLM-based decomposition | 2024+ | LLM handles ambiguity, goal merging, and skill inference better than rule engines |
| Manual dependency specification | LLM-inferred dependencies | 2024+ | Dependencies are part of the structured output, not a separate step |
| External graph library for small DAGs | Hand-rolled Kahn's algorithm | Always valid for <20 nodes | Simpler, fewer dependencies, better error messages |

**Deprecated/outdated:**
- None relevant -- this is a greenfield implementation following established project patterns.

## Open Questions

1. **RawPlanResponse vs PhaseSpec deserialization**
   - What we know: LLM generates `phases` only; `execution_order`, `parallel_groups`, `critical_path_length` are computed.
   - What's unclear: Whether to deserialize into `PhaseSpec` directly or a separate `RawPhaseSpec` without computed fields.
   - Recommendation: Deserialize into `Vec<PhaseSpec>` directly (all fields from LLM). Build `ExecutionPlan` wrapper after validation adds the computed fields. This avoids type duplication.

2. **Warning delivery mechanism**
   - What we know: Warnings are non-blocking (unusually large phases, long critical paths).
   - What's unclear: How to propagate warnings alongside the success result.
   - Recommendation: Return `(ExecutionPlan, Vec<PlanWarning>)` tuple from the decompose function. PlanWarning is a simple enum in ath-planner, not in ath-types.

3. **Transitive contract validation scope**
   - What we know: A phase consumes a contract that must be produced by a dependency (direct or transitive).
   - What's unclear: Whether transitive checking is the right default or if it should be strict (direct only).
   - Recommendation: Use transitive closure. It matches developer intuition: if Phase C depends on B which depends on A, then C implicitly has access to A's outputs. Strict checking would cause excessive false positives.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p ath-planner --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PLAN-01 | Decompose ProjectSpec into phases with named tasks | unit | `cargo test -p ath-planner decompose::tests::decompose_produces_phases_with_tasks` | No -- Wave 0 |
| PLAN-01 | Retry on invalid LLM output | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_invalid_json` | No -- Wave 0 |
| PLAN-01 | Retry on validation failure | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_validation_error` | No -- Wave 0 |
| PLAN-02 | Topological sort produces valid order | unit | `cargo test -p ath-planner decompose::dag::tests::topological_sort_linear` | No -- Wave 0 |
| PLAN-02 | Circular dependency detected | unit | `cargo test -p ath-planner decompose::dag::tests::circular_dependency_detected` | No -- Wave 0 |
| PLAN-02 | Missing dependency target detected | unit | `cargo test -p ath-planner decompose::validate::tests::missing_dep_target` | No -- Wave 0 |
| PLAN-02 | Orphaned goals detected | unit | `cargo test -p ath-planner decompose::validate::tests::orphaned_goal_detected` | No -- Wave 0 |
| PLAN-02 | Empty phase detected | unit | `cargo test -p ath-planner decompose::validate::tests::empty_phase_detected` | No -- Wave 0 |
| PLAN-02 | Contract violation detected | unit | `cargo test -p ath-planner decompose::validate::tests::unsatisfied_contract` | No -- Wave 0 |
| PLAN-03 | Parallel-eligible phases identified | unit | `cargo test -p ath-planner decompose::dag::tests::parallel_groups_computed` | No -- Wave 0 |
| PLAN-03 | Critical path length computed | unit | `cargo test -p ath-planner decompose::dag::tests::critical_path_computed` | No -- Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p ath-planner --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/ath-types/src/plan.rs` -- new types (ExecutionPlan, PhaseSpec, TaskSpec)
- [ ] `crates/ath-planner/src/decompose/mod.rs` -- decompose entry point with retry loop
- [ ] `crates/ath-planner/src/decompose/dag.rs` -- topological sort, parallelism, critical path
- [ ] `crates/ath-planner/src/decompose/validate.rs` -- DAG validation (cycles, orphans, contracts)
- [ ] `crates/ath-planner/src/decompose/prompt.rs` -- system prompt, JSON schema, request builder
- [ ] `crates/ath-planner/src/decompose/error.rs` -- DecomposeError type
- [ ] Extended `ValidationError` variants in `ath-types/src/error.rs`

## Sources

### Primary (HIGH confidence)

- Project codebase: `crates/ath-planner/src/input/mod.rs` -- parse_to_project_spec retry pattern (template)
- Project codebase: `crates/ath-types/src/project.rs` -- ProjectSpec input type
- Project codebase: `crates/ath-types/src/error.rs` -- ValidationError pattern
- Project codebase: `crates/ath-agents/src/backend.rs` -- AgentBackend trait
- Project codebase: `crates/ath-agents/src/mock.rs` -- MockBackend for testing
- Project codebase: `crates/ath-planner/src/input/prompt.rs` -- JSON schema and request builder patterns

### Secondary (MEDIUM confidence)

- [petgraph toposort docs](https://docs.rs/petgraph/latest/petgraph/algo/fn.toposort.html) -- confirmed Kahn's algorithm approach, O(V+E) complexity
- [topo_sort crate](https://lib.rs/crates/topo_sort) -- confirmed cycle-safe topological sort pattern

### Tertiary (LOW confidence)

- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, no new dependencies
- Architecture: HIGH -- follows proven Phase 4 patterns exactly, module structure matches existing codebase conventions
- Pitfalls: HIGH -- identified from direct code analysis of existing retry pattern and LLM structured output usage
- DAG algorithms: HIGH -- Kahn's algorithm is textbook, well-understood, trivially testable

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no fast-moving dependencies)