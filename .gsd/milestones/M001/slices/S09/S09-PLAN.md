# S09: Reporting And Error Quality

**Goal:** Introduce Athena's durable run-report artifact so every successful run leaves behind structured output that `ath report` can load later.
**Demo:** Introduce Athena's durable run-report artifact so every successful run leaves behind structured output that `ath report` can load later.

## Must-Haves


## Tasks

- [x] **T01: Plan 01**
  - Introduce Athena's durable run-report artifact so every successful run leaves behind structured output that `ath report` can load later.

Purpose: Phase 9 cannot satisfy `OUTP-03` until `ath run` writes a saved report artifact that preserves both the routed plan metadata and the completed execution/review outcomes.

Output: typed run-report schemas, latest-run lookup helpers, and successful-run persistence wiring in `ath-cli`.
- [x] **T02: Plan 02**
  - Finish Athena's token accounting so Phase 9 can report accurate per-phase and per-agent usage totals instead of partial executor-only counts.

Purpose: `QUAL-04` is not satisfied by the current code because reviewer token usage is omitted and phase contributions are aggregated too loosely for report-quality totals.

Output: review-inclusive token accounting in `PhaseRecord`, retry-safe accumulation, and tests proving the saved report input data is complete.
- [x] **T03: Plan 03**
  - Add real token-cost estimation so Athena's saved reports and `ath report` output can show meaningful usage totals per agent, per phase, and for the whole run.

Purpose: `QUAL-04` requires more than raw token counts. The saved report must turn those counts into estimated dollar cost using known provider/model pricing, while failing honestly when pricing is unavailable.

Output: pricing helpers, report aggregation totals, and human-readable cost sections on top of the persisted run-report artifact.
- [x] **T04: Plan 04**
  - Upgrade Athena's terminal error output so provider, review, and schema failures tell users what failed and what to do next without reading source code.

Purpose: `OUTP-04` is only satisfied if the CLI preserves the rich context already present in typed errors and fills in the missing reviewer/raw-value details where the current error variants are still too weak.

Output: richer typed error data where necessary, plus centralized actionable error rendering in `ath-cli`.

## Files Likely Touched

