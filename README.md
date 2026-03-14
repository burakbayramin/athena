# Athena

Multi-agent AI orchestrator — takes a project idea and builds it by coordinating AI agents working in parallel.

## Install

**Requires:** [Rust toolchain](https://rustup.rs/) (1.75+)

```sh
git clone https://github.com/burakbayramin/athena.git
cd athena
cargo install --path crates/ath-cli
```

## Setup

```sh
mkdir my-project && cd my-project
ath init
```

Edit `.ath/agents.toml` — set your provider:

```toml
[[agents]]
provider = "google"
model = "gemini-2.5-flash"
api_key_env = "GEMINI_API_KEY"
```

Set the API key:

```sh
# Linux/macOS
export GEMINI_API_KEY="your-key"

# Windows (PowerShell)
$env:GEMINI_API_KEY = "your-key"

# Or use a .env file (auto-loaded)
echo 'GEMINI_API_KEY=your-key' > .env
```

## Run

```sh
ath run "build a CLI calculator in Rust"
```

## Supported Providers

| Provider | `provider` | API Key Env Var | Example Model |
|----------|-----------|-----------------|---------------|
| Google (Gemini) | `google` | `GEMINI_API_KEY` | gemini-2.5-flash |
| Anthropic (Claude) | `anthropic` | `ANTHROPIC_API_KEY` | claude-sonnet-4-20250514 |
| OpenAI | `openai` | `OPENAI_API_KEY` | gpt-4o-mini |
| Local (Ollama) | `ollama` | — | llama3.3 |

For Ollama, add `base_url = "http://localhost:11434/v1"` to the agent config.

## License

MIT
