---
phase: 05
slug: phase-decomposition
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 05 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p ath-planner --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-planner --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_produces_phases_with_tasks` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_invalid_json` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_validation_error` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 1 | PLAN-02 | unit | `cargo test -p ath-planner decompose::dag::tests::topological_sort_linear` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 1 | PLAN-03 | unit | `cargo test -p ath-planner decompose::dag::tests::parallel_groups_computed` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 1 | PLAN-03 | unit | `cargo test -p ath-planner decompose::dag::tests::critical_path_computed` | ❌ W0 | ⬜ pending |
| 05-03-01 | 03 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::circular_dependency_detected` | ❌ W0 | ⬜ pending |
| 05-03-02 | 03 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::missing_dep_target` | ❌ W0 | ⬜ pending |
| 05-03-03 | 03 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::orphaned_goal_detected` | ❌ W0 | ⬜ pending |
| 05-03-04 | 03 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::empty_phase_detected` | ❌ W0 | ⬜ pending |
| 05-03-05 | 03 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::unsatisfied_contract` | ❌ W0 | ⬜ pending |
| 05-04-01 | 04 | 2 | PLAN-05 | unit | `cargo test -p ath-planner decompose::display::tests::dry_run_output` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-types/src/plan.rs` — ExecutionPlan, PhaseSpec, TaskSpec type definitions
- [ ] `crates/ath-planner/src/decompose/mod.rs` — decompose entry point module stub
- [ ] `crates/ath-planner/src/decompose/dag.rs` — topological sort and parallelism module stub
- [ ] `crates/ath-planner/src/decompose/validate.rs` — DAG validation module stub
- [ ] `crates/ath-planner/src/decompose/prompt.rs` — prompt and schema module stub
- [ ] `crates/ath-planner/src/decompose/error.rs` — DecomposeError type

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dry-run CLI output formatting | PLAN-05 | Visual output formatting | Run `ath run --dry-run "build a todo app"` and verify table is readable |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
