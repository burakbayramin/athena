# T03: Plan 03

**Slice:** S04 — **Milestone:** M001

## Description

Build the codebase scanner and wire all three input modes through the CLI. Codebase analysis uses the `ignore` crate for gitignore-respecting traversal, identifies key files, and sends tree + contents to the LLM. The CLI is updated to run the full input parsing pipeline for all modes and display a ProjectSpec summary.

Purpose: Deliver INPT-03 (codebase analysis) and complete the full input parsing pipeline.
Output: Codebase scanner, full parse_input function, CLI integration with ProjectSpec summary display.
