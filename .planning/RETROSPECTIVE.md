# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-03-13
**Phases:** 10 | **Plans:** 35 | **Commits:** 165

### What Was Built
- Complete Rust CLI orchestrating three AI agents (Claude, Gemini, Codex)
- DAG-based phase decomposition with topological sort and parallel group detection
- Module isolation with skill taxonomy routing and file ownership enforcement
- PhaseRunner typestate machine with cross-agent review gates
- Parallel execution via tokio JoinSet with isolation pre-checks
- Structured reporting with per-phase token usage and cost estimation
- Real-time terminal progress with indicatif + tracing integration

### What Worked
- Infrastructure-first build order (types → clients → git → parsing → decomposition → isolation → runner → CLI → reporting → parallel) prevented rework
- Typestate pattern for PhaseRunner caught invalid state transitions at compile time
- MockBackend test double enabled comprehensive testing without API calls
- Fine-grained plan decomposition (35 plans across 10 phases) kept each unit focused and reviewable
- 2-day execution timeline for 16k LOC demonstrates high throughput

### What Was Inefficient
- ROADMAP.md progress table fell out of sync with actual completion status for phases 1, 3-6
- Some phases had plan counts adjusted mid-execution (Phase 1: 2→4 plans, Phase 4: 2→4 plans)
- git2 async patterns required extra research due to limited ecosystem examples

### Patterns Established
- `AgentBackend` trait as uniform interface for all AI providers
- `CircuitBreaker` per-provider for fail-fast behavior
- `Arc<AgentRegistry>` pattern for safe concurrent agent access
- Frontmatter-based SUMMARY.md format with provides/affects dependency tracking
- `DelayedMockBackend` pattern for testing parallel timing behavior

### Key Lessons
1. Typestate machines are worth the boilerplate — compile-time state safety eliminates entire categories of runtime bugs
2. Plan decomposition granularity matters — 3-5 tasks per plan keeps execution focused
3. Test doubles (MockBackend) should be first-class citizens, not afterthoughts
4. Progress table maintenance should be automated rather than manually tracked

### Cost Observations
- Model mix: primarily opus for planning and execution
- Sessions: ~10 sessions across 2 days
- Notable: parallel agent execution (35 plans) completed in 2.2 hours total execution time

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Commits | Phases | Key Change |
|-----------|---------|--------|------------|
| v1.0 | 165 | 10 | Initial infrastructure-first build |

### Cumulative Quality

| Milestone | LOC | Crates | Plans |
|-----------|-----|--------|-------|
| v1.0 | 16,315 | 7 | 35 |

### Top Lessons (Verified Across Milestones)

1. Infrastructure-first ordering prevents cascading rework
2. Typestate patterns provide compile-time safety for complex state machines
