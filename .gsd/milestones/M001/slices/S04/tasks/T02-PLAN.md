# T02: Plan 02

**Slice:** S04 — **Milestone:** M001

## Description

Build the LLM parsing pipeline and spec file reader. Natural language and spec file inputs both flow through a shared parse_to_project_spec function that calls Claude with structured output (JsonSpec), validates the result, and retries with error feedback on failure.

Purpose: Deliver INPT-01 (natural language parsing) and INPT-02 (spec file parsing) as working features.
Output: parse_to_project_spec function, LLM prompt templates, spec file reader, call_provider JsonSpec threading.
