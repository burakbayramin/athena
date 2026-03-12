//! # ath
//!
//! CLI entry point for the Athena multi-agent orchestrator.

use std::path::PathBuf;

use anyhow::Result;
use ath_config::{ConfigError, ConfigStore};
use colored::Colorize;
use clap::{Parser, Subcommand};

/// Athena - a multi-agent software orchestrator.
#[derive(Parser)]
#[command(name = "ath", version, about = "Multi-agent AI orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose output (show provider details, full error chains)
    #[arg(long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Conduct a multi-agent run on the current project.
    Run {
        /// Project description in natural language.
        description: Option<String>,
        /// Path to a markdown spec file.
        #[arg(long, value_name = "FILE")]
        spec: Option<PathBuf>,
        /// Path to an existing codebase to analyze.
        #[arg(long, value_name = "DIR")]
        codebase: Option<PathBuf>,
    },
    /// Initialize an ath project in the current directory.
    Init,
    /// Generate a report on the latest run.
    Report,
}

fn main() {
    let cli = Cli::parse();

    // Respect --no-color flag and NO_COLOR env var.
    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    if let Err(err) = run(cli) {
        display_error(&err);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let config = match ConfigStore::load() {
        Ok(c) => c,
        Err(e) => {
            // Wrap ConfigError into anyhow while preserving it for downcasting.
            return Err(anyhow::Error::new(e));
        }
    };

    if cli.verbose {
        print_provider_status(&config);
    }

    match cli.command {
        Some(Commands::Run { description, spec, codebase }) => {
            use ath_planner::input::{InputMode, resolve_input_mode};
            let mode = resolve_input_mode(
                description.as_deref(),
                spec.as_deref(),
                codebase.as_deref(),
            ).map_err(|e| anyhow::anyhow!("{}", e))?;
            match &mode {
                InputMode::NaturalLanguage(desc) => println!("Input: natural language -- {}", desc),
                InputMode::SpecFile(path) => println!("Input: spec file -- {}", path.display()),
                InputMode::Codebase { path, intent } => {
                    println!("Input: codebase -- {}", path.display());
                    if let Some(i) = intent {
                        println!("Intent: {}", i);
                    }
                }
            }
            println!("Input parsing not yet fully implemented.");
        }
        Some(Commands::Init) => {
            println!("Init not yet implemented.");
        }
        Some(Commands::Report) => {
            println!("Report not yet implemented.");
        }
        None => {
            println!("Use `ath --help` for usage information.");
        }
    }

    Ok(())
}

/// Print which providers are configured and which are not.
fn print_provider_status(config: &ConfigStore) {
    let status = |configured: bool| -> &str {
        if configured { "configured" } else { "not configured" }
    };

    println!(
        "Providers: Anthropic ({}), Google ({}), OpenAI ({})",
        status(config.anthropic_api_key.is_some()),
        status(config.google_api_key.is_some()),
        status(config.openai_api_key.is_some()),
    );
}

/// Display an error with the project's standard error style:
/// - Red "Error:" prefix with the message
/// - If the underlying error is a ConfigError, show the fix hint
/// - If verbose mode was used, show the full error chain
fn display_error(err: &anyhow::Error) {
    eprintln!("{} {}", "Error:".red(), err);

    // If the root cause is a ConfigError, show the fix hint.
    if let Some(config_err) = err.downcast_ref::<ConfigError>() {
        eprintln!("  {} {}", "Fix:".yellow(), config_err.hint());
    }

    // Show the error chain for debugging (verbose is implicit when there's a chain).
    let err_ref: &(dyn std::error::Error + 'static) = err.as_ref();
    let mut source = err_ref.source();
    if source.is_some() {
        eprintln!();
        eprintln!("{}", "Caused by:".dimmed());
        let mut i = 0;
        while let Some(cause) = source {
            eprintln!("  {i}: {cause}");
            source = std::error::Error::source(cause);
            i += 1;
        }
    }
}
