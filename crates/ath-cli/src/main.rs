//! # ath
//!
//! CLI entry point for the Athena multi-agent orchestrator.

use anyhow::Result;
use clap::{Parser, Subcommand};

// Validate dependency wiring at compile time.
use ath_config as _;
use ath_types as _;

/// Athena — a multi-agent software orchestrator.
#[derive(Parser)]
#[command(name = "ath", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Conduct a multi-agent run on the current project.
    Run,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run) => {
            println!("Run not yet implemented.");
        }
        None => {
            println!("Use `ath --help` for usage information.");
        }
    }

    Ok(())
}
