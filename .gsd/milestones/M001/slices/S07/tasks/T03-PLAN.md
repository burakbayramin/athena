# T03: Plan 03

**Slice:** S07 — **Milestone:** M001

## Description

Build the run_phase orchestration function that drives a single phase through the typestate machine: execute tasks, send for review, handle retry loop, and atomically write files + git commit on success.

Purpose: Implements QUAL-02 (review gate blocks progression) and QUAL-03 (auto-retry with feedback, max 3 attempts) by wiring the typestate machine (Plan 01) with the ReviewEngine (Plan 02) into an async execution loop.
Output: Extended phase_runner.rs with run_phase function and supporting helpers.
