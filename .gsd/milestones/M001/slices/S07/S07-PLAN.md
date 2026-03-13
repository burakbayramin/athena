# S07: Phase Runner And Review

**Goal:** Create the PhaseState typestate machine and supporting error types for the phase runner.
**Demo:** Create the PhaseState typestate machine and supporting error types for the phase runner.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Create the PhaseState typestate machine and supporting error types for the phase runner.

Purpose: The typestate pattern makes invalid state transitions compile-time errors. This is the foundation for QUAL-02 (review gate blocks progression) -- the type system itself enforces that no code path can bypass review.
Output: phase_runner.rs with typestate machine, extended error.rs with PhaseRunnerError, updated Cargo.toml with async dependencies.
- [x] **T02: Plan 02**
  - Build the ReviewEngine module -- reviewer selection with cross-agent pairing, review prompt construction, and structured verdict parsing.

Purpose: Implements QUAL-01 (cross-agent review where reviewer != author). The reviewer selection logic enforces the never-same-as-author rule, handles circuit breaker fallback, and uses priority tiebreaking (Claude > Gemini > Codex).
Output: review.rs with all review orchestration functions.
- [x] **T03: Plan 03**
  - Build the run_phase orchestration function that drives a single phase through the typestate machine: execute tasks, send for review, handle retry loop, and atomically write files + git commit on success.

Purpose: Implements QUAL-02 (review gate blocks progression) and QUAL-03 (auto-retry with feedback, max 3 attempts) by wiring the typestate machine (Plan 01) with the ReviewEngine (Plan 02) into an async execution loop.
Output: Extended phase_runner.rs with run_phase function and supporting helpers.
- [x] **T04: Plan 04**
  - Build the AgentCoordinator that drives a full ExecutionPlan through sequential phase dispatch, and integration tests proving the end-to-end pipeline works.

Purpose: This is the top-level entry point that proves QUAL-01 (cross-review), QUAL-02 (review gate), and QUAL-03 (retry with feedback) work together in a real execution flow. Success criteria #5 from the roadmap: "A complete sequential run completes successfully."
Output: coordinator.rs with AgentCoordinator, integration tests for the full pipeline.

## Files Likely Touched

