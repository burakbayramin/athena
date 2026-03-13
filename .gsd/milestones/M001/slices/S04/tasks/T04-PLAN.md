# T04: Plan 04

**Slice:** S04 — **Milestone:** M001

## Description

Wire the full input parsing pipeline through the CLI so that `ath run` actually produces a ProjectSpec.

Purpose: Close the gap identified in 04-VERIFICATION.md where the CLI resolves the input mode but never calls `parse_input` or constructs a real AgentBackend. ROADMAP Success Criteria 1-3 for Phase 4 require the user to run `ath run` and get a ProjectSpec -- currently the CLI stops at mode resolution.

Output: A working CLI that constructs a ClaudeHandle from ConfigStore, calls parse_input with the resolved InputMode, and displays the resulting ProjectSpec summary.
