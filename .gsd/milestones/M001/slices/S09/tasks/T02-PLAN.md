# T02: Plan 02

**Slice:** S09 — **Milestone:** M001

## Description

Finish Athena's token accounting so Phase 9 can report accurate per-phase and per-agent usage totals instead of partial executor-only counts.

Purpose: `QUAL-04` is not satisfied by the current code because reviewer token usage is omitted and phase contributions are aggregated too loosely for report-quality totals.

Output: review-inclusive token accounting in `PhaseRecord`, retry-safe accumulation, and tests proving the saved report input data is complete.
