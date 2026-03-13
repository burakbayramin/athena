# Athena

AI orchestra conductor — takes a software project idea and autonomously builds it by coordinating Claude, Gemini, and Codex working in parallel.

Athena analyzes your input into dependency-aware phases, assigns tasks to the best-fit AI agent, enforces module isolation so agents never conflict, and gates each phase behind cross-agent review before proceeding.

## How It Works

```
Input (idea / spec / codebase)
  → Phase Decomposition (DAG with dependencies, parallel groups)
    → Skill-to-Agent Routing (15-tag taxonomy → Claude / Gemini / Codex)
      → Parallel Execution (tokio JoinSet, file ownership isolation)
        → Cross-Agent Review Gate (never self-review, max 3 retries)
          → Git Commit (per-phase, with agent metadata trailers)
            → Run Report (token usage, cost estimates, review outcomes)
```

## Install

### From source

```sh
cargo install --path crates/ath-cli
```

### From crates.io (when published)

```sh
cargo install ath-cli
```

## Configuration

Athena needs at least one AI provider API key. Set via environment variables or config file.

### Environment variables

```sh
export ANTHROPIC_API_KEY="sk-ant-..."    # Claude (recommended as primary)
export GOOGLE_API_KEY="..."               # Gemini
export OPENAI_API_KEY="sk-..."            # Codex / GPT
```

### Config file

Create `~/.config/athena/config.toml` (global) or `.ath.toml` (project-local):

```toml
[providers]
anthropic_key = "sk-ant-..."
google_key = "..."
openai_key = "sk-..."

[defaults]
claude_model = "claude-sonnet-4-20250514"
gemini_model = "gemini-2.5-pro"
codex_model = "o3"
```

Precedence: environment variables > project `.ath.toml` > global config > built-in defaults.

## Usage

### Run from a natural language description

```sh
ath run "build a REST API for a todo app with SQLite storage"
```

### Run from a spec file

```sh
ath run --spec project.md
```

### Run from an existing codebase

```sh
ath run --codebase ./my-project "add authentication"
```

### Preview the plan without executing

```sh
ath run --dry-run
```

### View the latest run report

```sh
ath report
```

### Verbose output (transcripts, provider details)

```sh
ath run --verbose "build a CLI calculator"
```

## Architecture

7-crate Rust workspace:

| Crate | Purpose |
|-------|---------|
| `ath-types` | Shared schemas: ProjectSpec, AgentRequest/Response, PhaseRecord, RunReport |
| `ath-config` | Layered config loading (env / TOML / defaults) |
| `ath-agents` | AgentBackend trait, CircuitBreaker, Claude/Gemini/Codex actor handles |
| `ath-git` | Git operations: staging, commits with trailers, async wrapper |
| `ath-planner` | Input parsing (3 modes), DAG decomposition, validation |
| `ath-orchestrator` | PhaseRunner typestate, ReviewEngine, AgentCoordinator, parallel dispatch |
| `ath-cli` | CLI surface, progress reporter, verbose transcripts, dry-run, reporting |

### Key design choices

- **Typestate pattern** for PhaseRunner — compile-time enforcement that review must happen before completion
- **Module isolation** — strict file ownership per agent prevents merge conflicts during parallel execution
- **Cross-agent review** — reviewer is always a different provider than the executor
- **Circuit breaker** per provider — fail-fast on unhealthy backends, prevents retry storms
- **DAG-based decomposition** — topological sort with parallel group detection and critical path computation

## Development

```sh
# Build
cargo build --workspace

# Test (415 tests)
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
```

## License

MIT
