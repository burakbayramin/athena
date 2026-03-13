# Phase 10: Parallel Execution - Research

**Researched:** 2026-03-13
**Domain:** Async concurrent phase dispatch, git commit serialization, cross-phase isolation
**Confidence:** HIGH

## Summary

Phase 10 enables parallel execution of independent phases that are already computed as `parallel_groups` by the DAG decomposition engine (Phase 5). The entire infrastructure for identifying parallelizable phases exists: `compute_parallel_groups()` in `ath-planner` produces `Vec<Vec<u32>>` groups, `check_isolation()` in `ath-orchestrator/isolation.rs` validates file ownership across phases within the same group, and `AgentRegistry` uses `Arc<dyn AgentBackend>` which is safe to share across concurrent tasks. The `AgentCoordinator` currently iterates `execution_order` sequentially in a `for` loop -- the change is to iterate `parallel_groups` instead and use `tokio::task::JoinSet` to fan out phases within each group.

The two hard problems are: (1) the `write_files` closure and git commits must be serialized when parallel phases complete, because `GitLayer` uses `Arc<Mutex<Repository>>` which serializes access but doesn't prevent interleaved staging, and (2) progress events from concurrent phases must not corrupt the observer state. Both are solvable with existing primitives -- a `tokio::sync::Mutex` around the commit path and the existing `Arc<dyn ProgressObserver>` which already requires `Send + Sync`.

**Primary recommendation:** Restructure `AgentCoordinator::run_plan_with_progress` to iterate `parallel_groups` instead of `execution_order`, spawning a `JoinSet` per group. Add a `tokio::sync::Mutex` commit gate. Run `check_isolation` before dispatch. No new crates needed.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ORCH-04 | Independent phases execute in parallel across agents simultaneously | `parallel_groups` already computed by DAG engine; `JoinSet` fan-out in coordinator loop; `check_isolation` pre-dispatch validation; commit serialization via `tokio::sync::Mutex` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x (workspace) | JoinSet for concurrent phase dispatch | Already in workspace; `rt-multi-thread` + `sync` features already enabled |
| git2 | 0.20 (workspace) | Commit serialization via existing `Arc<Mutex<Repository>>` | Already in workspace; thread-safe via Mutex |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::sync::Mutex | 1.x | Serialize commit operations across parallel phases | Wrap `write_files` + `commit_phase_async` in a single critical section |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JoinSet per group | `tokio::join!` macro | JoinSet handles dynamic-length groups; `join!` requires compile-time known count |
| tokio::sync::Mutex for commits | Channel-based commit queue | Mutex is simpler; channel adds unnecessary indirection for a serialize-only use case |
| Shared output_dir with mutex | Per-phase temp dirs merged after | Adds complexity; current `write_files` approach just needs serialization |

**Installation:**
```bash
# No new dependencies needed. All required crates are already in the workspace.
```

## Architecture Patterns

### Recommended Changes to Project Structure
```
crates/ath-orchestrator/src/
    coordinator.rs      # MODIFY: parallel group iteration + JoinSet fan-out
    isolation.rs        # MODIFY: add pre-dispatch check_isolation call site (already has logic)
    error.rs            # MODIFY: add ParallelPhaseError variant
    phase_runner.rs     # NO CHANGE: run_phase_with_progress stays as-is
    progress.rs         # NO CHANGE: observer is already Send + Sync
```

### Pattern 1: Group-Level JoinSet Fan-Out
**What:** Replace the flat `execution_order` iteration with a two-level loop: outer loop over `parallel_groups`, inner `JoinSet::spawn` for each phase in a group.
**When to use:** Every plan execution -- single-phase groups degenerate to sequential behavior.
**Example:**
```rust
// Source: Architecture research + tokio JoinSet docs
for group in &plan.parallel_groups {
    if group.len() == 1 {
        // Single phase -- run directly (avoids spawn overhead)
        let record = run_single_phase(&group[0], ...)?;
        results.push(record);
    } else {
        // Pre-dispatch isolation check
        check_isolation_for_group(plan, group)?;

        let mut set = JoinSet::new();
        for &phase_id in group {
            let phase = find_phase(plan, phase_id)?;
            let registry = registry.clone(); // Arc-backed, cheap
            let commit_gate = commit_gate.clone(); // Arc<tokio::sync::Mutex<()>>
            set.spawn(async move {
                let record = run_phase_with_progress(&phase, &registry, ...).await?;
                // Serialize file writes and git commits
                let _guard = commit_gate.lock().await;
                write_files(&record.files)?;
                git_commit(&record)?;
                Ok::<_, PhaseRunnerError>((phase_id, record))
            });
        }
        while let Some(result) = set.join_next().await {
            let (phase_id, record) = result
                .map_err(|e| /* JoinError handling */)?
                .map_err(|e| /* PhaseRunnerError propagation */)?;
            results.push(record);
        }
    }
}
```

### Pattern 2: Pre-Dispatch Isolation Gate
**What:** Call `check_isolation()` on the plan before entering the parallel dispatch loop. This catches file conflicts at plan validation time, not after LLM calls.
**When to use:** Always, before any JoinSet spawn.
**Example:**
```rust
// Source: existing isolation.rs check_isolation function
use crate::isolation::check_isolation;

// Before dispatch loop
check_isolation(plan).map_err(|e| PhaseRunnerError::IsolationViolation {
    details: e.to_string(),
})?;
```

### Pattern 3: Serialized Commit Gate
**What:** A `tokio::sync::Mutex<()>` shared across all spawned phase tasks within a group. After a phase completes its LLM work (which runs concurrently), it acquires the gate before writing files and committing.
**When to use:** Every parallel group with >1 phase.
**Why not std::sync::Mutex:** The commit includes `commit_phase_async` which is async (spawn_blocking internally). Using `std::sync::Mutex` across an await point would block the runtime.

### Anti-Patterns to Avoid
- **Spawning without isolation check:** Never spawn parallel phases without first validating file ownership disjointness. The check is O(files) and prevents silent data corruption.
- **Holding commit lock during LLM execution:** The commit gate must only be held during file-write + git-commit, not during the entire phase execution. Phase execution (LLM calls) is the slow part and must remain concurrent.
- **Modifying execution_order interpretation:** The `execution_order` field remains a valid topological sort. Parallel dispatch reads from `parallel_groups` which is the authoritative structure for concurrency. Do not try to infer parallelism from `execution_order` alone.
- **Per-phase output directories:** Don't create separate temp dirs per parallel phase. The existing `write_files` closure writes to `output_dir` and the commit gate serializes access. Separate dirs would require a merge step that complicates error handling.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Concurrent task collection | Custom future polling | `tokio::task::JoinSet` | Handles cancellation, panics, and result ordering correctly |
| Async-safe mutual exclusion | `std::sync::Mutex` across await | `tokio::sync::Mutex` | std Mutex blocks the runtime thread; tokio Mutex yields |
| Parallel group computation | Recomputing from depends_on | Existing `compute_parallel_groups()` | Already validated with tests in dag.rs |
| File conflict detection | Manual set intersection | Existing `check_isolation()` | Already tested with 8+ unit tests in isolation.rs |

**Key insight:** Almost all infrastructure for parallel execution already exists in the codebase. The work is primarily in `coordinator.rs` -- changing the iteration strategy and adding commit serialization.

## Common Pitfalls

### Pitfall 1: JoinError Panic Propagation
**What goes wrong:** `JoinSet::join_next()` returns `Result<T, JoinError>`. A `JoinError` means the spawned task panicked or was cancelled. If not handled, the coordinator silently drops completed phases.
**Why it happens:** Easy to `.unwrap()` the outer Result and only handle the inner `PhaseRunnerError`.
**How to avoid:** Map `JoinError` to a specific `PhaseRunnerError` variant that includes the phase name.
**Warning signs:** Test with a phase that panics (mock backend that panics on call).

### Pitfall 2: Non-Deterministic Result Order
**What goes wrong:** `JoinSet` returns results in completion order, not spawn order. If results are collected into a Vec with `push`, the `PhaseRecord` ordering is random across runs.
**Why it happens:** Network latency varies; different phases take different times.
**How to avoid:** Collect `(phase_id, PhaseRecord)` tuples, then sort by the original `parallel_groups` order before appending to results. Or use a `HashMap<u32, PhaseRecord>` and reconstruct order.
**Warning signs:** Flaky tests where record ordering assertions fail intermittently.

### Pitfall 3: Shared Registry Contention
**What goes wrong:** Multiple parallel phases dispatch to the same agent backend concurrently. If the backend has internal mutable state, this causes contention.
**Why it happens:** `AgentRegistry` returns `Arc<dyn AgentBackend>` -- the Arc is shared.
**How to avoid:** This is actually fine in the current design. `AgentBackend` implementations use actor-model channels (mpsc). Sending on a channel is safe from multiple callers. The `MockBackend` uses `Mutex<VecDeque>` which also handles concurrent access. No action needed, but worth a test to confirm.

### Pitfall 4: Progress Event Interleaving
**What goes wrong:** Parallel phases emit `PhaseStarted`, `TaskStarted`, etc. concurrently. The terminal display shows interleaved lines.
**Why it happens:** `ProgressObserver::on_event` is called from different tokio tasks simultaneously.
**How to avoid:** The observer implementation already uses `Mutex<Vec<ProgressEvent>>` for recording. For terminal display, indicatif multi-progress bars handle concurrent updates natively. The existing architecture handles this correctly because `ProgressObserver` requires `Send + Sync`.
**Warning signs:** Garbled terminal output during parallel execution.

### Pitfall 5: Git Index Corruption Under Concurrent Staging
**What goes wrong:** Two phases complete simultaneously, both call `stage_and_commit`. The git index is a single file -- concurrent writes corrupt it.
**Why it happens:** `GitLayer` uses `Arc<Mutex<Repository>>` but `stage_and_commit` holds the lock for the entire operation. If two async tasks try to stage concurrently, the Mutex serializes them. However, if `write_files` happens outside the Mutex, files could be written by phase B while phase A is staging.
**How to avoid:** The commit gate pattern (tokio::sync::Mutex wrapping both write_files AND git commit) ensures atomic write+commit per phase. The git2 Mutex alone is not sufficient because file writes happen before git staging.

## Code Examples

### AgentCoordinator Parallel Dispatch (Proposed Structure)
```rust
// Source: derived from existing coordinator.rs + tokio JoinSet docs
use tokio::task::JoinSet;
use tokio::sync::Mutex as AsyncMutex;
use std::sync::Arc;

pub async fn run_plan_with_progress(
    &self,
    plan: &ExecutionPlan,
    observer: Option<SharedProgressObserver>,
) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
    // Pre-dispatch: validate isolation across ALL parallel groups
    crate::isolation::check_isolation(plan)
        .map_err(|e| /* convert IsolationError to PhaseRunnerError */)?;

    let commit_gate = Arc::new(AsyncMutex::new(()));
    let mut results: Vec<PhaseRecord> = Vec::new();

    for group in &plan.parallel_groups {
        let group_results = if group.len() == 1 {
            // Optimize: skip JoinSet overhead for single-phase groups
            vec![self.run_single_phase(plan, group[0], &observer, &commit_gate).await?]
        } else {
            self.run_parallel_group(plan, group, &observer, &commit_gate).await?
        };
        results.extend(group_results);
    }

    Ok(results)
}
```

### Commit Gate Pattern
```rust
// Inside a spawned parallel phase task
let record = run_phase_with_progress(phase, registry, available, write_files, None, observer)
    .await?;

// Acquire commit gate -- only one phase commits at a time
let _guard = commit_gate.lock().await;
write_phase_files(&record.files, &output_dir)?;
if let Some(git) = &git {
    git.commit_phase_async(file_paths, metadata).await?;
}
drop(_guard);
// Return record for collection
```

### IsolationError to PhaseRunnerError Conversion
```rust
// New variant needed in PhaseRunnerError
#[error("Isolation violation: {details}")]
IsolationViolation { details: String },
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Sequential execution_order loop | parallel_groups + JoinSet | This phase | Reduces wall-clock time for parallelizable projects |
| check_isolation called externally | check_isolation integrated into coordinator dispatch | This phase | Isolation check is mandatory, not optional |
| write_files + git as separate unsynchronized calls | Commit gate serializes both atomically | This phase | Prevents git index corruption |

## Open Questions

1. **Error handling for partial parallel group failure**
   - What we know: If one phase in a group fails, other phases in the group may still be running.
   - What's unclear: Should we abort remaining phases in the group (JoinSet abort_all) or let them complete?
   - Recommendation: Abort remaining phases via `set.abort_all()` on first error. This matches the existing fail-fast behavior of sequential execution. Completed results before the failure are discarded.

2. **PhaseRecord ordering in results**
   - What we know: Sequential execution guarantees execution_order ordering. Parallel execution returns completion order.
   - What's unclear: Does downstream code (reporting, CLI) depend on specific ordering?
   - Recommendation: Sort group results by phase_id before extending the results vec. This preserves deterministic ordering for reports.

3. **Progress event phase_index semantics**
   - What we know: Current PhaseStarted/PhaseCompleted events include `phase_index` (1-based position in execution_order).
   - What's unclear: How to number parallel phases -- they share the same "position" in the plan.
   - Recommendation: Use the group index as the position, with all phases in a group sharing the same phase_index. The terminal can show "Phase group 2/4 (3 parallel phases)".

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + tokio::test for async |
| Config file | Cargo.toml per crate (workspace) |
| Quick run command | `cargo test -p ath-orchestrator --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ORCH-04-a | Parallel phases start times overlap | integration | `cargo test -p ath-orchestrator parallel_phases_overlap` | Wave 0 |
| ORCH-04-b | Parallel faster than sequential | integration | `cargo test -p ath-orchestrator parallel_faster_than_sequential` | Wave 0 |
| ORCH-04-c | Isolation blocks conflicting dispatch | unit | `cargo test -p ath-orchestrator parallel_isolation_blocks_conflict` | Wave 0 |
| ORCH-04-d | Git commits correct per-phase metadata | integration | `cargo test -p ath-orchestrator parallel_commits_correct_metadata` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-orchestrator --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] All 4 test cases above need to be created as failing tests before implementation
- [ ] No new test infrastructure (fixtures, framework) needed -- existing MockBackend and tempfile patterns cover all cases

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `coordinator.rs`, `isolation.rs`, `phase_runner.rs`, `dag.rs`, `async_ops.rs`, `layer.rs`, `plan.rs`, `error.rs` -- direct source of all architectural claims
- [tokio JoinSet docs](https://docs.rs/tokio/latest/tokio/task/join_set/struct.JoinSet.html) -- API reference for spawn/join_next/abort_all

### Secondary (MEDIUM confidence)
- `.planning/research/ARCHITECTURE.md` -- original architecture research establishing JoinSet as the parallel dispatch mechanism
- `.planning/research/SUMMARY.md` -- phase ordering rationale confirming "sequential before parallel" strategy

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in workspace, no new deps needed
- Architecture: HIGH - all building blocks exist in codebase, change is localized to coordinator.rs
- Pitfalls: HIGH - git2 concurrency issues are well-understood; commit gate pattern is standard
- Testing: HIGH - existing MockBackend + tempfile patterns cover all scenarios

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- no external dependencies changing)