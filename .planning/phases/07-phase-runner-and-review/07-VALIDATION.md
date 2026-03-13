---
phase: 7
slug: phase-runner-and-review
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace test settings |
| **Quick run command** | `cargo test -p ath-orchestrator --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-orchestrator --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | QUAL-02 | unit | `cargo test -p ath-orchestrator phase_runner::tests::valid_transitions` | ❌ W0 | ⬜ pending |
| 07-01-02 | 01 | 1 | QUAL-02 | unit | `cargo test -p ath-orchestrator phase_runner::tests::review_gate_blocks` | ❌ W0 | ⬜ pending |
| 07-02-01 | 02 | 1 | QUAL-01 | unit | `cargo test -p ath-orchestrator review::tests::reviewer_excludes_author` | ❌ W0 | ⬜ pending |
| 07-02-02 | 02 | 1 | QUAL-01 | unit | `cargo test -p ath-orchestrator review::tests::majority_author_excluded` | ❌ W0 | ⬜ pending |
| 07-02-03 | 02 | 1 | QUAL-01 | unit | `cargo test -p ath-orchestrator review::tests::reviewer_fallback_on_circuit_break` | ❌ W0 | ⬜ pending |
| 07-03-01 | 03 | 1 | QUAL-03 | unit | `cargo test -p ath-orchestrator phase_runner::tests::retry_injects_feedback` | ❌ W0 | ⬜ pending |
| 07-03-02 | 03 | 1 | QUAL-03 | unit | `cargo test -p ath-orchestrator phase_runner::tests::max_three_attempts` | ❌ W0 | ⬜ pending |
| 07-04-01 | 04 | 2 | QUAL-03 | integration | `cargo test -p ath-orchestrator coordinator::tests::full_pipeline_passes` | ❌ W0 | ⬜ pending |
| 07-04-02 | 04 | 2 | QUAL-03 | integration | `cargo test -p ath-orchestrator coordinator::tests::pipeline_retry_then_pass` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-orchestrator/src/phase_runner.rs` — typestate machine + transition tests
- [ ] `crates/ath-orchestrator/src/review.rs` — ReviewEngine + reviewer selection tests
- [ ] `crates/ath-orchestrator/src/coordinator.rs` — AgentCoordinator + integration tests
- [ ] `crates/ath-orchestrator/src/error.rs` — extend with PhaseRunnerError variants
- [ ] Add `tokio`, `async-trait`, `chrono`, `uuid`, `ath-git` to ath-orchestrator Cargo.toml

*All test files are new — Wave 0 creates the test infrastructure alongside production code.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Typestate prevents invalid transitions at compile time | QUAL-02 | Compile-fail tests are not standard cargo test — verify by attempting invalid transition in code and confirming compile error | Write a test that tries `PhaseState<Pending>.approve()` and verify it does not compile |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
