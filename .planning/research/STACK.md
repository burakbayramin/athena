# Stack Research

**Domain:** Multi-agent AI orchestration CLI (Rust)
**Researched:** 2026-03-12
**Confidence:** HIGH (core stack) / MEDIUM (AI-specific crates)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tokio | 1.x (1.50 current) | Async runtime for concurrent agent execution | The de-facto standard async runtime. Work-stealing scheduler handles I/O-bound tasks (HTTP calls to 3 AI APIs) efficiently. Entire ecosystem (reqwest, tracing, git2 async) builds on top of it. No serious competitor for multi-task orchestration. |
| clap | 4.5 (4.5.60 current) | CLI argument parsing with subcommands, flags, env var integration | v4 with derive macros is the industry standard. Supports Unix conventions, auto-generated help, shell completions, and type-safe argument structs. The `derive` feature eliminates boilerplate. Only alternative worth considering (lexopt) lacks help generation needed for a user-facing tool. |
| genai | 0.5.x (0.5.3 current) | Unified multi-provider AI client (Claude, Gemini, OpenAI) | Single crate with native adapters for all three required providers. Avoids maintaining three separate SDK integrations. Supports streaming, reasoning/thinking controls, custom auth/endpoint overrides. Uses reqwest 0.13 internally. Actively maintained (v0.5.0 released Jan 2026). |
| serde + serde_json | 1.x (serde_json 1.0.149) | Serialization for AI request/response structs, config, phase plans | Ubiquitous Rust serialization framework. Required for parsing AI API JSON responses, serializing phase plans to disk, and config files. Every alternative is marginal in comparison to ecosystem depth. |
| git2 | 0.20.x (0.20.2 current) | Programmatic git operations: stage, commit, branch per phase | libgit2 bindings — the only mature option for in-process git ops without shelling out. Supports all operations Athena needs: init repo, stage files (index manipulation), create commits, manage branches. Bundled libgit2 means no system dependency. |
| tracing + tracing-subscriber | 0.1.x | Structured async-aware logging and diagnostics | Designed for async Tokio applications. Spans capture task context across await points, so you can trace which agent produced which output. Integrates with indicatif for progress bars without terminal fighting. Superior to the `log` crate for multi-task orchestration. |
| anyhow | 1.x | Application-level error aggregation | CLI application errors should display cleanly, not match on enum variants. `anyhow` provides ergonomic `?` propagation with `.context()` for descriptive error chains. Correct choice at the binary level (vs. `thiserror` which belongs in libraries). |
| thiserror | 2.x | Typed errors for internal library modules | Phase orchestration, agent communication, git operations each need domain-specific error types that calling code can match on. Use `thiserror` in internal modules; `anyhow` at the top-level binary. Both are complementary. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest | 0.13.x | HTTP client (used via genai; may need direct use for custom endpoints) | Use directly only if genai's adapter doesn't cover a provider-specific feature (e.g., custom streaming endpoints, function calling with non-standard schemas). reqwest 0.13 uses rustls by default — no OpenSSL dependency. |
| dotenvy | 0.15.x | Load API keys from `.env` file for development | Always include for dev ergonomics. Athena requires `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `OPENAI_API_KEY` — dotenvy loads these from `.env` before clap processes them. Pass keys through clap's `env()` attribute for clean integration. |
| indicatif | 0.17.x | Terminal progress bars and spinners during agent execution | Use for per-agent progress display during parallel phase execution. Integrate via `tracing-indicatif` to avoid tracing logs and progress bars fighting for terminal. Multi-bar support shows all three agents simultaneously. |
| console | 0.15.x | Terminal color and styling (dependency of indicatif, usable standalone) | Use for colored status output: phase summaries, review pass/fail badges, agent assignment tables. Already pulled in via indicatif — no extra dep cost. |
| tokio-util | 0.7.x | Stream utilities, codec framing for SSE streaming responses | Required for consuming Server-Sent Events (SSE) streams from AI APIs when using streaming completions. `genai` handles this internally, but useful if implementing custom streaming. |
| futures | 0.3.x | `join_all`, `select`, stream combinators for parallel agent tasks | Use `futures::future::join_all` to fan out concurrent API calls to all three agents. `FuturesUnordered` for collecting results as they complete rather than waiting for all. |
| uuid | 1.x | Generate unique identifiers for phases, tasks, agent sessions | Each phase/task needs a stable ID for tracking, logging correlation, and git commit metadata. |
| chrono | 0.4.x | Timestamps for phase execution tracking and report generation | Git commits and structured reports need ISO 8601 timestamps. `chrono` with `serde` feature for serialization. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-watch | Auto-rebuild on file changes during development | `cargo install cargo-watch`, run as `cargo watch -x run`. Essential for tight iteration loop. |
| cargo-nextest | Faster test runner with better output for parallel tests | `cargo install cargo-nextest`. Especially useful when testing async agent orchestration with many unit tests. |
| cargo-dist | Cross-platform binary distribution (Linux, macOS, Windows) | Athena targets single-binary distribution. `cargo-dist` generates GitHub Actions workflows + installers. Use when ready to ship. |
| rust-analyzer | LSP server for IDE support | Standard. Enables inline type hints critical for async code with complex Future types. |

---

## Installation

```toml
# Cargo.toml

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive", "env"] }

# AI providers (all three: Claude, Gemini, OpenAI/Codex)
genai = "0.5"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Git integration
git2 = "0.20"

# Logging / tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

# Error handling
anyhow = "1"
thiserror = "2"

# Environment / config
dotenvy = "0.15"

# Terminal UI
indicatif = "0.17"
console = "0.15"

# Async combinators
futures = "0.3"

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

```bash
# Create new binary crate
cargo new athena --bin
cd athena

# Add all dependencies at once
cargo add tokio --features full
cargo add clap --features derive,env
cargo add genai
cargo add serde --features derive
cargo add serde_json
cargo add git2
cargo add tracing
cargo add tracing-subscriber --features env-filter,fmt
cargo add anyhow
cargo add thiserror
cargo add dotenvy
cargo add indicatif
cargo add console
cargo add futures
cargo add uuid --features v4
cargo add chrono --features serde
```

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Async runtime | tokio | async-std | tokio has significantly larger ecosystem; genai, reqwest, tracing all target tokio. async-std lacks critical ecosystem crates. |
| Async runtime | tokio | smol | smol is minimal/embedded-focused. Not suitable for orchestrating I/O-heavy multi-agent workflows with SSE streaming. |
| CLI parsing | clap 4 | argh | argh follows Fuchsia OS conventions (not Unix). Missing `--help` subcommand delegation, shell completions, env var integration. Wrong for user-facing tool. |
| CLI parsing | clap 4 | lexopt | Zero-dependency but zero-feature. No help generation, no derive macros, no env vars. Would require manual implementation of everything clap provides. |
| AI client | genai | Per-provider SDKs (anthropic-rs, async-openai) | Three separate SDKs with different APIs, auth patterns, and streaming models. genai unifies all three with one ergonomic interface. Use individual SDKs only if provider-specific features outpace genai. |
| AI client | genai | Raw reqwest | Requires hand-rolling JSON schemas for each provider's API, manually handling SSE streaming, implementing rate limiting. High implementation cost for no gain. |
| Git integration | git2 | Shelling out to `git` CLI | Process spawn has overhead, requires git installed on PATH, harder to test, error handling is string-parsing. git2 is in-process, no system dependency, typed errors. |
| Git integration | git2 | gitoxide (gix) | gitoxide is newer and pure-Rust but its API is less stable and less documented. git2 (libgit2 bindings) is battle-tested across thousands of projects. |
| Logging | tracing | log + env_logger | `log` crate has no concept of spans — cannot track which async task (agent) produced a log entry. In concurrent agent execution this makes logs unreadable. `tracing` was designed for async. |
| Error handling | anyhow + thiserror | eyre | eyre is an anyhow fork with richer error reporting. Reasonable alternative. anyhow is more widely used with better ecosystem integration. |
| Config | dotenvy + clap env() | figment | figment is powerful for layered config but overengineered for Athena v1. Three API keys loaded from env vars is the entire config surface. Figment adds complexity without benefit at this scope. |
| Serialization | serde | nanoserde | nanoserde trades ecosystem compatibility for compile speed. Incompatible with genai and reqwest's serde expectations. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `dotenv` crate (original) | Unmaintained since June 2020, known security advisory RUSTSEC-2021-0141 | `dotenvy` — the actively maintained fork |
| `structopt` | Merged into clap v3+, no longer developed separately | `clap 4` with `derive` feature |
| `openssl` feature in reqwest | Requires system OpenSSL installation — breaks cross-compilation and increases binary size | Leave reqwest 0.13 at its default (rustls) — no system dep, ships in binary |
| `async-openai` as sole provider | OpenAI-only; Athena requires Claude + Gemini too. You'll still need separate Anthropic + Gemini integration | `genai` for unified multi-provider access |
| `std::thread::spawn` for agent parallelism | OS threads are wasteful for I/O-bound AI API calls. 3 agents each making HTTP calls blocks threads for seconds | `tokio::spawn` — tasks are lightweight (64 bytes vs ~MB stack), thousands per runtime |
| `panic!` / `unwrap()` in orchestration paths | Silent crashes during autonomous multi-hour execution lose all state and progress | `anyhow::Result` with `.context()` throughout; reserve `expect()` only for provably-safe invariants |
| `log` + `env_logger` | Cannot track which agent produced a log line in concurrent execution — logs from 3 parallel tasks are interleaved without context | `tracing` with task-specific spans; each agent gets a span with `agent = "claude"` etc. |

---

## Stack Patterns by Variant

**For concurrent agent execution (3 agents in parallel):**
- Use `tokio::spawn` per agent task, collect with `futures::future::join_all`
- Each agent task gets its own tracing span: `tracing::info_span!("agent", name = "claude")`
- Use `tokio::sync::mpsc` channels for agent-to-conductor status updates (actor pattern)
- Rate limiting: `tokio::sync::Semaphore` to cap concurrent API calls if needed

**For streaming AI responses:**
- `genai` handles SSE streaming natively
- Stream token output to terminal via `indicatif` progress bar messages as tokens arrive
- Collect full response for cross-review before advancing phase

**For phase-gated execution (blocking on review gate):**
- Use `tokio::sync::watch` or `tokio::sync::Notify` to signal phase completion to waiting tasks
- Gate advancement with a review result enum: `ReviewResult::Pass | ReviewResult::Fail(issues)`
- Loop on `Fail` with retry budget (e.g., max 3 review cycles per phase)

**For git operations (one commit per phase):**
- Run git2 operations synchronously within a `tokio::task::spawn_blocking` block
- Never call blocking git2 code directly in an async context — it blocks the executor thread
- Stage all agent-written files, commit with structured message: `"[Athena] Phase N: <description> (agents: claude, gemini)"`

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| genai 0.5 | reqwest 0.13, tokio 1.x | genai ships reqwest 0.13 internally; if you add reqwest directly, pin to 0.13 to avoid duplicate versions |
| tracing 0.1 | tokio 1.x, tracing-subscriber 0.3 | tracing-subscriber 0.3 is required for the env-filter feature; 0.2 is EOL |
| clap 4.5 | Rust 1.74+ | clap 4 requires MSRV 1.74; verify CI uses stable Rust not older |
| git2 0.20 | libgit2 1.9.0 (bundled) | No system libgit2 required — bundled via libgit2-sys. Adds ~2MB to compile time but zero runtime dep. |
| serde 1.x | serde_json 1.x, chrono 0.4 | Keep serde features consistent: enable `derive` at the workspace level if using a workspace |

---

## Sources

- genai GitHub (jeremychone/rust-genai) — v0.5.3 confirmed, provider support, reqwest 0.13 upgrade verified
- docs.rs/genai — Adapter pattern, ClientBuilder API, provider routing
- tokio.rs — Version 1.50.0 current (March 2026), task spawning, channels, spawn_blocking docs
- crates.io — clap 4.5.60 (Feb 2026), git2 0.20.2, serde_json 1.0.149, dotenvy 0.15.7
- seanmonstar.com/blog/reqwest-v013-rustls-default — reqwest 0.13 TLS change details (MEDIUM confidence — single official source)
- corrode.dev/blog/async — Tokio vs alternatives comparison (MEDIUM confidence — community analysis)
- ryhl.io/blog/actors-with-tokio — Actor pattern with tokio::sync::mpsc (MEDIUM confidence — widely cited community resource)
- RUSTSEC-2021-0141 — dotenv security advisory justifying dotenvy (HIGH confidence — official advisory)

---

*Stack research for: Athena — Multi-agent AI orchestration CLI in Rust*
*Researched: 2026-03-12*
