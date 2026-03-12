# Phase 2: Agent Clients - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Athena can call Claude, Gemini, and Codex APIs reliably — with retry, backoff, and circuit-breaker behavior — through a uniform trait interface. Delivers: AgentBackend trait, actor-based provider implementations (Claude, Gemini, Codex), reliability primitives (retry, backoff, circuit breaker), and mock backend for testing. No orchestration logic, no task routing, no file isolation.

</domain>

<decisions>
## Implementation Decisions

### Concurrency Model
- Both trait + actor pattern: AgentBackend trait for testability/mocking, real implementations use tokio actor internally
- Per-provider actor with its own bounded mpsc channel (e.g., 32 slots) — a slow provider doesn't block healthy ones
- Actors spawn at startup for all configured providers — fail-fast on invalid API keys
- Oneshot channel per request for responses (no streaming in v1)
- MockBackend implements AgentBackend directly for tests (no actor needed)

### Reliability Tuning
- Exponential backoff: base 1s, multiplier 2x, max cap 60s, with jitter
- Fixed max 3 retries per request (hardcoded, not configurable)
- Circuit breaker trips after 3 consecutive failures per provider
- Circuit breaker recovery: half-open probe after 30s cooldown — one probe request, close on success, restart cooldown on failure
- Per-request timeout: 5 minutes (generous for complex code generation prompts)
- Honor Retry-After headers from providers when present, fall back to exponential backoff otherwise

### Structured Output
- Use provider JSON mode where available (Claude tool_use, Gemini JSON mode, OpenAI json_object) via genai
- AgentResponse.content stays as String (raw text) — callers parse into domain types
- Agent layer validates that response is valid JSON when JSON mode was requested
- On JSON parse failure: retry with error context appended to prompt (counts toward 3-retry limit)
- Prompt wrapping fallback for providers without native JSON mode — transparent to callers

### Provider Quirks
- genai as primary HTTP layer, but structure actor internals so the HTTP client is behind a trait — reqwest fallback is easy to add without rewriting actors
- Normalized common error enum: RateLimit, AuthFailed, ServerError, Timeout, InvalidResponse, Unknown — callers don't handle per-provider variants
- Provider-specific differences (JSON mode gaps, response format) handled inside each actor, invisible to callers

### Claude's Discretion
- Exact mpsc channel buffer size per actor
- genai client configuration details
- Internal actor message types beyond AgentRequest/response
- Circuit breaker cooldown duration tuning
- Prompt wrapping strategy for JSON mode fallback

</decisions>

<specifics>
## Specific Ideas

- The trait + actor split mirrors how real service clients work — lightweight handle for callers, stateful actor for reliability logic
- "Fail-fast at startup" is important — don't discover a bad API key mid-execution when Phase 7 needs all three providers
- Retry-with-error-context for JSON failures is a proven pattern with LLMs — the model almost always fixes the output on the second try

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AgentKind` enum (ath-types/src/agent.rs): Type-safe agent identification with model variant — use for actor routing
- `AgentRequest` / `AgentResponse` (ath-types/src/agent.rs): Already defined with uuid, timestamps, token counts — no schema changes needed
- `ConfigStore` (ath-config/src/store.rs): Has API keys + model defaults per provider — actors read from this at startup
- `ValidationError` (ath-types/src/error.rs): Pattern for errors with fix hints — extend for agent-specific errors

### Established Patterns
- `thiserror` for domain errors, `anyhow` at binary boundary
- All types derive Debug, Clone, Serialize, Deserialize, PartialEq
- `validate()` returns `Result<(), ValidationError>` with fix hints
- Workspace-level dependency management in root Cargo.toml

### Integration Points
- ath-agents crate (stub exists) — this is where all new code goes
- ath-agents depends on ath-types (already in Cargo.toml) and ath-config (needs adding)
- Will need tokio, genai, and async-trait as new workspace dependencies
- Downstream: Phase 6 (Module Isolation) routes tasks to these actors, Phase 7 (Phase Runner) dispatches through AgentBackend trait

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-agent-clients*
*Context gathered: 2026-03-12*
