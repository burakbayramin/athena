# T04: Plan 04

**Slice:** S08 — **Milestone:** M001

## Description

Implement an honest dry-run path that never spends API or git work, while still printing the full routed execution plan when local planning data exists.

Purpose: `PLAN-05` is only satisfied if `--dry-run` is cost-free. Since the current planning path is LLM-backed, dry-run needs a local serialized plan artifact and a clear failure mode when that artifact is absent.

Output: `dry_run.rs`, local serialized plan-cache helpers, assigned-agent-aware plan display, and tests proving no agent/git execution occurs in dry-run mode.
