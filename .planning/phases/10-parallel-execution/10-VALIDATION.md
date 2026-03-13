---
phase: 10
slug: parallel-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + tokio::test for async |
| **Config file** | Cargo.toml per crate (workspace) |
| **Quick run command** | `cargo test -p ath-orchestrator --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-orchestrator --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | ORCH-04-a | integration | `cargo test -p ath-orchestrator parallel_phases_overlap` | ❌ W0 | ⬜ pending |
| 10-02-01 | 02 | 1 | ORCH-04-c | unit | `cargo test -p ath-orchestrator parallel_isolation_blocks_conflict` | ❌ W0 | ⬜ pending |
| 10-03-01 | 03 | 2 | ORCH-04-d | integration | `cargo test -p ath-orchestrator parallel_commits_correct_metadata` | ❌ W0 | ⬜ pending |
| 10-04-01 | 04 | 2 | ORCH-04-b | integration | `cargo test -p ath-orchestrator parallel_faster_than_sequential` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] All 4 test cases above need to be created as failing tests before implementation
- [ ] No new test infrastructure (fixtures, framework) needed — existing MockBackend and tempfile patterns cover all cases

*Existing infrastructure covers most phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Parallel faster than sequential | ORCH-04-b | Timing-dependent, may need generous thresholds | Run with `--nocapture`, verify wall-clock difference |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
