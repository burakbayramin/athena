# Phase 2: Agent Clients - Research

**Researched:** 2026-03-12
**Domain:** Async Rust LLM provider clients with reliability primitives (retry, backoff, circuit breaker)
**Confidence:** HIGH

## Summary

This phase implements a uniform `AgentBackend` trait backed by per-provider tokio actors that call Claude, Gemini, and Codex APIs through the `genai` crate. The reliability layer (exponential backoff, circuit breaker) is built inside each actor using `backon` for retry and hand-rolled circuit breaker state (simple enough to not warrant a library). The `genai` crate v0.5.x already normalizes provider differences (auth, token counting, JSON mode) and supports programmatic API key injection via `AuthResolver`, which maps cleanly to the existing `ConfigStore`.

The actor pattern from Alice Ryhl's canonical guide (mpsc + oneshot per request) is the standard Rust/tokio approach. Each actor owns its `genai::Client`, circuit breaker state, and retry logic. The handle (cloneable sender) is what callers interact with.

**Primary recommendation:** Use `genai 0.5` as the HTTP/provider layer, `backon 1.6` for retry with exponential backoff, `async-trait` for the `AgentBackend` trait (needed for future `dyn` dispatch in Phases 6-7), and tokio mpsc+oneshot for the actor pattern. Hand-roll the circuit breaker -- it's 30 lines of state, not worth a dependency.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Both trait + actor pattern: AgentBackend trait for testability/mocking, real implementations use tokio actor internally
- Per-provider actor with its own bounded mpsc channel (e.g., 32 slots) -- a slow provider doesn't block healthy ones
- Actors spawn at startup for all configured providers -- fail-fast on invalid API keys
- Oneshot channel per request for responses (no streaming in v1)
- MockBackend implements AgentBackend directly for tests (no actor needed)
- Exponential backoff: base 1s, multiplier 2x, max cap 60s, with jitter
- Fixed max 3 retries per request (hardcoded, not configurable)
- Circuit breaker trips after 3 consecutive failures per provider
- Circuit breaker recovery: half-open probe after 30s cooldown -- one probe request, close on success, restart cooldown on failure
- Per-request timeout: 5 minutes (generous for complex code generation prompts)
- Honor Retry-After headers from providers when present, fall back to exponential backoff otherwise
- Use provider JSON mode where available (Claude tool_use, Gemini JSON mode, OpenAI json_object) via genai
- AgentResponse.content stays as String (raw text) -- callers parse into domain types
- Agent layer validates that response is valid JSON when JSON mode was requested
- On JSON parse failure: retry with error context appended to prompt (counts toward 3-retry limit)
- Prompt wrapping fallback for providers without native JSON mode -- transparent to callers
- genai as primary HTTP layer, but structure actor internals so the HTTP client is behind a trait -- reqwest fallback is easy to add without rewriting actors
- Normalized common error enum: RateLimit, AuthFailed, ServerError, Timeout, InvalidResponse, Unknown
- Provider-specific differences handled inside each actor, invisible to callers

### Claude's Discretion
- Exact mpsc channel buffer size per actor
- genai client configuration details
- Internal actor message types beyond AgentRequest/response
- Circuit breaker cooldown duration tuning
- Prompt wrapping strategy for JSON mode fallback

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ORCH-05 | Athena calls Claude, Gemini, and Codex APIs autonomously to execute assigned tasks | genai 0.5 provides unified client for all three providers; actor pattern isolates each; AgentBackend trait provides uniform interface; backoff/circuit-breaker ensure reliability |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| genai | 0.5.x | Multi-provider LLM client (Anthropic, OpenAI, Gemini) | Rust's primary multi-provider crate; normalizes auth, token counting, JSON mode; active maintenance |
| tokio | 1.x | Async runtime, mpsc/oneshot channels, timers | De facto Rust async runtime; already implicit via genai |
| backon | 1.6 | Retry with exponential backoff + jitter | Defaults match spec exactly (1s base, 2x factor, 60s max, 3 retries); clean async API |
| async-trait | 0.1.x | Async methods in traits with dyn dispatch | Needed because Phase 6-7 will use `dyn AgentBackend` for runtime provider selection |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.0 | Agent error enum definition | Already in workspace; use for `AgentError` |
| serde_json | 1.0 | JSON validation of LLM responses | Already in workspace; validate JSON mode responses |
| reqwest | 0.12.x | HTTP client (brought in by genai) | No direct use in v1; genai wraps it |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| backon | tokio-retry2, backoff | backon has most ergonomic API and defaults that exactly match our spec; others require more config |
| async-trait | Native async fn in traits | Native works for static dispatch but Phase 6-7 needs `dyn AgentBackend`; async-trait is the safe choice |
| Hand-rolled circuit breaker | revoke-resilience crate | Circuit breaker is ~30 lines of state; a dependency is overkill for 3-consecutive-failure + cooldown |

**Installation (add to workspace Cargo.toml):**
```toml
[workspace.dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
genai = "0.5"
backon = "1.6"
async-trait = "0.1"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/ath-agents/src/
├── lib.rs              # Public API: AgentBackend trait, AgentError, re-exports
├── backend.rs          # AgentBackend trait definition
├── error.rs            # AgentError enum (normalized across providers)
├── mock.rs             # MockBackend for testing
├── actor/
│   ├── mod.rs          # Shared actor infrastructure (message types, circuit breaker)
│   ├── claude.rs       # ClaudeActor implementation
│   ├── gemini.rs       # GeminiActor implementation
│   └── codex.rs        # CodexActor implementation
└── circuit_breaker.rs  # CircuitBreaker state machine
```

### Pattern 1: Tokio Actor with Handle
**What:** Each provider runs as a spawned tokio task with mpsc receiver. Callers hold a cloneable handle (sender).
**When to use:** Always for real provider implementations.
**Example:**
```rust
// Source: https://ryhl.io/blog/actors-with-tokio/
use tokio::sync::{mpsc, oneshot};

/// Message sent to the actor
struct ActorMessage {
    request: AgentRequest,
    respond_to: oneshot::Sender<Result<AgentResponse, AgentError>>,
}

/// Handle that callers use -- implements AgentBackend
#[derive(Clone)]
pub struct ClaudeHandle {
    sender: mpsc::Sender<ActorMessage>,
}

#[async_trait]
impl AgentBackend for ClaudeHandle {
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ActorMessage { request, respond_to: tx })
            .await
            .map_err(|_| AgentError::ActorStopped)?;
        rx.await.map_err(|_| AgentError::ActorStopped)?
    }
}

/// The actor itself -- owns genai client, circuit breaker, retry logic
struct ClaudeActor {
    receiver: mpsc::Receiver<ActorMessage>,
    client: genai::Client,
    circuit_breaker: CircuitBreaker,
}

impl ClaudeActor {
    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            let result = self.handle_request(msg.request).await;
            let _ = msg.respond_to.send(result);
        }
    }
}
```

### Pattern 2: Circuit Breaker State Machine
**What:** Three states (Closed, Open, HalfOpen) tracking consecutive failures.
**When to use:** Inside each actor to prevent hammering a failing provider.
**Example:**
```rust
use std::time::{Duration, Instant};

pub enum CircuitState {
    Closed { consecutive_failures: u32 },
    Open { opened_at: Instant },
    HalfOpen,
}

pub struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,  // 3
    cooldown: Duration,      // 30s
}

impl CircuitBreaker {
    pub fn can_attempt(&mut self) -> bool {
        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() >= self.cooldown {
                    self.state = CircuitState::HalfOpen;
                    true // allow one probe
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => false, // probe already in flight
        }
    }

    pub fn record_success(&mut self) {
        self.state = CircuitState::Closed { consecutive_failures: 0 };
    }

    pub fn record_failure(&mut self) {
        match &self.state {
            CircuitState::Closed { consecutive_failures } => {
                let new_count = consecutive_failures + 1;
                if new_count >= self.failure_threshold {
                    self.state = CircuitState::Open { opened_at: Instant::now() };
                } else {
                    self.state = CircuitState::Closed { consecutive_failures: new_count };
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open { opened_at: Instant::now() };
            }
            CircuitState::Open { .. } => {} // already open
        }
    }
}
```

### Pattern 3: Retry with backon Inside Actor
**What:** Each LLM call is retried with exponential backoff + jitter inside the actor.
**When to use:** For every provider call that might fail transiently.
**Example:**
```rust
use backon::{ExponentialBuilder, Retryable};
use std::time::Duration;

async fn call_with_retry(
    client: &genai::Client,
    model: &str,
    chat_req: genai::chat::ChatRequest,
) -> Result<genai::chat::ChatResponse, AgentError> {
    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_secs(1))
        .with_max_delay(Duration::from_secs(60))
        .with_factor(2.0)
        .with_jitter()
        .with_max_times(3);

    (|| async {
        client.exec_chat(model, chat_req.clone(), None).await
            .map_err(|e| classify_error(e))
    })
    .retry(backoff)
    .when(|e| e.is_retryable())
    .await
}
```

### Pattern 4: genai AuthResolver from ConfigStore
**What:** Inject API keys from ConfigStore into genai client at startup.
**When to use:** Actor construction.
**Example:**
```rust
use genai::resolver::AuthResolver;
use genai::Client;

fn build_client(config: &ConfigStore) -> Client {
    let anthropic_key = config.anthropic_api_key.clone();
    let google_key = config.google_api_key.clone();
    let openai_key = config.openai_api_key.clone();

    let auth_resolver = AuthResolver::from_resolver_fn(
        move |model_iden| {
            // model_iden.adapter_kind tells us which provider
            // Return AuthData::from_single(key) for the right provider
            // Return Err for missing keys (fail-fast)
            todo!("match on adapter kind, return correct key")
        }
    );

    Client::builder()
        .with_auth_resolver(auth_resolver)
        .build()
}
```

### Anti-Patterns to Avoid
- **Shared mutable state across actors:** Each actor owns its own circuit breaker and client. No Arc/Mutex sharing.
- **Blocking in async context:** genai calls are already async. Never use `std::thread::sleep` -- use `tokio::time::sleep`.
- **Retrying non-retryable errors:** Auth failures (RequiresApiKey, NoAuthData) must NOT be retried. Only retry RateLimit, ServerError, Timeout.
- **Unbounded channels:** Always use bounded mpsc (32 slots is reasonable). Unbounded channels can cause OOM under backpressure.
- **Ignoring Retry-After headers:** The genai `Error::HttpError` includes the response body. Parse Retry-After from response headers when available before falling back to exponential backoff.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-provider LLM HTTP client | Custom reqwest calls per provider | genai 0.5 | Auth, token normalization, JSON mode, model naming -- massive surface area |
| Exponential backoff with jitter | Custom sleep loop | backon 1.6 | Jitter randomization, clean async API, exact defaults match our spec |
| Async trait dispatch | Manual Pin<Box<dyn Future>> | async-trait 0.1 | Boilerplate-heavy, error-prone, no benefit to hand-rolling |
| JSON serialization | Custom parsers | serde_json (in workspace) | Standard Rust ecosystem choice |

**Key insight:** The genai crate eliminates ~80% of provider-specific code. Without it, each provider would need its own HTTP client, auth flow, request/response format, and token counting. The actor layer focuses on reliability (retry, circuit breaker) rather than HTTP plumbing.

## Common Pitfalls

### Pitfall 1: genai JSON Mode Provider Gaps
**What goes wrong:** Not all providers support native JSON mode identically. Claude uses tool_use for structured output, Gemini has `response_mime_type`, OpenAI has `response_format: json_object`.
**Why it happens:** genai's `ChatResponseFormat::JsonMode` and `ChatResponseFormat::JsonSpec` are silently ignored by unsupported providers.
**How to avoid:** Always validate response is valid JSON after the call. On parse failure, retry with error context in prompt. The `with_response_format()` on ChatOptions sets the hint, but validation is the caller's responsibility.
**Warning signs:** Provider returns prose instead of JSON with no error.

### Pitfall 2: genai Error Classification
**What goes wrong:** genai's `Error::HttpError` bundles all HTTP errors (429, 500, 401, etc.) into one variant with a `StatusCode` field. You must match on the status code to classify.
**Why it happens:** genai doesn't pre-classify errors by retryability.
**How to avoid:** Build a `classify_error(genai::Error) -> AgentError` function that maps: 429 -> RateLimit, 5xx -> ServerError, 401/403 -> AuthFailed, timeout -> Timeout, JSON parse -> InvalidResponse, everything else -> Unknown.
**Warning signs:** All errors being retried or no errors being retried.

### Pitfall 3: Retry-After Header Not in genai Error
**What goes wrong:** The `Error::HttpError` variant has `body: String` but not raw headers. Retry-After may not be accessible.
**Why it happens:** genai abstracts away HTTP response details.
**How to avoid:** Check if genai exposes headers. If not, fall back to exponential backoff for all retries. This is acceptable -- Retry-After is an optimization, not a requirement.
**Warning signs:** Implementing complex header parsing when backoff already works.

### Pitfall 4: Actor Shutdown Without Draining
**What goes wrong:** Dropping the handle (sender) immediately stops the actor, but in-flight requests get lost.
**Why it happens:** mpsc receiver returns None when all senders drop.
**How to avoid:** The actor loop processes all remaining messages before shutting down. The `while let Some(msg) = receiver.recv().await` pattern already handles this -- it drains the channel before returning None.
**Warning signs:** Tests that drop handles and expect responses to complete.

### Pitfall 5: Circuit Breaker Blocking Startup Validation
**What goes wrong:** If startup key validation fails 3 times, the circuit breaker trips before the system even starts.
**Why it happens:** Mixing startup validation with runtime circuit breaker logic.
**How to avoid:** Startup key validation should be a separate one-shot check (simple API call), not routed through the circuit breaker. Circuit breaker protects runtime traffic only.
**Warning signs:** Circuit breaker already open at first real request.

### Pitfall 6: ChatRequest Clone for Retry
**What goes wrong:** `ChatRequest` may not implement `Clone`, breaking retry loops that need to resend.
**Why it happens:** genai types may not derive Clone.
**How to avoid:** Check if ChatRequest is Clone. If not, reconstruct from stored prompt/context on each retry attempt rather than cloning the request.
**Warning signs:** Compile error on retry loop.

## Code Examples

### AgentBackend Trait Definition
```rust
// Source: project design decision + async-trait docs
use async_trait::async_trait;
use ath_types::agent::{AgentRequest, AgentResponse};

#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Send a request to the agent and wait for a response.
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;

    /// Check if the backend is currently available (circuit breaker not tripped).
    async fn is_available(&self) -> bool;

    /// Provider name for logging/error reporting.
    fn provider_name(&self) -> &str;
}
```

### AgentError Enum
```rust
// Source: CONTEXT.md locked decision on normalized error enum
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Rate limited by {provider}")]
    RateLimit { provider: String, retry_after: Option<std::time::Duration> },

    #[error("Authentication failed for {provider}: {reason}")]
    AuthFailed { provider: String, reason: String },

    #[error("Server error from {provider}: {status}")]
    ServerError { provider: String, status: u16 },

    #[error("Request to {provider} timed out after {duration:?}")]
    Timeout { provider: String, duration: std::time::Duration },

    #[error("Invalid response from {provider}: {reason}")]
    InvalidResponse { provider: String, reason: String },

    #[error("Circuit breaker open for {provider}")]
    CircuitOpen { provider: String },

    #[error("Agent actor has stopped")]
    ActorStopped,

    #[error("Unknown error from {provider}: {source}")]
    Unknown { provider: String, source: String },
}

impl AgentError {
    /// Whether this error should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        matches!(self, AgentError::RateLimit { .. }
                     | AgentError::ServerError { .. }
                     | AgentError::Timeout { .. }
                     | AgentError::InvalidResponse { .. })
    }
}
```

### MockBackend for Testing
```rust
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockBackend {
    responses: Arc<Mutex<Vec<Result<AgentResponse, AgentError>>>>,
}

impl MockBackend {
    pub fn new(responses: Vec<Result<AgentResponse, AgentError>>) -> Self {
        Self { responses: Arc::new(Mutex::new(responses)) }
    }

    pub fn always_ok(content: &str) -> Self {
        // Returns a mock that always succeeds with given content
        todo!()
    }

    pub fn failing(error: AgentError) -> Self {
        // Returns a mock that always fails with given error
        todo!()
    }
}

#[async_trait]
impl AgentBackend for MockBackend {
    async fn send(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!("MockBackend: no more responses queued");
        }
        responses.remove(0)
    }

    async fn is_available(&self) -> bool { true }
    fn provider_name(&self) -> &str { "mock" }
}
```

### genai Client Initialization
```rust
use genai::Client;
use genai::resolver::AuthResolver;
use ath_config::ConfigStore;

fn build_genai_client(config: &ConfigStore) -> Result<Client, AgentError> {
    let ant_key = config.anthropic_api_key.clone();
    let goog_key = config.google_api_key.clone();
    let oai_key = config.openai_api_key.clone();

    let auth_resolver = AuthResolver::from_resolver_fn(move |model_iden| {
        // model_iden contains adapter_kind for provider routing
        // Return AuthData::from_single(key) for the matching provider
        // This is where ConfigStore keys get injected into genai
        todo!("implement per-provider key resolution")
    });

    let client = Client::builder()
        .with_auth_resolver(auth_resolver)
        .build();

    Ok(client)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-provider HTTP clients (reqwest) | genai unified client | genai 0.5 (Jan 2026) | Eliminates provider-specific HTTP code |
| `async-trait` crate always | Native async fn in traits | Rust 1.75 (Dec 2023) | Native works for static dispatch; async-trait still needed for dyn |
| Manual retry loops | backon / tokio-retry2 | backon 1.0 (2024) | Declarative retry with backoff strategies |
| genai `with_json_mode()` | `with_response_format(ChatResponseFormat::JsonMode)` | genai 0.5 | Old method deprecated |

**Deprecated/outdated:**
- `genai::ChatOptions::with_json_mode()` -- deprecated in favor of `with_response_format()`
- Direct `reqwest` calls per provider -- genai handles this; only fall back if genai has a critical bug

## Open Questions

1. **Does genai::ChatRequest implement Clone?**
   - What we know: genai types typically derive standard traits, but ChatRequest contains Vec<ChatMessage> which may or may not be Clone
   - What's unclear: Cannot verify without compiling
   - Recommendation: Check at implementation time. If not Clone, store prompt/context separately and reconstruct on retry.

2. **How does genai expose Retry-After headers?**
   - What we know: `Error::HttpError` has `status`, `canonical_reason`, `body` but NOT headers
   - What's unclear: Whether the body or another error variant contains retry-after info
   - Recommendation: Implement exponential backoff as primary strategy. If Retry-After is accessible, use it as an optimization. Don't block on this.

3. **genai AdapterKind enum values for provider matching in AuthResolver**
   - What we know: AuthResolver receives `ModelIden` which contains adapter kind
   - What's unclear: Exact enum variant names for Anthropic/Google/OpenAI
   - Recommendation: Check genai docs.rs for `AdapterKind` enum at implementation time. Model name prefixing (e.g., "anthropic::claude-3-opus") may also work.

4. **genai model name format**
   - What we know: genai supports namespace prefixing like `"provider::model"`
   - What's unclear: Whether Claude needs `"anthropic::claude-opus-4"` or just `"claude-opus-4"`
   - Recommendation: Check genai examples at implementation time. The model names in ConfigStore (e.g., "opus-4") may need mapping.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[cfg(test)]` + `#[tokio::test]`) |
| Config file | None needed -- Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test -p ath-agents` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ORCH-05a | Claude send/receive via MockBackend | unit | `cargo test -p ath-agents -- claude` | No -- Wave 0 |
| ORCH-05b | Gemini send/receive via MockBackend | unit | `cargo test -p ath-agents -- gemini` | No -- Wave 0 |
| ORCH-05c | Codex send/receive via MockBackend | unit | `cargo test -p ath-agents -- codex` | No -- Wave 0 |
| ORCH-05d | Exponential backoff on 429/500 | unit | `cargo test -p ath-agents -- backoff` | No -- Wave 0 |
| ORCH-05e | Circuit breaker trips after 3 failures | unit | `cargo test -p ath-agents -- circuit` | No -- Wave 0 |
| ORCH-05f | Circuit breaker half-open probe after cooldown | unit | `cargo test -p ath-agents -- half_open` | No -- Wave 0 |
| ORCH-05g | AgentError classification from genai errors | unit | `cargo test -p ath-agents -- classify` | No -- Wave 0 |
| ORCH-05h | JSON validation + retry with error context | unit | `cargo test -p ath-agents -- json_valid` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-agents`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/ath-agents/src/circuit_breaker.rs` tests -- covers ORCH-05e, ORCH-05f
- [ ] `crates/ath-agents/src/error.rs` tests -- covers ORCH-05g
- [ ] `crates/ath-agents/src/mock.rs` tests -- covers ORCH-05a/b/c
- [ ] Add `tokio` with `test-util` feature to `[dev-dependencies]` for time manipulation in circuit breaker tests
- [ ] Add `backon` to workspace dependencies

## Sources

### Primary (HIGH confidence)
- [genai docs.rs](https://docs.rs/genai/latest/genai/) -- Client, ChatOptions, ChatResponse, ChatResponseFormat, Error enum, Usage struct, AuthResolver
- [genai GitHub](https://github.com/jeremychone/rust-genai) -- v0.5.x features, AuthResolver example (c02-auth.rs)
- [backon docs.rs](https://docs.rs/backon/latest/backon/) -- ExponentialBuilder API, defaults verified (1s/2x/60s/3 retries)
- [Alice Ryhl: Actors with Tokio](https://ryhl.io/blog/actors-with-tokio/) -- Canonical actor pattern (mpsc + oneshot + handle)
- [tokio mpsc docs](https://docs.rs/tokio/latest/tokio/sync/mpsc/) -- Bounded channel semantics

### Secondary (MEDIUM confidence)
- [Rust Blog: async fn in traits](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html) -- Native async traits stable since 1.75, no dyn support
- [genai ChatOptions](https://docs.rs/genai/latest/genai/chat/struct.ChatOptions.html) -- response_format field, deprecated json_mode

### Tertiary (LOW confidence)
- Retry-After header availability in genai errors -- could not verify; only `HttpError { status, body, canonical_reason }` documented
- ChatRequest Clone derivation -- could not verify without compilation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- genai, backon, tokio, async-trait are well-documented with verified APIs
- Architecture: HIGH -- actor pattern is canonical Rust/tokio; circuit breaker is standard state machine
- Pitfalls: MEDIUM -- genai error classification verified from docs.rs; Retry-After and Clone status unverified

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (genai 0.5 is stable; backon 1.6 is stable)