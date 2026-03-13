---
phase: 9
slug: reporting-and-error-quality
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 9 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace test settings |
| **Quick run command** | `cargo test -p ath-cli` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-cli`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::writes_run_report_artifacts` | ❌ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::latest_run_default_resolves` | ❌ W0 | ⬜ pending |
| 09-02-01 | 02 | 2 | OUTP-03 | unit | `cargo test -p ath-cli report::tests::renders_human_readable_summary` | ❌ W0 | ⬜ pending |
| 09-03-01 | 03 | 3 | QUAL-04 | unit | `cargo test -p ath-orchestrator phase_runner::tests::run_phase_record_tracks_token_usage` | ✅ | ⬜ pending |
| 09-03-02 | 03 | 3 | QUAL-04 | unit | `cargo test -p ath-cli report::tests::computes_per_phase_and_run_costs` | ❌ W0 | ⬜ pending |
| 09-04-01 | 04 | 4 | OUTP-04 | unit | `cargo test -p ath-cli error_display::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠ flaky*

---

## Wave 0 Requirements

- [ ] `crates/ath-types/src/` - serializable report types and test fixtures for persisted run artifacts
- [ ] `crates/ath-cli/src/report.rs` (and supporting modules) - report loading and rendering tests
- [ ] `crates/ath-orchestrator/src/phase_runner.rs` - reviewer token accounting coverage
- [ ] `crates/ath-cli/src/main.rs` - error rendering tests for provider/review/schema failures

*Existing infrastructure covers the rest once these fixtures and test modules exist.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Human-readable report is readable in a real terminal | OUTP-03 | Table width and summary layout are presentation-sensitive | Run `ath report` on a saved run and confirm the phase, review, and cost sections are legible without opening raw JSON |
| Actionable error copy is understandable to users | OUTP-04 | The final composed terminal message is best judged end-to-end | Trigger one provider auth error, one review failure, and one schema/validation error; confirm the output names the source and tells the user what to do next |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
