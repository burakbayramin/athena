# Phase 4: Input Parsing - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Accept project input in three modes (natural language description, markdown spec file, existing codebase pointer) and produce a normalized ProjectSpec. Delivers: CLI flag surface for input modes, LLM-based parsing pipeline, spec file reader, codebase scanner, and ProjectSpec validation with structured errors. No phase decomposition, no agent routing, no execution logic.

</domain>

<decisions>
## Implementation Decisions

### CLI Invocation Design
- Flags on `ath run`: positional arg for natural language, `--spec ./file.md` for spec files, `--codebase ./path` for existing projects
- Input is required — error with clear message if no description, --spec, or --codebase provided
- Combining allowed: `--codebase ./project "add auth"` gives codebase context + user intent. `--spec` is exclusive (already a full definition)
- After parsing, show ProjectSpec summary for user confirmation before proceeding. Phase 8 will add `--yes` flag for CI/scripting
- No interactive prompt mode — always require explicit input

### LLM Parsing Strategy
- Claude is the default parsing agent (best at structured reasoning and JSON schema adherence)
- On malformed JSON or failed ProjectSpec validation: retry with the validation error appended to prompt, max 3 retries (matches Phase 2 retry-with-context pattern)
- Infer target_language and target_framework when obvious from description ("React dashboard" → TypeScript/React). Leave as None when ambiguous
- Expand short descriptions into 3-8 concrete goals with skill tags — give the phase decomposer (Phase 5) real structure to work with
- Use JSON mode (Claude tool_use) for guaranteed structure

### Spec File Format
- Markdown only — no YAML, JSON, or TOML spec files
- LLM-parsed: any freeform markdown works, the LLM reads and extracts ProjectSpec (same pipeline as natural language, but from file content)
- File size cap at ~50KB (~12K tokens). Warn and refuse if larger — prevents accidental cost explosions
- `ath init` generates an optional example spec.md template alongside config for discoverability

### Codebase Analysis Scope
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

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ProjectSpec` (ath-types/src/project.rs): Already defined with name, description, goals, constraints, target_language, target_framework, expected_files. Has validate() method
- `GoalSpec` / `SkillTag` (ath-types/src/project.rs): Goal structure with skill tags — LLM output maps directly to these
- `AgentBackend` trait + Claude actor (ath-agents): Ready-to-use LLM client with retry, backoff, circuit breaker
- `ConfigStore` (ath-config): Has API keys — needed to initialize agent for parsing
- `Cli` struct (ath-cli/src/main.rs): Already has `Run` subcommand with `description: Option<String>` — extend with --spec and --codebase flags
- `display_error` (ath-cli/src/main.rs): Error display with fix hints — reuse for parsing errors

### Established Patterns
- `thiserror` for domain errors, `anyhow` at binary boundary
- `validate()` returns `Result<(), ValidationError>` with fix hints
- JSON serialization for all inter-agent communication
- Agent actors use JSON mode for structured output

### Integration Points
- ath-cli `Commands::Run` — add --spec and --codebase args here
- Agent actor system (ath-agents) — call Claude for parsing
- ProjectSpec feeds into Phase 5 (Phase Decomposition) as input
- Error types follow existing pattern: domain-specific enum with fix hints

</code_context>

<specifics>
## Specific Ideas

- The confirmation step after parsing mirrors how `cargo publish` shows what will be published before proceeding — familiar pattern for Rust CLI users
- Retry-with-feedback for malformed LLM output is proven (Phase 2 context decision) — the model almost always fixes on second try
- Codebase scanning should feel fast — tree traversal + selective reads, not a full codebase indexing operation

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-input-parsing*
*Context gathered: 2026-03-12*
