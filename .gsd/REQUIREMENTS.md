# Requirements

<!-- Living document tracking requirement status across milestones -->

## Validated

| ID | Requirement | Validated By | Milestone |
|----|-------------|-------------|-----------|
| REQ-ANALYZE | Analyze project input into structured phases | M001 integration tests | M001 |
| REQ-DEPS | Dependency analysis for phase ordering and parallelization | M001 integration tests | M001 |
| REQ-SKILLS | Map required skills and assign to appropriate AI agents | M001 integration tests | M001 |
| REQ-ISOLATION | Module isolation — agents work on separate files/modules | M001 integration tests | M001 |
| REQ-EXEC | Call Claude, Gemini, and Codex APIs autonomously | M001 integration tests | M001 |
| REQ-GIT | Write generated code to local git repo with commits per phase | M001 integration tests | M001 |
| REQ-REVIEW | Cross-review between phases with review gate | M001 integration tests | M001 |
| REQ-RETRY | Auto-retry on review failure with reviewer feedback (max 3) | M001 integration tests | M001 |
| REQ-REPORT | Structured final report with phase table, costs, outcomes | M001 integration tests | M001 |
| REQ-APIKEYS | Accept user-provided API keys via env vars or config | M001 integration tests | M001 |
| REQ-PROGRESS | Real-time terminal progress showing phase/agent/task status | M001 integration tests | M001 |
| REQ-DRYRUN | Dry-run mode to preview plan without execution | M001 integration tests | M001 |
| REQ-PARALLEL | Parallel execution of independent phases via tokio JoinSet | M001 integration tests | M001 |
| REQ-ERRORS | Actionable error messages for API errors, review failures, schema violations | M001 integration tests | M001 |
| REQ-MEM-PERSIST | Memory persistence across runs in `.ath/memory/` | VikingStore markdown persistence + two-run e2e test (S06) | M002 |
| REQ-MEM-INJECT | Automatic context injection from prior runs into agent prompts | ContextInjector + two-run e2e test proving Run 2 contains Run 1 context (S04/S06) | M002 |
| REQ-MEM-CLI | CLI inspection, search, and manual memory entry | 6 subcommands with 21 passing tests (S05) | M002 |
| REQ-MEM-BUDGET | Token budget enforcement for injected context | InjectionConfig with configurable limits, 13 injector tests (S04) | M002 |
| REQ-MEM-KEYWORD | Keyword fallback when no embedding API available | KeywordIndex with TF scoring, 7 tests, full system works without embedding (S01) | M002 |
| REQ-RESUME | Resumable execution from last completed phase | Checkpoint-based resume with integration tests, --fresh/--status CLI flags (S01-S03) | M003 |

## Out of Scope

| ID | Requirement | Reason |
|----|-------------|--------|
| REQ-WEB-UI | Web dashboard UI | CLI only for v1 |
| REQ-MANAGED-KEYS | Built-in/managed API keys (SaaS model) | User provides their own |
| REQ-PR-WORKFLOW | PR-based workflow | Direct commits to local repo |
| REQ-MOBILE | Mobile app | CLI distribution only |
| REQ-COLLAB | Real-time collaboration | Single-user tool |

| REQ-PLUGINS | Plugin system for custom agent definitions | v2 scope |
| REQ-LOCAL-MODELS | Support for local/self-hosted models | v2 scope |
| REQ-CROSS-PROJECT-MEM | Cross-project memory sharing | v3 scope |
| REQ-MEM-TUI | Interactive TUI memory editor | v3 scope |
| REQ-MEM-DIFF | Memory diffing between runs | v3 scope |
| REQ-LOCAL-EMBED | Local/self-hosted embedding models | v3 scope |
