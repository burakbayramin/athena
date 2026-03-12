---
phase: 4
slug: input-parsing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p ath-types -p ath-agents -p ath-cli --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p ath-types -p ath-agents -p ath-cli --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | INPT-01 | unit | `cargo test -p ath-cli test_run_subcommand` | ❌ W0 | ⬜ pending |
| 04-02-01 | 02 | 1 | INPT-01 | unit | `cargo test -p ath-types test_project_spec` | ❌ W0 | ⬜ pending |
| 04-02-02 | 02 | 1 | INPT-01 | integration | `cargo test -p ath-agents test_nl_to_spec` | ❌ W0 | ⬜ pending |
| 04-03-01 | 03 | 2 | INPT-02 | unit | `cargo test -p ath-types test_spec_file_parse` | ❌ W0 | ⬜ pending |
| 04-04-01 | 04 | 2 | INPT-03 | unit | `cargo test -p ath-cli test_codebase_scan` | ❌ W0 | ⬜ pending |
| 04-05-01 | 05 | 3 | INPT-01,02,03 | unit | `cargo test -p ath-types test_spec_validation` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `ath-types/src/spec.rs` — ProjectSpec struct with serde Deserialize + validate()
- [ ] `ath-cli/tests/run_cmd.rs` — test stubs for run subcommand input modes
- [ ] `ath-agents/tests/nl_parse.rs` — test stubs for NL-to-ProjectSpec extraction

*Existing test infrastructure (cargo test) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| LLM output quality | INPT-01 | Non-deterministic LLM responses | Run `ath run "build a todo app"` — verify ProjectSpec has sensible goals/constraints |
| Codebase scan on real project | INPT-03 | Requires real file tree | Run `ath run --codebase ./fixtures/sample-project` — verify meaningful summary |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
