# S04: Generic OpenAI-Compatible Provider

**Goal:** Any OpenAI-compatible endpoint (Ollama, Groq, Together, Mistral, etc.) can be used as an agent backend via `.ath/agents.toml` config.
**Demo:** A config entry with `provider = "ollama"` and `base_url = "http://localhost:11434"` creates a working agent backend.

## Must-Haves

- Generic handle that works with any OpenAI-compatible API endpoint
- Takes base_url, model, api_key_env from AgentConfig
- Uses genai's OpenAI adapter with custom base URL
- Wired into build_backend_for_agent() for non-builtin providers
- Unit tests for handle construction and error cases

## Verification

- `cargo test --workspace` — 658+ tests pass, 0 failures
- `cargo test -p ath-agents -- generic` — new generic handle tests pass

## Tasks

- [x] **T01: Implement GenericHandle with custom base_url support** `est:30m`
  - Why: Custom providers need a backend that talks to any OpenAI-compatible endpoint
  - Files: `crates/ath-agents/src/actor/generic.rs` (new), `crates/ath-agents/src/actor/mod.rs`, `crates/ath-agents/src/lib.rs`
  - Do: Create `GenericHandle` similar to existing handles but parameterized: provider name, model, API key, base_url. Build genai Client with custom AuthResolver that uses the specific key. Provider name injected for error classification. Register in actor/mod.rs exports.
  - Verify: `cargo test -p ath-agents -- generic`
  - Done when: GenericHandle constructs, implements AgentBackend, handles missing key errors

- [x] **T02: Wire GenericHandle into build_backend_for_agent** `est:10m`
  - Why: The run.rs fallthrough for custom providers currently prints a warning — replace with GenericHandle
  - Files: `crates/ath-cli/src/run.rs`
  - Do: In `build_backend_for_agent()`, replace the custom provider warning branch with GenericHandle construction. Pass provider, model, api_key, base_url from AgentConfig.
  - Verify: `cargo test --workspace`
  - Done when: Custom provider entries in agents.toml create functional backends

## Files Likely Touched

- `crates/ath-agents/src/actor/generic.rs` (new)
- `crates/ath-agents/src/actor/mod.rs`
- `crates/ath-agents/src/lib.rs`
- `crates/ath-cli/src/run.rs`
