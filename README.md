# Athena

Multi-agent AI orchestrator — takes a project idea and builds it by coordinating AI agents working in parallel.

Athena decomposes your input into dependency-aware phases, routes tasks to the best-fit AI agent, executes them in parallel, and gates each phase behind review before proceeding.

## Quick Start

### 1. Install

**Requires:** [Rust toolchain](https://rustup.rs/) (1.75+)

```sh
git clone https://github.com/burakbayramin/athena.git
cd athena
cargo install --path crates/ath-cli
```

This installs the `ath` binary to `~/.cargo/bin/` (already in PATH if you use rustup).

### 2. Initialize a project

```sh
mkdir my-project && cd my-project
ath init
```

This creates `.ath/agents.toml` and `.ath/skills.toml` with example configs.

### 3. Configure an API key

Edit `.ath/agents.toml` — uncomment/add the provider you want to use:

```toml
[[agents]]
provider = "google"
model = "gemini-2.5-flash"
api_key_env = "GEMINI_API_KEY"
```

Then set the API key in your environment:

```sh
# Linux/macOS
export GEMINI_API_KEY="your-key-here"

# Windows (PowerShell)
$env:GEMINI_API_KEY = "your-key-here"

# Or put it in a .env file in the project root (auto-loaded)
echo 'GEMINI_API_KEY=your-key-here' > .env
```

### 4. Run

```sh
ath run "build a CLI calculator in Rust"
```

Athena will:
1. Parse your input into a structured project spec
2. Decompose it into phases with dependencies
3. Route tasks to available agents
4. Execute each phase, review the output, commit results
5. Generate a run report

## Supported Providers

| Provider | `provider` value | API Key Env Var | Models |
|----------|-----------------|-----------------|--------|
| Anthropic (Claude) | `anthropic` | `ANTHROPIC_API_KEY` | claude-sonnet-4-20250514, opus-4 |
| Google (Gemini) | `google` | `GEMINI_API_KEY` or `GOOGLE_API_KEY` | gemini-2.5-pro, gemini-2.5-flash |
| OpenAI | `openai` | `OPENAI_API_KEY` | o3, gpt-4o-mini |
| Custom (OpenAI-compatible) | any name | any env var | any model |

### Using a local model (Ollama)

```toml
[[agents]]
provider = "ollama"
model = "llama3.3"
base_url = "http://localhost:11434/v1"
# No api_key_env needed
```

You need at least **one** agent with a working API key (or local endpoint) to run Athena.

## Usage

```sh
# Run from a description
ath run "build a REST API for a todo app with SQLite"

# Run from a spec file
ath run --spec project.md

# Run from an existing codebase
ath run --codebase ./my-project "add authentication"

# Preview the plan without executing
ath run --dry-run

# Resume an interrupted run (checkpoint is automatic)
ath run

# Start fresh, ignoring any checkpoint
ath run --fresh

# Check run status
ath run --status

# View the latest run report
ath report

# List configured agents
ath agents list

# Test agent connectivity
ath agents test

# Verbose output (full transcripts)
ath run --verbose "build something"
```

## Configuration

### agents.toml

Located at `.ath/agents.toml`. Defines which AI agents are available:

```toml
[[agents]]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[[agents]]
provider = "google"
model = "gemini-2.5-flash"
api_key_env = "GEMINI_API_KEY"

[[agents]]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
```

### skills.toml

Located at `.ath/skills.toml`. Optional — routes specific skill tags to preferred agents:

```toml
[[routes]]
tag = "rust"
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[[routes]]
tag = "python"
provider = "google"
model = "gemini-2.5-flash"
```

### Environment variables (legacy)

You can also set keys directly as env vars without `agents.toml`:

```sh
export ANTHROPIC_API_KEY="sk-ant-..."
export GEMINI_API_KEY="..."
export OPENAI_API_KEY="sk-..."
```

### Config file (legacy)

`~/.config/ath/config.toml` (global) or `.ath.toml` (project-local):

```toml
[providers]
anthropic_api_key = "sk-ant-..."
google_api_key = "..."
openai_api_key = "sk-..."
```

**Precedence:** env vars > `.ath/agents.toml` > project `.ath.toml` > global config > defaults.

## How It Works

```
Input (idea / spec / codebase)
  → Phase Decomposition (DAG with dependencies, parallel groups)
    → Skill-to-Agent Routing (taxonomy → available agents)
      → Parallel Execution (tokio tasks, file ownership tracking)
        → Review Gate (cross-agent when possible, self-review for single agent)
          → Checkpoint Save (resume on failure)
            → Run Report (token usage, cost estimates, review outcomes)
```

## Architecture

8-crate Rust workspace:

| Crate | Purpose |
|-------|---------|
| `ath-types` | Shared types: ProjectSpec, AgentRequest/Response, PhaseRecord, Conversations |
| `ath-config` | Layered config (env / TOML / defaults), AgentsConfig, SkillsConfig |
| `ath-agents` | AgentBackend trait, streaming, Claude/Gemini/Codex/Generic actor handles |
| `ath-git` | Git operations: staging, commits with trailers |
| `ath-planner` | Input parsing (3 modes), DAG decomposition, validation |
| `ath-orchestrator` | PhaseRunner, ReviewEngine, Coordinator, checkpoints, memory integration |
| `ath-memory` | Viking memory store, keyword index, vector search, context injection |
| `ath-cli` | CLI surface, progress bars, streaming output, dry-run, reporting |

## Development

```sh
cargo build --workspace          # Build all crates
cargo test --workspace           # Run 699 tests
cargo clippy --workspace -- -D warnings  # Lint
cargo fmt --all -- --check       # Format check
```

## License

MIT
