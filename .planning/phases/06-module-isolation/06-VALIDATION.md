---
phase: 6
slug: module-isolation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (#[test], #[cfg(test)]) |
| **Config file** | None needed -- Rust test framework is zero-config |
| **Quick run command** | `cargo test -p ath-orchestrator` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-orchestrator`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | ORCH-01 | unit | `cargo test -p ath-orchestrator -- taxonomy` | No -- W0 | pending |
| 06-01-02 | 01 | 1 | ORCH-01 | unit | `cargo test -p ath-orchestrator -- taxonomy::fallback` | No -- W0 | pending |
| 06-02-01 | 02 | 1 | ORCH-02 | unit | `cargo test -p ath-orchestrator -- router::majority` | No -- W0 | pending |
| 06-02-02 | 02 | 1 | ORCH-02 | unit | `cargo test -p ath-orchestrator -- router::tiebreak` | No -- W0 | pending |
| 06-02-03 | 02 | 1 | ORCH-02 | unit | `cargo test -p ath-orchestrator -- router::fallback` | No -- W0 | pending |
| 06-02-04 | 02 | 1 | ORCH-02 | unit | `cargo test -p ath-orchestrator -- router::all_unavailable` | No -- W0 | pending |
| 06-02-05 | 02 | 1 | ORCH-02 | unit | `cargo test -p ath-orchestrator -- router::rationale` | No -- W0 | pending |
| 06-03-01 | 03 | 2 | ORCH-03 | unit | `cargo test -p ath-orchestrator -- isolation::conflict` | No -- W0 | pending |
| 06-03-02 | 03 | 2 | ORCH-03 | unit | `cargo test -p ath-orchestrator -- isolation::sequential_ok` | No -- W0 | pending |
| 06-03-03 | 03 | 2 | ORCH-03 | unit | `cargo test -p ath-orchestrator -- isolation::audit` | No -- W0 | pending |
| 06-03-04 | 03 | 2 | ORCH-03 | unit | `cargo test -p ath-orchestrator -- isolation::clean_pass` | No -- W0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-orchestrator/src/taxonomy.rs` -- routing table module with test stubs for ORCH-01
- [ ] `crates/ath-orchestrator/src/router.rs` -- agent router with test stubs for ORCH-02
- [ ] `crates/ath-orchestrator/src/isolation.rs` -- file ownership + audit with test stubs for ORCH-03
- [ ] `crates/ath-orchestrator/src/error.rs` -- IsolationError type
- [ ] Update `crates/ath-types/src/plan.rs` -- add assigned_agent field to TaskSpec
- [ ] Update `crates/ath-orchestrator/Cargo.toml` -- add ath-agents, thiserror, serde deps

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Verbose output shows assignment rationale | ORCH-02 | CLI output formatting | Run `athena execute --verbose` and verify rationale lines appear |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
