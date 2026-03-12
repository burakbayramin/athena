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
| 05-01-01 | 01 | 1 | PLAN-02 | unit | `cargo test -p ath-planner decompose::dag::tests::topological_sort_linear` | No W0 | pending |
| 05-01-02 | 01 | 1 | PLAN-03 | unit | `cargo test -p ath-planner decompose::dag::tests::parallel_groups_computed` | No W0 | pending |
| 05-01-03 | 01 | 1 | PLAN-03 | unit | `cargo test -p ath-planner decompose::dag::tests::critical_path_computed` | No W0 | pending |
| 05-02-01 | 02 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::circular_dependency_detected` | No W0 | pending |
| 05-02-02 | 02 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::missing_dep_target` | No W0 | pending |
| 05-02-03 | 02 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::orphaned_goal_detected` | No W0 | pending |
| 05-02-04 | 02 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::empty_phase_detected` | No W0 | pending |
| 05-02-05 | 02 | 2 | PLAN-02 | unit | `cargo test -p ath-planner decompose::validate::tests::unsatisfied_contract` | No W0 | pending |
| 05-02-06 | 02 | 2 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_produces_phases_with_tasks` | No W0 | pending |
| 05-02-07 | 02 | 2 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_invalid_json` | No W0 | pending |
| 05-02-08 | 02 | 2 | PLAN-01 | unit | `cargo test -p ath-planner decompose::tests::decompose_retries_on_validation_error` | No W0 | pending |
| 05-03-01 | 03 | 3 | PLAN-01 | unit | `cargo test -p ath-planner decompose::display::tests::display_contains_phase_info` | No W0 | pending |

*Status: pending / green / red / flaky*

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

*None for Phase 5. Dry-run CLI output formatting is deferred to Phase 8 (PLAN-05 scope).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
