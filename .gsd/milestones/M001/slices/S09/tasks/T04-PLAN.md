# T04: Plan 04

**Slice:** S09 — **Milestone:** M001

## Description

Upgrade Athena's terminal error output so provider, review, and schema failures tell users what failed and what to do next without reading source code.

Purpose: `OUTP-04` is only satisfied if the CLI preserves the rich context already present in typed errors and fills in the missing reviewer/raw-value details where the current error variants are still too weak.

Output: richer typed error data where necessary, plus centralized actionable error rendering in `ath-cli`.
