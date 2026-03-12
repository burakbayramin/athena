---
phase: 2
slug: agent-clients
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`#[cfg(test)]` + `#[tokio::test]`) |
| **Config file** | None needed — Cargo.toml `[dev-dependencies]` |
| **Quick run command** | `cargo test -p ath-agents` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-agents`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | ORCH-05 | unit | `cargo test -p ath-agents -- backend` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 2 | ORCH-05a | unit | `cargo test -p ath-agents -- claude` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 2 | ORCH-05d | unit | `cargo test -p ath-agents -- backoff` | ❌ W0 | ⬜ pending |
| 02-02-03 | 02 | 2 | ORCH-05e | unit | `cargo test -p ath-agents -- circuit` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 2 | ORCH-05b | unit | `cargo test -p ath-agents -- gemini` | ❌ W0 | ⬜ pending |
| 02-04-01 | 04 | 2 | ORCH-05c | unit | `cargo test -p ath-agents -- codex` | ❌ W0 | ⬜ pending |
| 02-05-01 | 05 | 3 | ORCH-05a/b/c | integration | `cargo test -p ath-agents -- integration` | ❌ W0 | ⬜ pending |
| 02-05-02 | 05 | 3 | ORCH-05e/f | integration | `cargo test -p ath-agents -- circuit_integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-agents/src/circuit_breaker.rs` tests — covers ORCH-05e, ORCH-05f
- [ ] `crates/ath-agents/src/error.rs` tests — covers ORCH-05g
- [ ] `crates/ath-agents/src/mock.rs` tests — covers ORCH-05a/b/c
- [ ] Add `tokio` with `test-util` feature to `[dev-dependencies]` for time manipulation in circuit breaker tests
- [ ] Add `backon` to workspace dependencies

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real API round-trip | ORCH-05a/b/c | Requires live API keys and network | Run `cargo test -p ath-agents -- --ignored live_` with env vars set |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
