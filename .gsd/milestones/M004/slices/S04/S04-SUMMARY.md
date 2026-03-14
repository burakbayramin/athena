---
id: S04
parent: M004
milestone: M004
provides:
  - GenericHandle for any OpenAI-compatible endpoint
  - Custom base_url support via genai ServiceTargetResolver
  - Optional API key (Ollama doesn't need one)
  - Wired into build_backend_for_agent for all non-builtin providers
requires:
  - S01
  - S02
affects:
  - S05
key_files:
  - crates/ath-agents/src/actor/generic.rs
  - crates/ath-cli/src/run.rs
key_decisions:
  - "D035: Custom base_url uses OpenAI adapter kind — all OpenAI-compatible endpoints share the same protocol"
  - "D036: GenericHandle accepts optional API key — Ollama and other local models may not require auth"
drill_down_paths:
  - .gsd/milestones/M004/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S04/tasks/T02-SUMMARY.md
duration: 15m
verification_result: passed
completed_at: 2026-03-14
---

# S04: Generic OpenAI-Compatible Provider

**Any OpenAI-compatible endpoint (Ollama, Groq, Together, etc.) now works as an agent backend. 663 tests pass (5 new).**

## What Happened

**T01** created `GenericHandle` — a provider-agnostic actor that uses genai's `ServiceTargetResolver` to route to custom endpoints and `AuthResolver` for auth injection. Works with or without API key (Ollama), with or without custom base_url (Groq uses standard genai routing).

**T02** replaced the placeholder warning in `build_backend_for_agent()` with GenericHandle construction — any non-builtin provider in `.ath/agents.toml` now creates a functional backend.

## Verification

- `cargo test --workspace` — 663 passed, 0 failed
- GenericHandle constructs successfully with various config combinations

## Forward Intelligence

- S05 will add CLI commands to list/test agents, including generic ones
- The `api_key` is resolved at handle construction time from the env var name in config
