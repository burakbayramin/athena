//! # ath
//!
//! CLI entry point for the Athena multi-agent orchestrator.

use anyhow::Result;
use ath_config::{ConfigError, ConfigStore};
use colored::Colorize;
use clap::{Parser, Subcommand};

// Validate ath-types dependency wiring at compile time.
use ath_types as _;

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
        /// Optional task description for the run.
        description: Option<String>,
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
        Some(Commands::Run { description }) => {
            if let Some(desc) = &description {
                println!("Run description: {desc}");
            }
            println!("Run not yet implemented.");
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
