---
phase: 1
slug: foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (cargo test) |
| **Config file** | None needed — Cargo's default test runner |
| **Quick run command** | `cargo test -p ath-types && cargo test -p ath-config` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <affected-crate>`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | INPT-04 | unit | `cargo test -p ath-config -- env` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | INPT-04 | unit | `cargo test -p ath-config -- file` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | INPT-04 | integration | `cargo test -p ath-config --test precedence` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 1 | INPT-04 | unit | `cargo test -p ath-config -- missing_key` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | PLAN-04 | unit | `cargo test -p ath-types -- project_spec_round_trip` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 1 | PLAN-04 | unit | `cargo test -p ath-types -- agent_round_trip` | ❌ W0 | ⬜ pending |
| 01-02-03 | 02 | 1 | PLAN-04 | unit | `cargo test -p ath-types -- review_round_trip` | ❌ W0 | ⬜ pending |
| 01-02-04 | 02 | 1 | PLAN-04 | unit | `cargo test -p ath-types -- phase_record_round_trip` | ❌ W0 | ⬜ pending |
| 01-02-05 | 02 | 1 | PLAN-04 | unit | `cargo test -p ath-types -- validate` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 2 | SC-1 | integration | `cargo test -p ath-cli --test version_check` | ❌ W0 | ⬜ pending |
| 01-03-02 | 03 | 2 | SC-2 | integration | `cargo test -p ath-config --test error_display` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] All test files — greenfield project, no test infrastructure exists yet
- [ ] Tests will be created alongside implementation in each plan
- [ ] No external test framework install needed — cargo test is built-in

*Existing infrastructure covers framework needs; test files are Wave 0 gaps.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `ath --version` output format | SC-1 | Visual inspection of CLI output | Run `cargo run -- --version`, verify version string and no panic |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
