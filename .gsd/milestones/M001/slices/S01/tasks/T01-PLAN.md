# T01: Plan 01

**Slice:** S01 — **Milestone:** M001

## Description

Create the Cargo workspace scaffold with all 7 ath-* crates, workspace-level dependency management, and minimal compilable stubs for each crate.

Purpose: Every subsequent plan needs a compilable workspace to build against. This establishes the crate layout, dependency graph, and shared dependency versions that all Phase 1 plans (and all future phases) depend on.

Output: A `cargo check --workspace` passing workspace with 7 crates and a binary target named `ath`.
