---
phase: 10
slug: parallel-execution
status: draft
nyquist_compliant: true
wave_0_complete: true
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
| 10-00-01 | 00 | 0 | ORCH-04 | stub | `cargo test -p ath-orchestrator parallel_ -- 2>&1 \| grep FAILED` | W0 creates | ⬜ pending |
| 10-01-01 | 01 | 1 | ORCH-04-a | integration | `cargo test -p ath-orchestrator parallel_phases_overlap` | ✅ W0 | ⬜ pending |
| 10-01-02 | 01 | 1 | ORCH-04-c | unit | `cargo test -p ath-orchestrator parallel_isolation_blocks_conflict` | ✅ W0 | ⬜ pending |
| 10-02-01 | 02 | 2 | ORCH-04-d | integration | `cargo test -p ath-orchestrator parallel_commits_correct_metadata` | ✅ W0 | ⬜ pending |
| 10-02-02 | 02 | 2 | ORCH-04-b | integration | `cargo test -p ath-orchestrator parallel_faster_than_sequential` | ✅ W0 | ⬜ pending |

*Status: ⬜ pending -- ✅ green -- ❌ red -- ⚠️ flaky*

---

## Wave 0 Requirements

- [x] All 4 test cases created as failing stubs in plan 00 before implementation
- [x] No new test infrastructure (fixtures, framework) needed -- existing MockBackend and tempfile patterns cover all cases

*Wave 0 plan (10-00-PLAN.md) creates the 4 failing test stubs. Plans 01 and 02 depend on plan 00.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Parallel faster than sequential | ORCH-04-b | Timing-dependent, may need generous thresholds | Run with `--nocapture`, verify wall-clock difference |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (Wave 0 plan added)
