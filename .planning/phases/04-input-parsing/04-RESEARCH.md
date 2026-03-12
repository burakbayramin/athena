# Phase 4: Input Parsing - Research

**Researched:** 2026-03-12
**Domain:** CLI input modes, LLM-based structured extraction, codebase scanning
**Confidence:** HIGH

## Summary

Phase 4 adds three input modes to the `ath run` command: natural language description (positional arg), spec file (`--spec ./file.md`), and codebase analysis (`--codebase ./path`). All three produce a normalized `ProjectSpec` via LLM parsing with retry-on-validation-failure. The codebase is well-prepared: `ProjectSpec` with `validate()` exists in ath-types, `ClaudeHandle` with retry/circuit-breaker exists in ath-agents, and the CLI already has a `Run` subcommand with `description: Option<String>`.

The primary technical challenges are: (1) designing the LLM prompt that reliably produces valid JSON matching `ProjectSpec`'s serde schema, (2) using genai's `ChatResponseFormat::JsonSpec` for structured output enforcement, and (3) implementing gitignore-respecting directory traversal for codebase scanning. All three are well-supported by existing ecosystem libraries.

**Primary recommendation:** Build an `InputParser` abstraction in a new module (either in ath-planner or a new ath-input crate) that takes raw input (string, file path, or directory path) and returns `Result<ProjectSpec, InputError>`. Use genai's `JsonSpec` response format with the ProjectSpec JSON schema. Use the `ignore` crate for gitignore-aware codebase traversal.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Flags on `ath run`: positional arg for natural language, `--spec ./file.md` for spec files, `--codebase ./path` for existing projects
- Input is required -- error with clear message if no description, --spec, or --codebase provided
- Combining allowed: `--codebase ./project "add auth"` gives codebase context + user intent. `--spec` is exclusive (already a full definition)
- After parsing, show ProjectSpec summary for user confirmation before proceeding. Phase 8 will add `--yes` flag for CI/scripting
- No interactive prompt mode -- always require explicit input
- Claude is the default parsing agent (best at structured reasoning and JSON schema adherence)
- On malformed JSON or failed ProjectSpec validation: retry with the validation error appended to prompt, max 3 retries (matches Phase 2 retry-with-context pattern)
- Infer target_language and target_framework when obvious from description ("React dashboard" -> TypeScript/React). Leave as None when ambiguous
- Expand short descriptions into 3-8 concrete goals with skill tags -- give the phase decomposer (Phase 5) real structure to work with
- Use JSON mode (Claude tool_use) for guaranteed structure
- Markdown only for spec files -- no YAML, JSON, or TOML spec files
- LLM-parsed: any freeform markdown works, the LLM reads and extracts ProjectSpec (same pipeline as natural language, but from file content)
- File size cap at ~50KB (~12K tokens). Warn and refuse if larger -- prevents accidental cost explosions
- `ath init` generates an optional example spec.md template alongside config for discoverability
- Tree + key files strategy: read directory structure, then read key files (package.json/Cargo.toml, README, main entry points, config files)
- Send tree + key file contents to LLM for ProjectSpec extraction
- If --codebase provided without a description: prompt user "What do you want to build next?" after showing scan results
- If --codebase provided with a description: use description as intent, codebase as context
- Respect .gitignore patterns + skip known directories (node_modules, target/, .git/, dist/, build/, vendor/)

### Claude's Discretion
- Exact LLM prompt design for ProjectSpec extraction
- Which files count as "key files" in codebase scan (heuristics for entry points)
- ProjectSpec summary display format for confirmation step
- Internal module structure within ath-planner or new ath-input crate

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INPT-01 | User can describe a project in natural language and Athena parses it into structured intent | Natural language string sent to Claude via AgentBackend with JsonSpec response format; LLM extracts ProjectSpec with goals, constraints, language/framework inference |
| INPT-02 | User can provide a spec file (markdown/structured document) as project input | Read markdown file (with 50KB size cap), send contents through same LLM parsing pipeline as INPT-01; file content replaces user description in prompt |
| INPT-03 | User can point Athena at an existing codebase to analyze and determine next steps | Use `ignore` crate for gitignore-respecting tree traversal, identify key files by name heuristics, send tree + file contents to LLM for ProjectSpec extraction |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| genai | 0.5 | LLM API calls with JSON mode | Already in workspace; provides ChatResponseFormat::JsonSpec for structured output |
| ignore | 0.4 | Gitignore-respecting directory traversal | BurntSushi's crate (ripgrep's engine); respects .gitignore, .git/info/exclude, global gitignore |
| serde_json | 1.0 | JSON schema generation and parsing | Already in workspace; needed for JsonSpec schema Value |
| clap | 4.5 | CLI arg parsing with --spec and --codebase flags | Already in workspace; supports PathBuf args |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio (fs) | 1 | Async file reading for spec files | Already in workspace; use tokio::fs::read_to_string for non-blocking file reads |
| colored | 3 | ProjectSpec summary display formatting | Already in workspace; for confirmation output |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| ignore | walkdir + manual .gitignore parsing | walkdir already transitive dep but lacks gitignore support; ignore wraps it with full gitignore semantics |
| genai JsonSpec | Prompt-only JSON extraction + serde parse | JsonSpec leverages provider-native JSON mode; prompt-only is fragile across providers |

**Installation:**
```bash
# Add to workspace Cargo.toml
ignore = "0.4"

# Add to relevant crate Cargo.toml
ignore.workspace = true
```

## Architecture Patterns

### Recommended Module Structure
```
crates/ath-planner/src/
  lib.rs                  # Re-exports
  input/
    mod.rs                # InputParser trait/struct, InputMode enum
    prompt.rs             # LLM prompt templates for ProjectSpec extraction
    spec_file.rs          # Markdown spec file reader (read + size check)
    codebase.rs           # Codebase scanner (tree + key files)
    error.rs              # InputError enum with fix hints
```

**Rationale for ath-planner over new crate:** Input parsing feeds directly into phase planning (Phase 5). A separate ath-input crate adds a dependency hop with no reuse benefit. Keep it as an `input` submodule of ath-planner.

### Pattern 1: InputMode Enum
**What:** Discriminated union of the three input modes
**When to use:** CLI dispatches to the correct parsing path
**Example:**
```rust
pub enum InputMode {
    /// Natural language description from positional arg
    NaturalLanguage(String),
    /// Markdown spec file path
    SpecFile(PathBuf),
    /// Codebase directory + optional intent description
    Codebase {
        path: PathBuf,
        intent: Option<String>,
    },
}
```

### Pattern 2: LLM Parsing with Retry-on-Validation
**What:** Send prompt to Claude, parse response as ProjectSpec, retry with error feedback on failure
**When to use:** All three input modes converge on this pattern after assembling the prompt
**Example:**
```rust
async fn parse_to_project_spec(
    backend: &dyn AgentBackend,
    prompt: &str,
    system: &str,
) -> Result<ProjectSpec, InputError> {
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 0..max_retries {
        let request = build_request(prompt, system, last_error.as_deref());
        let response = backend.send(request).await
            .map_err(InputError::Agent)?;

        match serde_json::from_str::<ProjectSpec>(&response.content) {
            Ok(spec) => match spec.validate() {
                Ok(()) => return Ok(spec),
                Err(e) => {
                    last_error = Some(format!(
                        "Validation failed: {}. Hint: {}", e, e.hint()
                    ));
                }
            },
            Err(e) => {
                last_error = Some(format!("JSON parse error: {e}"));
            }
        }
    }

    Err(InputError::ParseFailed { attempts: max_retries, last_error })
}
```

### Pattern 3: genai JsonSpec for Structured Output
**What:** Use genai's ChatResponseFormat::JsonSpec to enforce JSON schema at the provider level
**When to use:** Every LLM call for ProjectSpec extraction
**Example:**
```rust
use genai::chat::{ChatOptions, ChatResponseFormat, JsonSpec};

fn build_chat_options() -> ChatOptions {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "description": { "type": "string" },
            "goals": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string" },
                        "skill_tags": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["description", "skill_tags"]
                }
            },
            "constraints": { "type": "array", "items": { "type": "string" } },
            "target_language": { "type": ["string", "null"] },
            "target_framework": { "type": ["string", "null"] },
            "expected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name", "description", "goals", "constraints", "expected_files"]
    });

    let json_spec = JsonSpec::new("project_spec", schema);
    ChatOptions::default()
        .with_response_format(ChatResponseFormat::JsonSpec(json_spec))
}
```

**Note on SkillTag:** ProjectSpec uses `SkillTag(String)` newtype. The JSON schema should use `"type": "string"` for skill_tags items. Serde serializes `SkillTag("rust")` as just `"rust"` since it's a tuple struct, so the JSON schema naturally matches.

### Pattern 4: Codebase Tree + Key Files
**What:** Walk directory respecting gitignore, build tree string, identify and read key files
**When to use:** `--codebase` mode
**Example:**
```rust
use ignore::WalkBuilder;

fn scan_codebase(path: &Path) -> Result<(String, Vec<(String, String)>), InputError> {
    let mut tree_lines = Vec::new();
    let mut key_files: Vec<(String, String)> = Vec::new();

    let walker = WalkBuilder::new(path)
        .hidden(true)       // skip hidden files by default
        .git_ignore(true)   // respect .gitignore
        .git_global(true)   // respect global gitignore
        .build();

    for entry in walker.flatten() {
        let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
        tree_lines.push(format_tree_line(rel, entry.depth()));

        if is_key_file(rel) {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                key_files.push((rel.display().to_string(), content));
            }
        }
    }

    Ok((tree_lines.join("\n"), key_files))
}
```

### Pattern 5: CLI Flag Extension
**What:** Add --spec and --codebase to the existing Run subcommand
**When to use:** CLI layer only
**Example:**
```rust
#[derive(Subcommand)]
enum Commands {
    Run {
        /// Project description in natural language
        description: Option<String>,
        /// Path to a markdown spec file
        #[arg(long, value_name = "FILE")]
        spec: Option<PathBuf>,
        /// Path to an existing codebase to analyze
        #[arg(long, value_name = "DIR")]
        codebase: Option<PathBuf>,
    },
    // ...
}
```

### Anti-Patterns to Avoid
- **Sending entire codebase to LLM:** Token cost explosion. Always tree + key files only.
- **Parsing LLM output without retry:** LLMs occasionally produce malformed JSON even with JsonSpec. Always validate + retry with error feedback.
- **Blocking file I/O in async context:** Use tokio::fs or spawn_blocking for file reads, especially codebase scanning.
- **Hardcoded prompt strings:** Keep prompts in dedicated constants or a prompt module for iteration and testing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Gitignore parsing | Custom .gitignore parser | `ignore` crate WalkBuilder | Gitignore spec has 10+ edge cases (negation, re-inclusion, nested ignores, global patterns) |
| JSON schema enforcement | Prompt-only "return JSON" | genai `ChatResponseFormat::JsonSpec` | Provider-native JSON mode is significantly more reliable than prompt-only |
| Directory tree traversal | Manual fs::read_dir recursion | `ignore` crate Walk | Handles symlinks, permission errors, .gitignore cascading correctly |
| CLI argument validation | Manual if/else on Options | clap's `conflicts_with` / `requires` | clap generates correct error messages and help text automatically |

**Key insight:** The LLM parsing pipeline is the core value; filesystem and CLI concerns should use battle-tested libraries.

## Common Pitfalls

### Pitfall 1: SkillTag Serde Representation
**What goes wrong:** SkillTag is `SkillTag(pub String)` -- a newtype tuple struct. Need to confirm serde serializes/deserializes it as a bare string, not `{"0": "rust"}`.
**Why it happens:** Some serde configurations wrap newtypes in objects.
**How to avoid:** SkillTag already derives Serialize/Deserialize with default settings, which serializes tuple structs transparently. Verified in existing test `project_spec_round_trip`. The JSON schema uses `"type": "string"` for skill_tag items.
**Warning signs:** Deserialization errors on the skill_tags field specifically.

### Pitfall 2: genai ChatOptions Not Passed Through AgentBackend
**What goes wrong:** The current `AgentBackend::send()` takes `AgentRequest` which has `prompt` and `context` but no `ChatOptions`. The actor's `call_provider` builds `ChatRequest::from_user(prompt)` with no response_format.
**Why it happens:** Phase 2 built the agent layer for general-purpose text completion, not structured output.
**How to avoid:** Two options: (1) Add an optional `response_format` field to `AgentRequest` and thread it through `call_provider`, or (2) bypass the actor pattern for input parsing and call genai directly. Option 1 is cleaner -- extend AgentRequest with an optional serde_json::Value for json_schema, and have call_provider apply it as ChatOptions.
**Warning signs:** LLM returns prose instead of JSON despite JsonSpec being set.

### Pitfall 3: 50KB File Size Check Timing
**What goes wrong:** Reading an entire large file into memory before checking size.
**Why it happens:** Natural to `read_to_string` then check `.len()`.
**How to avoid:** Use `std::fs::metadata(path)?.len()` to check file size before reading. 50KB = 51200 bytes.
**Warning signs:** Memory spike on unexpectedly large spec files.

### Pitfall 4: Codebase Scan Token Budget
**What goes wrong:** Key files in a large project exceed LLM context window.
**Why it happens:** package.json can be huge, README can be long, many entry points.
**How to avoid:** Cap total key file content at ~30KB. Read files in priority order (manifest first, then README, then entry points). Truncate individual files at ~5KB with a "[truncated]" marker.
**Warning signs:** LLM errors about context length, or very high token costs on codebase mode.

### Pitfall 5: --codebase Without Description Needs User Input
**What goes wrong:** The decision says "prompt user 'What do you want to build next?'" but the project also says "No interactive prompt mode -- always require explicit input."
**Why it happens:** Tension between codebase-only mode and non-interactive design.
**How to avoid:** If `--codebase` is provided alone with no description, show the scan summary and error with a message like: "Codebase scanned. Provide a description of what to build next: `ath run --codebase ./path 'your description'`". Do NOT use stdin prompt -- keep it CLI-error-driven.
**Warning signs:** Hanging stdin read in CI/automation contexts.

### Pitfall 6: Confirmation Step Blocking Async Runtime
**What goes wrong:** Using `std::io::stdin().read_line()` in async context blocks the tokio runtime.
**Why it happens:** Confirmation step reads user input synchronously.
**How to avoid:** Use `tokio::io::BufReader` on `tokio::io::stdin()` or `spawn_blocking` for the confirmation read. Or simpler: since Phase 8 adds `--yes`, for now just display the summary and proceed (confirmation can be a simple "Press Enter to continue" with spawn_blocking).
**Warning signs:** Runtime stalls during confirmation on multi-threaded tokio.

## Code Examples

### Extending AgentRequest for Structured Output
```rust
// In ath-types/src/agent.rs -- add optional json_schema field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRequest {
    pub id: Uuid,
    pub agent: AgentKind,
    pub prompt: String,
    pub context: Option<String>,
    /// Optional JSON schema for structured output (provider-native JSON mode)
    pub json_schema: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
```

### Threading JsonSpec Through call_provider
```rust
// In ath-agents/src/actor/mod.rs -- extend call_provider
pub async fn call_provider(
    client: &genai::Client,
    model: &str,
    prompt: &str,
    context: Option<&str>,
    json_schema: Option<&serde_json::Value>,
    provider: &str,
) -> Result<(String, u64, u64), AgentError> {
    let mut chat_req = ChatRequest::from_user(prompt);
    if let Some(ctx) = context {
        chat_req = chat_req.with_system(ctx);
    }

    let options = json_schema.map(|schema| {
        let spec = JsonSpec::new("project_spec", schema.clone());
        ChatOptions::default()
            .with_response_format(ChatResponseFormat::JsonSpec(spec))
    });

    let response = client
        .exec_chat(model, chat_req, options.as_ref())
        .await
        .map_err(|e| classify_error(e, provider))?;

    let input_tokens = response.usage.prompt_tokens.unwrap_or(0) as u64;
    let output_tokens = response.usage.completion_tokens.unwrap_or(0) as u64;
    let content = response.into_first_text().unwrap_or_default();

    Ok((content, input_tokens, output_tokens))
}
```

### Key File Heuristics
```rust
/// Files considered "key" for codebase understanding (read in priority order)
const KEY_FILE_NAMES: &[&str] = &[
    // Package manifests
    "Cargo.toml", "package.json", "pyproject.toml", "go.mod",
    "pom.xml", "build.gradle", "Gemfile", "requirements.txt",
    // Documentation
    "README.md", "README", "README.rst",
    // Configuration
    ".env.example", "docker-compose.yml", "Dockerfile",
    "tsconfig.json", "rustfmt.toml",
    // Entry points (checked by pattern, not exact name)
];

const KEY_ENTRY_PATTERNS: &[&str] = &[
    "src/main.rs", "src/lib.rs", "src/index.ts", "src/index.js",
    "src/app.ts", "src/app.js", "main.go", "app.py", "manage.py",
];

fn is_key_file(rel_path: &Path) -> bool {
    let filename = rel_path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    let rel_str = rel_path.to_str().unwrap_or("");

    KEY_FILE_NAMES.contains(&filename)
        || KEY_ENTRY_PATTERNS.iter().any(|p| rel_str == *p)
}
```

### LLM Prompt Template (Natural Language Mode)
```rust
const SYSTEM_PROMPT: &str = r#"You are a project specification parser. Given a user's project description, extract a structured ProjectSpec.

Rules:
- name: Derive a short kebab-case project name from the description
- description: Expand the user's intent into a clear 1-2 sentence description
- goals: Break the project into 3-8 concrete goals, each with relevant skill_tags
- constraints: Extract any explicit constraints; if none stated, leave empty array
- target_language: Infer when obvious (e.g., "React app" -> "TypeScript"); null when ambiguous
- target_framework: Infer when obvious; null when ambiguous
- expected_files: List 3-10 key files the project would produce

Respond with valid JSON matching the ProjectSpec schema."#;
```

### InputError Type
```rust
#[derive(Debug, Error)]
pub enum InputError {
    #[error("No input provided")]
    NoInput {
        hint: String,
    },

    #[error("Spec file not found: {path}")]
    SpecFileNotFound {
        path: PathBuf,
        hint: String,
    },

    #[error("Spec file too large: {size} bytes (max {max} bytes)")]
    SpecFileTooLarge {
        size: u64,
        max: u64,
        hint: String,
    },

    #[error("Codebase path not found: {path}")]
    CodebaseNotFound {
        path: PathBuf,
        hint: String,
    },

    #[error("--spec cannot be combined with other input modes")]
    SpecExclusive {
        hint: String,
    },

    #[error("Failed to parse ProjectSpec after {attempts} attempts")]
    ParseFailed {
        attempts: usize,
        last_error: Option<String>,
    },

    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Prompt-only "return JSON" | Provider-native JSON mode (JsonSpec) | genai 0.5+ | Much more reliable structured output |
| Manual .gitignore parsing | `ignore` crate with WalkBuilder | Stable since 2020 | Correct gitignore semantics without bugs |
| Custom retry logic | retry-with-context-feedback pattern | Phase 2 decision | LLM almost always fixes on second try with error context |

## Open Questions

1. **exec_chat ChatOptions parameter threading**
   - What we know: `client.exec_chat(model, chat_req, None)` takes `Option<&ChatOptions>` as third param (already used in call_provider with None)
   - What's unclear: Whether adding json_schema to AgentRequest is the cleanest way vs. a separate method on the backend trait
   - Recommendation: Extend AgentRequest with optional json_schema field -- minimal API surface change, backward compatible (defaults to None)

2. **Confirmation step UX for v1**
   - What we know: Decision says show summary and confirm before proceeding. Phase 8 adds --yes flag.
   - What's unclear: Whether to use stdin confirmation now or just display + auto-proceed
   - Recommendation: Display summary, then proceed automatically with a note "Use --yes to skip this summary in CI". Avoids blocking stdin issues. Phase 8 makes it skippable.

3. **Codebase-only mode without description**
   - What we know: Decision says "prompt user" but also "no interactive prompt mode"
   - Recommendation: Error with actionable message rather than stdin prompt. Show scan summary in the error output.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Workspace Cargo.toml (resolver = "2") |
| Quick run command | `cargo test -p ath-planner --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INPT-01 | Natural language string parsed into ProjectSpec | unit | `cargo test -p ath-planner input::tests::natural_language -- --exact` | Wave 0 |
| INPT-01 | Retry on malformed LLM response | unit | `cargo test -p ath-planner input::tests::retry_on_malformed -- --exact` | Wave 0 |
| INPT-02 | Spec file read and parsed into ProjectSpec | unit | `cargo test -p ath-planner input::tests::spec_file_parse -- --exact` | Wave 0 |
| INPT-02 | Spec file size cap enforced | unit | `cargo test -p ath-planner input::tests::spec_file_too_large -- --exact` | Wave 0 |
| INPT-03 | Codebase scanned respecting gitignore | unit | `cargo test -p ath-planner input::tests::codebase_scan -- --exact` | Wave 0 |
| INPT-03 | Key files identified correctly | unit | `cargo test -p ath-planner input::tests::key_file_heuristics -- --exact` | Wave 0 |
| ALL | CLI flags --spec and --codebase parsed correctly | unit | `cargo test -p ath-cli -- --exact` | Wave 0 |
| ALL | InputMode validation (--spec exclusive, no input error) | unit | `cargo test -p ath-planner input::tests::input_validation -- --exact` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ath-planner --lib && cargo test -p ath-cli`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/ath-planner/src/input/mod.rs` -- InputParser, InputMode, tests
- [ ] `crates/ath-planner/src/input/error.rs` -- InputError enum
- [ ] `crates/ath-planner/src/input/prompt.rs` -- LLM prompt templates
- [ ] `crates/ath-planner/src/input/spec_file.rs` -- spec file reader
- [ ] `crates/ath-planner/src/input/codebase.rs` -- codebase scanner
- [ ] Add `ignore` to workspace dependencies
- [ ] Add `tokio`, `genai`, `ath-agents`, `serde_json`, `ignore` to ath-planner Cargo.toml
- [ ] Tests use MockBackend for LLM calls (no real API needed)

## Sources

### Primary (HIGH confidence)
- genai docs.rs -- ChatOptions, ChatResponseFormat, JsonSpec struct and usage (https://docs.rs/genai/latest/genai/chat/index.html)
- Existing codebase -- ProjectSpec, AgentBackend, ClaudeHandle, MockBackend, call_provider signatures verified by reading source
- ignore crate docs (https://docs.rs/ignore/latest/ignore/) -- WalkBuilder API for gitignore-respecting traversal

### Secondary (MEDIUM confidence)
- genai JsonSpec provider support -- docs say "only applied when provider supports it"; Anthropic Claude supports JSON mode natively
- ignore crate version 0.4.25 latest (https://crates.io/crates/ignore)

### Tertiary (LOW confidence)
- None -- all findings verified against source code or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in workspace or well-established (ignore)
- Architecture: HIGH - extends existing patterns (AgentBackend, error types, CLI structure)
- Pitfalls: HIGH - identified from direct code reading (AgentRequest lacks json_schema, call_provider passes None for options)

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no fast-moving dependencies)
