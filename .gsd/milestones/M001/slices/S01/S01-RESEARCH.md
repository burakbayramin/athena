# Phase 1: Foundation - Research

**Researched:** 2026-03-12
**Domain:** Rust workspace scaffolding, type system design, config loading, error hierarchy
**Confidence:** HIGH

## Summary

Phase 1 establishes the foundational Cargo workspace with fine-grained `ath-*` crates, a shared type system for inter-agent communication, config loading from TOML files and environment variables, and a unified error hierarchy using thiserror (domain errors) + anyhow (binary boundary). This is a greenfield Rust CLI project with no existing code.

The Rust ecosystem has mature, well-documented solutions for every component in this phase. The core libraries (serde, thiserror, anyhow, clap, toml, dotenvy) are stable, widely adopted, and well-tested. The primary risk is over-engineering the type system upfront -- the CONTEXT.md explicitly calls for full contracts defined in Phase 1, so schema design must be thorough but not speculative about Phase 2+ internals.

**Primary recommendation:** Use workspace-level dependency management with exact library versions. Keep ath-types and ath-config fully synchronous (no tokio dependency). Define all inter-agent schemas with `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]` and validate() methods returning typed errors.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Fine-grained multi-crate Cargo workspace with ath-* prefix
- Minimum 6 crates: ath-types, ath-config, ath-agents, ath-git, ath-planner, ath-orchestrator, ath-cli
- ath-types includes validation logic (impl blocks with validate() methods), not pure data
- No ath-common/prelude crate -- each crate imports directly from specific crates
- Workspace-level dependency management in root Cargo.toml [workspace.dependencies]
- Tokio only in crates that need async (ath-agents, ath-engine, ath-cli) -- ath-types and ath-config are sync
- Tests: inline #[cfg(test)] mod tests + tests/ directory for integration tests per crate
- Binary name: `ath` (not `athena`)
- TOML format for config files
- Dual config locations: global at ~/.config/ath/ + optional project-local override (.ath.toml in project root)
- Precedence: CLI flags > env vars > project-local config > global config
- Standard provider env var names: ANTHROPIC_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY
- Missing API keys: warn and skip that provider (graceful degradation)
- `ath init` creates config interactively with guided setup
- Full contracts defined upfront in Phase 1 -- no extensible/HashMap-based fields
- ProjectSpec: project name, description, goals list, constraints list, target language/framework, expected file manifest, per-goal skill requirement tags
- AgentRequest/AgentResponse: typed messages between orchestrator and agents
- ReviewVerdict: pass/fail boolean + text reason, severity levels (critical/warning/info -- only critical blocks), code suggestion capability
- Agent identification: Enum with model variant -- e.g., Agent::Claude("opus-4")
- PhaseRecord: full audit trail -- start/end timestamps, token counts per agent, estimated cost, files produced per agent, all review attempts with each verdict
- JSON serialization for all inter-agent communication
- Error style: red "Error:" prefix + message + fix suggestion on next line
- Every error includes a fix hint (actionable next step)
- --verbose adds: component chain context + raw API response
- Color output: auto-detect TTY, override with --no-color flag or NO_COLOR env var

### Claude's Discretion
- Cargo feature flag strategy for optional provider compilation
- Exact crate boundary for planner vs orchestrator vs engine
- Internal error type granularity (how many error variants per crate)
- Specific serde attributes and derive patterns

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INPT-04 | User can configure API keys via environment variables or config file | ConfigStore design with dotenvy for .env, toml for config files, clap env integration for CLI, dirs for ~/.config/ath/ path resolution |
| PLAN-04 | Athena defines typed JSON schemas for inter-agent communication at every boundary | serde + serde_json for JSON round-trip, thiserror for validation errors, chrono for timestamps, uuid for unique identifiers |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 | Serialization framework | Universal Rust serialization; derive macros for Serialize/Deserialize |
| serde_json | 1.0 | JSON serialization | Standard JSON codec for serde; needed for inter-agent JSON communication |
| thiserror | 2.0 | Domain error derive macros | Generates Display + Error impls; standard for library-style error enums |
| anyhow | 1.0 | Application error type | Opaque error + context chaining at binary boundary; pairs with thiserror |
| clap | 4.5 | CLI argument parsing | Derive-based parser with env var support; industry standard |
| toml | 0.8 | TOML config parsing | Official TOML serde implementation for Rust |
| dotenvy | 0.15 | .env file loading | Well-maintained dotenv fork; loads env vars from .env files |
| chrono | 0.4 | Date/time types | Timestamps in PhaseRecord; serde integration via feature flag |
| uuid | 1.8 | Unique identifiers | V4 UUIDs for request/response correlation; serde integration |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs | 6.0 | Platform config dirs | Resolving ~/.config/ath/ cross-platform (XDG on Linux, Known Folders on Windows) |
| colored | 3.0 | Terminal coloring | Error output formatting; respects NO_COLOR env var and TTY detection |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| chrono | time 0.3 | time is lighter but chrono has better serde integration and wider ecosystem adoption |
| colored | termcolor | termcolor is more low-level; colored is simpler for the "red Error: prefix" pattern |
| dotenvy | manual env::var | dotenvy handles .env file loading and CI compatibility automatically |

**Installation (workspace-level Cargo.toml):**
```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
anyhow = "1.0"
clap = { version = "4.5", features = ["derive", "env"] }
toml = "0.8"
dotenvy = "0.15"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.8", features = ["v4", "serde"] }
dirs = "6.0"
colored = "3.0"
```

## Architecture Patterns

### Recommended Project Structure
```
athena/
├── Cargo.toml              # [workspace] definition + [workspace.dependencies]
├── Cargo.lock
├── crates/
│   ├── ath-types/
│   │   ├── Cargo.toml      # depends on: serde, serde_json, chrono, uuid, thiserror
│   │   ├── src/
│   │   │   ├── lib.rs       # re-exports all public types
│   │   │   ├── agent.rs     # AgentKind enum, AgentRequest, AgentResponse
│   │   │   ├── project.rs   # ProjectSpec, SkillTag
│   │   │   ├── review.rs    # ReviewVerdict, Severity
│   │   │   ├── phase.rs     # PhaseRecord, ReviewAttempt
│   │   │   └── error.rs     # Shared validation error types
│   │   └── tests/           # Integration tests
│   ├── ath-config/
│   │   ├── Cargo.toml      # depends on: ath-types, serde, toml, dotenvy, dirs, thiserror
│   │   ├── src/
│   │   │   ├── lib.rs       # ConfigStore public API
│   │   │   ├── store.rs     # ConfigStore impl: load, merge, validate
│   │   │   ├── file.rs      # TOML file reading + parsing
│   │   │   ├── env.rs       # Environment variable loading
│   │   │   └── error.rs     # Config-specific error types
│   │   └── tests/
│   ├── ath-cli/
│   │   ├── Cargo.toml      # depends on: ath-config, ath-types, clap, anyhow, colored
│   │   └── src/
│   │       └── main.rs      # Binary entry point: clap parse -> config load -> dispatch
│   ├── ath-agents/          # Stub Cargo.toml only in Phase 1
│   ├── ath-git/             # Stub Cargo.toml only in Phase 1
│   ├── ath-planner/         # Stub Cargo.toml only in Phase 1
│   └── ath-orchestrator/    # Stub Cargo.toml only in Phase 1
```

### Pattern 1: Workspace Dependency Inheritance
**What:** Declare all shared dependencies once in root Cargo.toml under `[workspace.dependencies]`, then reference them with `dep.workspace = true` in each crate.
**When to use:** Always -- this is the standard workspace pattern.
**Example:**
```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"

# crates/ath-types/Cargo.toml
[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

### Pattern 2: thiserror for Domain Errors, anyhow at Binary Boundary
**What:** Each library crate (ath-types, ath-config) defines its own error enum with `#[derive(thiserror::Error)]`. The binary crate (ath-cli) uses `anyhow::Result` as return type for main() and converts domain errors with `.context()`.
**When to use:** Standard pattern for multi-crate Rust projects. Library crates expose typed errors so callers can match on variants. The binary crate aggregates them into anyhow for display.
**Example:**
```rust
// ath-config/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config file not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid TOML in config file: {source}")]
    InvalidToml {
        #[source]
        source: toml::de::Error,
    },

    #[error("Missing API key for provider {provider}")]
    MissingApiKey { provider: String },
}

// ath-cli/src/main.rs
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = ath_config::ConfigStore::load()
        .context("Failed to load configuration")?;
    Ok(())
}
```

### Pattern 3: Validate Methods on Types
**What:** Each schema type has an `impl` block with a `pub fn validate(&self) -> Result<(), ValidationError>` method that checks business rules beyond what serde can enforce.
**When to use:** For all inter-agent types where structural validity (serde) is not sufficient -- e.g., ProjectSpec must have at least one goal, ReviewVerdict with severity=Critical must have a non-empty reason.
**Example:**
```rust
// ath-types/src/project.rs
use serde::{Deserialize, Serialize};
use crate::error::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSpec {
    pub name: String,
    pub description: String,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub target_language: Option<String>,
    pub target_framework: Option<String>,
    pub expected_files: Vec<String>,
    pub skill_tags: Vec<SkillTag>,
}

impl ProjectSpec {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.is_empty() {
            return Err(ValidationError::empty_field("name"));
        }
        if self.goals.is_empty() {
            return Err(ValidationError::empty_field("goals"));
        }
        Ok(())
    }
}
```

### Pattern 4: Config Layered Loading
**What:** ConfigStore loads config from multiple sources in precedence order and merges them. Lower-priority values fill in gaps; higher-priority values override.
**When to use:** For the dual config location pattern (global + project-local + env vars).
**Example:**
```rust
// ath-config/src/store.rs
use std::path::PathBuf;

pub struct ConfigStore {
    pub anthropic_api_key: Option<String>,
    pub google_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    // ... other config fields
}

impl ConfigStore {
    pub fn load() -> Result<Self, ConfigError> {
        // 1. Load global config: ~/.config/ath/config.toml
        let global = Self::load_file(Self::global_config_path()?)?;
        // 2. Overlay project-local config: ./.ath.toml
        let local = Self::load_file(PathBuf::from(".ath.toml")).ok();
        // 3. Overlay env vars (ANTHROPIC_API_KEY, etc.)
        let env = Self::load_env();
        // 4. Merge: env > local > global
        Ok(Self::merge(global, local, env))
    }

    fn global_config_path() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|p| p.join("ath").join("config.toml"))
            .ok_or(ConfigError::NoConfigDir)
    }
}
```

### Pattern 5: Agent Enum with Model Variant
**What:** Type-safe agent identification using an enum that carries the model string.
**When to use:** For all inter-agent communication types.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentKind {
    Claude(String),   // e.g., "opus-4"
    Gemini(String),   // e.g., "2.5-pro"
    Codex(String),    // e.g., "o3"
}

impl AgentKind {
    pub fn provider_name(&self) -> &str {
        match self {
            AgentKind::Claude(_) => "Anthropic",
            AgentKind::Gemini(_) => "Google",
            AgentKind::Codex(_) => "OpenAI",
        }
    }
}
```

### Anti-Patterns to Avoid
- **HashMap-based extensible fields:** CONTEXT.md explicitly forbids this. All fields must be typed. No `extra: HashMap<String, Value>`.
- **Shared prelude/common crate:** No ath-common. Each crate imports from the specific crate it needs.
- **Tokio in ath-types or ath-config:** These crates must be fully synchronous. Only ath-cli, ath-agents, and ath-engine use async.
- **Duplicated type definitions:** All shared types live in ath-types. Other crates re-export or import from there. Never copy a struct definition.
- **Panicking on missing config:** Missing API keys produce a warning and graceful degradation, not panic.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing | Custom parser | `toml` crate with serde | Edge cases in TOML spec (multiline strings, inline tables, datetime) |
| Error display formatting | Manual Display impls | `thiserror` derive | Boilerplate-prone, easy to forget #[source] chaining |
| XDG config paths | Hardcoded ~/.config | `dirs` crate | Cross-platform (Linux XDG, macOS ~/Library, Windows AppData) |
| .env file loading | Manual file parser | `dotenvy` | Handles comments, quotes, multiline values, missing file gracefully |
| CLI arg parsing | Manual arg parsing | `clap` derive | Validation, help generation, env var fallback, completions |
| UUID generation | Custom ID scheme | `uuid` v4 | Cryptographic randomness, standard format, serde integration |

**Key insight:** Every "simple" config/parsing problem has edge cases that will bite during cross-platform testing. Use the ecosystem.

## Common Pitfalls

### Pitfall 1: Circular Dependencies Between Crates
**What goes wrong:** ath-types depends on ath-config for loading, ath-config depends on ath-types for config struct. Cargo refuses to compile.
**Why it happens:** Natural temptation to have types reference the config system or vice versa.
**How to avoid:** ath-types has ZERO dependencies on other ath-* crates. ath-config depends on ath-types (one-way). ath-cli depends on both. Strict unidirectional dependency graph.
**Warning signs:** `cargo check` fails with "cyclic package dependency" error.

### Pitfall 2: serde Default vs Option for Config Fields
**What goes wrong:** Using `#[serde(default)]` on required fields silently fills in empty strings or zeros, hiding misconfiguration.
**Why it happens:** Developer wants to avoid Option wrapping overhead but loses the ability to distinguish "not set" from "set to default."
**How to avoid:** Use `Option<T>` for truly optional config. Use required fields (no default) for must-have config like project name. Use `#[serde(default = "default_fn")]` only when a sensible default exists.
**Warning signs:** Tests pass with empty config that should have been rejected.

### Pitfall 3: Windows Path Handling in Config
**What goes wrong:** Hardcoding `/` separators or `~` expansion breaks on Windows.
**Why it happens:** Developer tests only on Unix.
**How to avoid:** Use `dirs::config_dir()` which returns platform-correct paths. Use `PathBuf::join()` not string concatenation.
**Warning signs:** Tests fail on Windows CI or user reports config not found.

### Pitfall 4: thiserror 2.0 Breaking Changes
**What goes wrong:** Using thiserror 1.x syntax patterns that changed in 2.0.
**Why it happens:** Most tutorials still reference thiserror 1.x.
**How to avoid:** thiserror 2.0 changed some attribute syntax. Key change: `#[error(transparent)]` still works. The `#[from]` attribute is still supported. Check docs.rs/thiserror/2.0 for current syntax.
**Warning signs:** Compile errors on derive macros.

### Pitfall 5: Config Precedence Bugs
**What goes wrong:** Environment variables don't actually override config file values, or project-local config doesn't override global.
**Why it happens:** Merge logic processes sources in wrong order or uses wrong merge strategy (first-wins vs last-wins).
**How to avoid:** Build and test the merge function with explicit test cases for each precedence level. Load in order: global (lowest) -> project-local -> env vars (highest). Each layer overwrites `Some` values from the previous layer.
**Warning signs:** Setting an env var has no effect when config file also has the value.

### Pitfall 6: Timestamp Serialization Mismatch
**What goes wrong:** chrono DateTime serializes to RFC 3339 by default, but some consumers expect Unix timestamps.
**Why it happens:** Different serde modules produce different formats.
**How to avoid:** Standardize on ISO 8601 / RFC 3339 strings for all timestamps in JSON. Use chrono's default serde implementation which produces "2026-03-12T10:30:00Z" format. Document this in the type itself with a comment.
**Warning signs:** Deserialization fails when reading timestamps written by a different component.

## Code Examples

### Complete ProjectSpec with Validation
```rust
// ath-types/src/project.rs
use serde::{Deserialize, Serialize};
use crate::error::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTag(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSpec {
    pub name: String,
    pub description: String,
    pub goals: Vec<GoalSpec>,
    pub constraints: Vec<String>,
    pub target_language: Option<String>,
    pub target_framework: Option<String>,
    pub expected_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalSpec {
    pub description: String,
    pub skill_tags: Vec<SkillTag>,
}

impl ProjectSpec {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyField {
                field: "name".into(),
                hint: "Provide a project name".into(),
            });
        }
        if self.goals.is_empty() {
            return Err(ValidationError::EmptyField {
                field: "goals".into(),
                hint: "Provide at least one project goal".into(),
            });
        }
        Ok(())
    }
}
```

### ReviewVerdict with Severity
```rust
// ath-types/src/review.rs
use serde::{Deserialize, Serialize};
use crate::agent::AgentKind;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeSuggestion {
    pub file: String,
    pub line: Option<u32>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewVerdict {
    pub passed: bool,
    pub reviewer: AgentKind,
    pub severity: Severity,
    pub reason: String,
    pub suggestions: Vec<CodeSuggestion>,
}

impl ReviewVerdict {
    pub fn blocks_progress(&self) -> bool {
        !self.passed && self.severity == Severity::Critical
    }
}
```

### PhaseRecord Audit Trail
```rust
// ath-types/src/phase.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::agent::AgentKind;
use crate::review::ReviewVerdict;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentContribution {
    pub agent: AgentKind,
    pub tokens: TokenUsage,
    pub files_produced: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewAttempt {
    pub attempt_number: u32,
    pub verdict: ReviewVerdict,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseRecord {
    pub id: Uuid,
    pub phase_name: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub contributions: Vec<AgentContribution>,
    pub review_attempts: Vec<ReviewAttempt>,
}
```

### Error with Fix Hint
```rust
// ath-types/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Required field '{field}' is empty")]
    EmptyField { field: String, hint: String },

    #[error("Invalid value for '{field}': {reason}")]
    InvalidValue { field: String, reason: String, hint: String },
}

impl ValidationError {
    pub fn hint(&self) -> &str {
        match self {
            ValidationError::EmptyField { hint, .. } => hint,
            ValidationError::InvalidValue { hint, .. } => hint,
        }
    }
}
```

### Config TOML File Format
```toml
# ~/.config/ath/config.toml
[providers]
anthropic_api_key = "sk-ant-..."
google_api_key = "AIza..."
openai_api_key = "sk-..."

[defaults]
# Default model selections (optional)
claude_model = "opus-4"
gemini_model = "2.5-pro"
codex_model = "o3"
```

### Round-Trip Serialization Test
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn project_spec_round_trip() {
        let spec = ProjectSpec {
            name: "todo-app".into(),
            description: "A REST API todo application".into(),
            goals: vec![GoalSpec {
                description: "CRUD endpoints".into(),
                skill_tags: vec![SkillTag("rust".into()), SkillTag("api".into())],
            }],
            constraints: vec!["Must use PostgreSQL".into()],
            target_language: Some("Rust".into()),
            target_framework: Some("Axum".into()),
            expected_files: vec!["src/main.rs".into()],
        };

        let json = serde_json::to_string(&spec).expect("serialize");
        let deserialized: ProjectSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(spec, deserialized);
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| dotenv crate | dotenvy | 2021 (RUSTSEC-2021-0141) | dotenv unmaintained; dotenvy is the security-advisory replacement |
| thiserror 1.x | thiserror 2.0 | 2024 | Minor attribute syntax changes; same core pattern |
| Manual CLI parsing | clap 4.x derive | 2022 | Derive macros eliminate boilerplate; env feature handles env vars |
| config crate | Direct toml + dotenvy | Ongoing | config crate adds complexity; direct toml parsing is simpler for known-format configs |

**Deprecated/outdated:**
- `dotenv`: Unmaintained, security advisory -- use dotenvy
- `structopt`: Merged into clap 4 derive -- use clap directly

## Open Questions

1. **Exact dirs crate version**
   - What we know: dirs 6.x is current, provides config_dir() cross-platform
   - What's unclear: Whether 6.0 is the exact latest or a newer patch exists
   - Recommendation: Use `dirs = "6"` (semver-compatible range) and let Cargo.lock pin the exact version

2. **Colored crate exact version**
   - What we know: colored 3.x is current, supports NO_COLOR
   - What's unclear: Exact latest patch version
   - Recommendation: Use `colored = "3"` (semver range) -- the API is stable

3. **Feature flag strategy for providers**
   - What we know: User left this to Claude's discretion
   - Recommendation: Skip feature flags in Phase 1. All provider config fields are `Option<String>`. Feature-gated compilation can be added in Phase 2 when actual provider crates are introduced. Premature feature flags add complexity without benefit when there's no optional compilation target yet.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) |
| Config file | None needed -- Cargo's default test runner |
| Quick run command | `cargo test -p ath-types && cargo test -p ath-config` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INPT-04 | Config loads from env vars | unit | `cargo test -p ath-config -- env` | Wave 0 |
| INPT-04 | Config loads from TOML file | unit | `cargo test -p ath-config -- file` | Wave 0 |
| INPT-04 | Config precedence: env > local > global | integration | `cargo test -p ath-config --test precedence` | Wave 0 |
| INPT-04 | Missing API key warns, doesn't panic | unit | `cargo test -p ath-config -- missing_key` | Wave 0 |
| PLAN-04 | ProjectSpec JSON round-trip | unit | `cargo test -p ath-types -- project_spec_round_trip` | Wave 0 |
| PLAN-04 | AgentRequest/AgentResponse round-trip | unit | `cargo test -p ath-types -- agent_round_trip` | Wave 0 |
| PLAN-04 | ReviewVerdict round-trip | unit | `cargo test -p ath-types -- review_round_trip` | Wave 0 |
| PLAN-04 | PhaseRecord round-trip | unit | `cargo test -p ath-types -- phase_record_round_trip` | Wave 0 |
| PLAN-04 | Validation rejects invalid ProjectSpec | unit | `cargo test -p ath-types -- validate` | Wave 0 |
| SC-1 | `ath --version` succeeds with config | integration | `cargo test -p ath-cli --test version_check` | Wave 0 |
| SC-2 | Invalid API key produces typed error | integration | `cargo test -p ath-config --test error_display` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p <affected-crate>`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before verify-work

### Wave 0 Gaps
- All test files -- this is a greenfield project, no test infrastructure exists yet
- Tests will be created alongside implementation in each plan
- No external test framework install needed -- cargo test is built-in

## Sources

### Primary (HIGH confidence)
- [thiserror crates.io](https://crates.io/crates/thiserror) - version 2.0.18, derive patterns
- [anyhow crates.io](https://crates.io/crates/anyhow) - version 1.0.102, context chaining
- [clap crates.io](https://crates.io/crates/clap) - version 4.5.60, derive + env features
- [dotenvy crates.io](https://crates.io/crates/dotenvy) - version 0.15.7, .env loading
- [serde.rs](https://serde.rs/) - serde 1.0.228, derive usage
- [Cargo Workspaces Book](https://doc.rust-lang.org/cargo/reference/workspaces.html) - workspace.dependencies pattern

### Secondary (MEDIUM confidence)
- [thiserror guide](https://generalistprogrammer.com/tutorials/thiserror-rust-crate-guide) - thiserror 2.0 patterns verified against docs.rs
- [Clap env vars](https://rust.code-maven.com/clap/clap-and-environment-variables.html) - env attribute usage
- [dirs GitHub](https://github.com/xdg-rs/dirs) - cross-platform config directory resolution

### Tertiary (LOW confidence)
- Exact latest patch versions for dirs and colored -- semver ranges used as mitigation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries are widely adopted, stable, well-documented
- Architecture: HIGH - workspace pattern and thiserror/anyhow split are established Rust best practices
- Pitfalls: HIGH - common issues are well-documented in Rust community
- Schema design: MEDIUM - specific field choices (GoalSpec structure, SkillTag type) are based on CONTEXT.md requirements but will be validated through round-trip tests

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable ecosystem, 30-day validity)