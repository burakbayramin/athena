# T03: Plan 03

**Slice:** S09 — **Milestone:** M001

## Description

Add real token-cost estimation so Athena's saved reports and `ath report` output can show meaningful usage totals per agent, per phase, and for the whole run.

Purpose: `QUAL-04` requires more than raw token counts. The saved report must turn those counts into estimated dollar cost using known provider/model pricing, while failing honestly when pricing is unavailable.

Output: pricing helpers, report aggregation totals, and human-readable cost sections on top of the persisted run-report artifact.
