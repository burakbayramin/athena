---
phase: 03
slug: git-layer
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + cargo test |
| **Config file** | None needed — cargo test works out of the box |
| **Quick run command** | `cargo test -p ath-git` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-git`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | OUTP-01 | unit | `cargo test -p ath-git -- git_layer_new_opens_repo` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | OUTP-01 | integration | `cargo test -p ath-git -- initial_commit_empty_repo` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | OUTP-01 | integration | `cargo test -p ath-git -- commit_contains_only_staged_files` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | OUTP-01 | unit | `cargo test -p ath-git -- commit_message_has_trailers` | ❌ W0 | ⬜ pending |
| 03-02-03 | 02 | 1 | OUTP-01 | integration | `cargo test -p ath-git -- one_commit_per_phase` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 2 | OUTP-01 | unit | `cargo test -p ath-git -- empty_diff_skips_commit` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 2 | OUTP-01 | unit | `cargo test -p ath-git -- missing_file_errors` | ❌ W0 | ⬜ pending |
| 03-03-03 | 03 | 2 | OUTP-01 | integration | `cargo test -p ath-git -- dirty_tree_errors` | ❌ W0 | ⬜ pending |
| 03-03-04 | 03 | 2 | OUTP-01 | unit | `cargo test -p ath-git -- conflict_detection` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-git/tests/integration.rs` — integration tests using tempfile for isolated repos
- [ ] `crates/ath-git/src/error.rs` — GitError type needed before tests can compile
- [ ] Add `tempfile = { workspace = true }` to workspace dependencies and ath-git dev-dependencies
- [ ] Add `git2 = { workspace = true }` to workspace dependencies

*Wave 0 creates test stubs that compile but may fail — execution fills them in.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
