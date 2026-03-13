---
phase: 8
slug: cli-and-progress
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 8 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace test settings |
| **Quick run command** | `cargo test -p ath-cli` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-cli`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | SC-01 | unit | `cargo test -p ath-cli cli_surface::tests` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 2 | OUTP-02 | unit | `cargo test -p ath-cli progress::tests` | ❌ W0 | ⬜ pending |
| 08-02-02 | 02 | 2 | OUTP-02 | unit | `cargo test -p ath-orchestrator progress::tests` | ❌ W0 | ⬜ pending |
| 08-03-01 | 03 | 3 | SC-04 | unit | `cargo test -p ath-cli verbose::tests` | ❌ W0 | ⬜ pending |
| 08-04-01 | 04 | 4 | PLAN-05 | unit | `cargo test -p ath-cli dry_run::tests::dry_run_requires_local_plan` | ❌ W0 | ⬜ pending |
| 08-04-02 | 04 | 4 | PLAN-05 | integration | `cargo test -p ath-cli dry_run::tests::dry_run_prints_full_plan_without_execution` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠ flaky*

---

## Wave 0 Requirements

- [ ] `Cargo.toml` - add progress-reporting dependencies chosen by the planner
- [ ] `crates/ath-orchestrator/src/progress.rs` (or equivalent) - observer/event infrastructure
- [ ] `crates/ath-cli/src/progress.rs` - default terminal reporter
- [ ] `crates/ath-cli/src/verbose.rs` - transcript grouping/redaction tests
- [ ] `crates/ath-cli/src/dry_run.rs` - local plan loading and dry-run tests

*Existing infrastructure covers the rest once these modules are added.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Hybrid live board remains readable in a real terminal | OUTP-02 | Animated progress output is difficult to assert reliably in unit tests | Run `ath run` in an interactive terminal and verify current phase, current agent, task checklist, and retry messages stay legible |
| `NO_COLOR` / redirected output remains plain-text friendly | OUTP-02 | Interactivity detection is environment-sensitive | Run `NO_COLOR=1 ath run ...` and `ath run ... > out.txt` and confirm there are no escape codes or broken spinner frames |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
