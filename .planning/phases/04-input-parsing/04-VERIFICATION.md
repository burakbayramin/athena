---
phase: 04-input-parsing
verified: 2026-03-12T20:30:00Z
status: human_needed
score: 14/14 must-haves verified
re_verification:
  previous_status: gaps_found
  previous_score: 11/14
  gaps_closed:
    - "CLI wires all three input modes to parse_to_project_spec and displays ProjectSpec summary"
    - "ROADMAP Success Criterion 1: User can run `ath run '...'` and Athena produces a ProjectSpec"
    - "ROADMAP Success Criterion 2: User can run `ath run --spec ./spec.md` and Athena parses the markdown into ProjectSpec"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Run `ath run 'build a todo app'` with a configured Anthropic API key"
    expected: "A formatted ProjectSpec summary is printed to the terminal with name, goals, constraints, and expected files"
    why_human: "Requires a live LLM backend (API key); not testable without a real Claude API call"
  - test: "Run `ath run --spec ./spec.md` with a real markdown spec file and configured API key"
    expected: "The spec file is parsed into a ProjectSpec and displayed as a summary"
    why_human: "Requires live LLM call; size-cap enforcement verified in unit tests but end-to-end display needs human verification"
  - test: "Run `ath run --codebase ./some-project 'add authentication'` with a configured API key"
    expected: "The codebase is scanned, key files extracted, and a ProjectSpec is produced describing next steps"
    why_human: "Requires live LLM call; scan behavior is verified in unit tests but end-to-end output needs human verification"
  - test: "Run `ath run 'test'` without an Anthropic API key configured"
    expected: "Error message reads 'Authentication failed for claude: Anthropic API key not configured' (not a panic)"
    why_human: "Error message content and formatting needs human confirmation of clarity"
---

# Phase 4: Input Parsing Verification Report

**Phase Goal:** Athena accepts a project description in natural language, as a spec file, or as a pointer to an existing codebase, and produces a normalized ProjectSpec in all three cases
**Verified:** 2026-03-12T20:30:00Z
**Status:** human_needed
**Re-verification:** Yes — after gap closure via Plan 04

## Goal Achievement

### Observable Truths

All 14 truths are now verified. The three gaps from the initial verification (CLI not calling `parse_input`, `display_project_spec_summary` suppressed, no real backend construction) are fully closed by Plan 04.

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | InputMode enum discriminates NaturalLanguage, SpecFile, and Codebase variants | VERIFIED | `crates/ath-planner/src/input/mod.rs:28-39` — three variants with serde derives; round-trip tests pass |
| 2 | InputError enum covers all failure modes with fix hints | VERIFIED | `error.rs` has 9 variants with `hint()` method |
| 3 | CLI accepts --spec and --codebase flags on ath run | VERIFIED | `main.rs:35-39` — both flags declared with correct `#[arg(long, value_name)]` attributes |
| 4 | --spec is exclusive and cannot combine with description or --codebase | VERIFIED | `resolve_input_mode` enforces this; exclusivity tests pass |
| 5 | AgentRequest has optional json_schema field for structured output | VERIFIED | `agent.rs` — field with `#[serde(skip_serializing_if = "Option::is_none")]`; backward-compat tests pass |
| 6 | Natural language description is parsed into a valid ProjectSpec via LLM call | VERIFIED | `parse_to_project_spec` calls backend and parses JSON; `parse_succeeds_on_first_try_with_valid_json` passes with MockBackend |
| 7 | Malformed LLM output triggers retry with error feedback appended to prompt | VERIFIED | Retry loop at `mod.rs:106-129`; `parse_retries_on_invalid_json_then_succeeds` passes |
| 8 | After 3 failed retries, InputError::ParseFailed is returned | VERIFIED | `mod.rs:131-134`; `parse_fails_after_3_attempts_with_always_invalid_json` passes with `attempts: 3` |
| 9 | Markdown spec file under 50KB is read and parsed into ProjectSpec via same LLM pipeline | VERIFIED | `spec_file.rs` reads file; `parse_input` branches to `build_spec_file_request`; all spec file tests pass |
| 10 | Spec file over 50KB is rejected with SpecFileTooLarge error before reading | VERIFIED | `spec_file.rs:31-41` checks metadata size before read; size-cap test passes |
| 11 | Spec file that does not exist returns SpecFileNotFound error | VERIFIED | `spec_file.rs:19-28` maps NotFound IO error; not-found test passes |
| 12 | json_schema field on AgentRequest is threaded through call_provider to genai ChatOptions | VERIFIED | `actor/mod.rs` passes `request.json_schema.as_ref()` to `call_provider`; `call_provider` builds `ChatOptions::JsonSpec` when provided |
| 13 | Codebase directory is scanned respecting .gitignore patterns; known dirs skipped; key files extracted with size caps | VERIFIED | `codebase.rs` uses `WalkBuilder` with `filter_entry` for SKIP_DIRS; 11 tests pass including git skip, node_modules skip, truncation, total budget cap |
| 14 | CLI wires all three input modes to parse_to_project_spec and displays ProjectSpec summary | VERIFIED | `main.rs:77-93` — `resolve_input_mode` -> `ClaudeHandle::new` -> `parse_input(mode, &backend).await` -> `display_project_spec_summary(&project_spec)`. No placeholder code remains. No `let _ =` suppression. |

**Score:** 14/14 truths fully verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/ath-planner/src/input/mod.rs` | InputMode enum, resolve_input_mode, parse_to_project_spec, parse_input, display_project_spec_summary | VERIFIED | All five public items present; `parse_input` dispatches all three modes; `display_project_spec_summary` formats terminal output with bold labels |
| `crates/ath-planner/src/input/error.rs` | InputError enum with all variants | VERIFIED | 9 variants, `hint()` method, thiserror derives |
| `crates/ath-types/src/agent.rs` | AgentRequest with json_schema field | VERIFIED | Field with backward-compatible serde attributes |
| `crates/ath-cli/src/main.rs` | CLI wired to ClaudeHandle -> parse_input -> display_project_spec_summary | VERIFIED | `#[tokio::main] async fn main()`, Run arm at lines 76-93: resolve -> ClaudeHandle::new -> parse_input -> display. No deferral comments. No suppressed imports. |
| `crates/ath-cli/Cargo.toml` | ath-agents and tokio dependencies | VERIFIED | `ath-agents = { path = "../ath-agents" }` and `tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }` present |
| `crates/ath-planner/src/input/prompt.rs` | LLM system prompts and request builders for all 3 modes | VERIFIED | SYSTEM_PROMPT, SPEC_FILE_SYSTEM_PROMPT, CODEBASE_SYSTEM_PROMPT; three build_*_request functions; project_spec_json_schema() |
| `crates/ath-planner/src/input/spec_file.rs` | Spec file reader with 50KB size cap | VERIFIED | MAX_SPEC_FILE_SIZE = 51_200; read_spec_file checks metadata before reading |
| `crates/ath-planner/src/input/codebase.rs` | Codebase scanner with gitignore-aware traversal | VERIFIED | scan_codebase using WalkBuilder; is_key_file; size caps; 11 tests |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `crates/ath-cli/src/main.rs` | `crates/ath-agents/src/actor/claude.rs` | `ClaudeHandle::new(&config)` | VERIFIED | `main.rs:77` imports `ath_agents::ClaudeHandle`; `main.rs:86` calls `ClaudeHandle::new(&config)`. `ClaudeHandle::new` at `claude.rs:64` validates API key and returns `AgentError::AuthFailed` when missing. |
| `crates/ath-cli/src/main.rs` | `crates/ath-planner/src/input/mod.rs` | `parse_input(mode, &backend).await` | VERIFIED | `main.rs:78` imports `parse_input`; `main.rs:89` calls `parse_input(mode, &backend).await`. Actively called, not commented out. |
| `crates/ath-cli/src/main.rs` | `crates/ath-planner/src/input/mod.rs` | `display_project_spec_summary(&spec)` | VERIFIED | `main.rs:78` imports `display_project_spec_summary`; `main.rs:92` calls `display_project_spec_summary(&project_spec)`. Not suppressed. |
| `crates/ath-planner/src/input/error.rs` | `crates/ath-agents/src/error.rs` | InputError::Agent wraps AgentError | VERIFIED | `error.rs` — `Agent(#[from] ath_agents::error::AgentError)` |
| `crates/ath-planner/src/input/mod.rs` | `crates/ath-agents/src/backend.rs` | AgentBackend::send for LLM calls | VERIFIED | `mod.rs:108` — `backend.send(request).await` in parse_to_project_spec |
| `crates/ath-planner/src/input/prompt.rs` | `crates/ath-types/src/project.rs` | JSON schema matches ProjectSpec serde structure | VERIFIED | project_spec_json_schema() matches ProjectSpec fields |
| `crates/ath-agents/src/actor/mod.rs` | genai ChatOptions | json_schema threaded to ChatResponseFormat::JsonSpec | VERIFIED | `actor/mod.rs` — `JsonSpec::new("structured_output", schema.clone())` inside `call_provider` |
| `crates/ath-planner/src/input/codebase.rs` | ignore crate WalkBuilder | gitignore-respecting directory traversal | VERIFIED | `codebase.rs` — `ignore::WalkBuilder::new(path).hidden(true).git_ignore(true).git_global(true)` |
| `crates/ath-planner/src/input/mod.rs` | `crates/ath-planner/src/input/prompt.rs` | build_codebase_request for LLM parsing | VERIFIED | `mod.rs:170` — `build_codebase_request(&tree, &key_files, intent.as_deref(), last_err)` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| INPT-01 | 04-01, 04-02, 04-04 | User can describe a project in natural language and Athena parses it into structured intent | SATISFIED | Pipeline fully implemented and tested. CLI now wires NaturalLanguage mode through ClaudeHandle -> parse_input -> display_project_spec_summary. 33 unit tests with MockBackend pass. End-to-end requires configured API key (human verification). |
| INPT-02 | 04-01, 04-02, 04-04 | User can provide a spec file as project input | SATISFIED | read_spec_file, build_spec_file_request, and parse_to_project_spec exist and are tested. CLI resolves SpecFile mode and flows through parse_input. End-to-end requires configured API key. |
| INPT-03 | 04-01, 04-03, 04-04 | User can point Athena at an existing codebase to analyze and determine next steps | SATISFIED | scan_codebase and build_codebase_request implemented and tested. parse_input dispatches Codebase mode via spawn_blocking. CLI wires Codebase mode through ClaudeHandle -> parse_input. End-to-end requires configured API key. |

**No orphaned requirements:** REQUIREMENTS.md maps exactly INPT-01, INPT-02, INPT-03 to Phase 4. All three are claimed in plans and have full implementation evidence. INPT-04 is mapped to Phase 1, not Phase 4.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/ath-cli/src/main.rs` | 95-98 | `Init` and `Report` commands print "not yet implemented" | Info | These are separate commands, not part of the input parsing phase goal. They are correctly out of scope for Phase 4. |

No placeholder implementations, no `return null`, no `return {}`, no suppressed imports, no commented-out function calls in the Run arm. The `let _ = display_project_spec_summary` suppression from the previous verification is gone. The `parse_input` call is active and unconditional.

### Human Verification Required

#### 1. Natural Language to ProjectSpec (full round-trip)

**Test:** Run `ath run "build a REST API for a todo app"` with a configured `ANTHROPIC_API_KEY` environment variable
**Expected:** Terminal output shows a formatted ProjectSpec with a kebab-case name, numbered goals with skill tags in parentheses, target language/framework, constraints, and expected files
**Why human:** Requires a live Claude API key; LLM output quality (goal relevance, constraint extraction, name formatting) is not verifiable programmatically

#### 2. Spec File Parsing (full round-trip)

**Test:** Create a markdown spec file and run `ath run --spec ./spec.md` with a configured API key
**Expected:** The spec is parsed into the same ProjectSpec structure and the summary is displayed identically to the natural language path
**Why human:** Requires live LLM call; unit tests cover the size check and request building but not the LLM output quality or display formatting

#### 3. Codebase Analysis (full round-trip)

**Test:** Run `ath run --codebase ./some-project "add authentication"` pointing at a real project with a configured API key
**Expected:** Codebase tree appears in the prompt (verifiable via --verbose), key files are extracted, and the resulting ProjectSpec reflects the project's technology and proposes relevant next steps
**Why human:** Requires live LLM call; scan mechanics are fully tested but LLM interpretation quality is not verifiable programmatically

#### 4. Missing API Key Error Clarity

**Test:** Run `ath run "build a todo app"` without any `ANTHROPIC_API_KEY` configured
**Expected:** Terminal shows `Error: Authentication failed for claude: Anthropic API key not configured` — not a panic, not a generic error
**Why human:** Error message content and user experience clarity requires human confirmation; the error path is structurally verified (ClaudeHandle::new returns AgentError::AuthFailed when key is absent) but the displayed text depends on anyhow's formatting

### Re-Verification Summary

**Previous status:** gaps_found (11/14, 2026-03-12T15:10:00Z)

**Gaps closed by Plan 04 (`cc5aacb`):**

1. `parse_input` is now called from `main.rs:89` — confirmed by grep, not in a comment.
2. `display_project_spec_summary` is now called at `main.rs:92` — the `let _ =` suppression is absent.
3. `ClaudeHandle::new(&config)` is constructed at `main.rs:86` — the real AgentBackend is wired, not deferred.

**What changed in `crates/ath-cli/src/main.rs`:**
- `main()` converted from sync to `#[tokio::main] async fn main()`
- `run()` converted from sync to `async fn run(cli: Cli) -> Result<()>`
- `ath-agents` and `tokio` added to `crates/ath-cli/Cargo.toml`
- Run arm replaced placeholder `println!` calls with the full pipeline: `resolve_input_mode -> ClaudeHandle::new -> parse_input -> display_project_spec_summary`
- All Phase 7 deferral comments removed

**No regressions detected:** All previously-verified truths (1-13) remain intact. No previously-passing artifacts were modified in a degrading way.

---

_Verified: 2026-03-12T20:30:00Z_
_Verifier: Claude (gsd-verifier)_
_Mode: Re-verification (previous gaps_found 2026-03-12T15:10:00Z)_
