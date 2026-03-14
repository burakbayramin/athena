use std::fs;
use std::path::Path;

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::GlobalArgs;

#[derive(Debug, Args, Clone, PartialEq, Eq, Default)]
#[command(about = "Initialize Athena in the current project directory.")]
pub(crate) struct InitArgs {
    /// Force overwrite of existing config files.
    #[arg(long, default_value_t = false)]
    force: bool,
}

/// Example agents.toml content.
const EXAMPLE_AGENTS_TOML: &str = r#"# Athena Agent Configuration
# Define which AI agents are available for task execution and review.
# See: https://github.com/burakbayramin/athena#agents-configuration

# Built-in providers: set the model and ensure the API key env var is set.
# Claude (Anthropic) — requires ANTHROPIC_API_KEY
[[agents]]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

# Gemini (Google) — requires GOOGLE_API_KEY
[[agents]]
provider = "google"
model = "gemini-2.5-pro"
api_key_env = "GOOGLE_API_KEY"

# Codex (OpenAI) — requires OPENAI_API_KEY
# [[agents]]
# provider = "openai"
# model = "o3"
# api_key_env = "OPENAI_API_KEY"

# Custom OpenAI-compatible endpoint (e.g., Ollama, Groq, Together)
# [[agents]]
# provider = "ollama"
# model = "llama3.3"
# base_url = "http://localhost:11434/v1"
# No api_key_env needed for local Ollama
"#;

/// Example skills.toml content.
const EXAMPLE_SKILLS_TOML: &str = r#"# Athena Skill Routing Configuration
# Map skill tags to preferred agents for task assignment.
# See: https://github.com/burakbayramin/athena#skills-configuration

# Example: route Rust tasks to Claude, Python to Gemini
# [[routes]]
# tag = "rust"
# provider = "anthropic"
# model = "claude-sonnet-4-20250514"

# [[routes]]
# tag = "python"
# provider = "google"
# model = "gemini-2.5-pro"
"#;

/// Known API key environment variables to check.
const API_KEY_CHECKS: &[(&str, &str)] = &[
    ("ANTHROPIC_API_KEY", "Anthropic (Claude)"),
    ("GOOGLE_API_KEY", "Google (Gemini)"),
    ("OPENAI_API_KEY", "OpenAI (Codex)"),
];

pub(crate) fn init_command(args: InitArgs, _global: GlobalArgs) -> Result<()> {
    init_at(Path::new("."), args.force)
}

/// Initialize Athena at the given base directory.
pub(crate) fn init_at(base: &Path, force: bool) -> Result<()> {
    let ath_dir = base.join(".ath");
    let memory_dir = ath_dir.join("memory");
    let agents_path = ath_dir.join("agents.toml");
    let skills_path = ath_dir.join("skills.toml");

    // Create directories
    if !ath_dir.exists() {
        fs::create_dir_all(&ath_dir)?;
        println!("{} {}", "Created".green(), ath_dir.display());
    } else {
        println!(
            "{} {} (already exists)",
            "Skipped".yellow(),
            ath_dir.display()
        );
    }

    if !memory_dir.exists() {
        fs::create_dir_all(&memory_dir)?;
        println!("{} {}", "Created".green(), memory_dir.display());
    }

    // Write config files
    write_config_file(&agents_path, EXAMPLE_AGENTS_TOML, force)?;
    write_config_file(&skills_path, EXAMPLE_SKILLS_TOML, force)?;

    // Check API keys
    println!();
    println!("{}", "API Key Status:".bold());
    let mut any_found = false;
    for (env_var, provider_name) in API_KEY_CHECKS {
        if std::env::var(env_var).is_ok() {
            println!("  {} {} ({})", "✓".green(), provider_name, env_var);
            any_found = true;
        } else {
            println!("  {} {} ({} not set)", "✗".red(), provider_name, env_var);
        }
    }

    if !any_found {
        println!();
        println!(
            "{}",
            "No API keys detected. Set at least one to use Athena.".yellow()
        );
        println!("  Example: export ANTHROPIC_API_KEY=sk-ant-...");
    }

    println!();
    println!("{}", "Athena initialized. Next steps:".bold());
    println!("  1. Edit .ath/agents.toml to configure your agents");
    println!("  2. Set API keys in your environment");
    println!("  3. Run: ath run --input \"your project description\"");

    Ok(())
}

fn write_config_file(path: &Path, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        println!(
            "{} {} (use --force to overwrite)",
            "Skipped".yellow(),
            path.display()
        );
    } else {
        fs::write(path, content)?;
        let action = if force && path.exists() {
            "Overwrote"
        } else {
            "Created"
        };
        println!("{} {}", action.green(), path.display());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn init_placeholder_message() -> &'static str {
    "Run `ath init` to set up Athena in your project."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_agents_toml_is_valid_toml() {
        let _: toml::Value =
            toml::from_str(EXAMPLE_AGENTS_TOML).expect("agents.toml should be valid TOML");
    }

    #[test]
    fn example_skills_toml_is_valid_toml() {
        let _: toml::Value =
            toml::from_str(EXAMPLE_SKILLS_TOML).expect("skills.toml should be valid TOML");
    }

    #[test]
    fn init_creates_directory_structure() {
        let dir = tempfile::tempdir().unwrap();

        let result = init_at(dir.path(), false);
        assert!(result.is_ok());

        assert!(dir.path().join(".ath").exists());
        assert!(dir.path().join(".ath/memory").exists());
        assert!(dir.path().join(".ath/agents.toml").exists());
        assert!(dir.path().join(".ath/skills.toml").exists());
    }

    #[test]
    fn init_does_not_overwrite_without_force() {
        let dir = tempfile::tempdir().unwrap();

        // Create initial files
        fs::create_dir_all(dir.path().join(".ath")).unwrap();
        fs::write(dir.path().join(".ath/agents.toml"), "custom content").unwrap();

        let result = init_at(dir.path(), false);
        assert!(result.is_ok());

        // Custom content should be preserved
        let content = fs::read_to_string(dir.path().join(".ath/agents.toml")).unwrap();
        assert_eq!(content, "custom content");
    }

    #[test]
    fn init_overwrites_with_force() {
        let dir = tempfile::tempdir().unwrap();

        // Create initial files
        fs::create_dir_all(dir.path().join(".ath")).unwrap();
        fs::write(dir.path().join(".ath/agents.toml"), "custom content").unwrap();

        let result = init_at(dir.path(), true);
        assert!(result.is_ok());

        // Should be overwritten
        let content = fs::read_to_string(dir.path().join(".ath/agents.toml")).unwrap();
        assert!(content.contains("Athena Agent Configuration"));
    }
}
