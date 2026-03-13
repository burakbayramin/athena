//! # ath
//!
//! CLI entry point for the Athena multi-agent orchestrator.

mod dry_run;
mod init;
mod progress;
mod report;
mod run;
mod verbose;

use anyhow::Result;
use ath_config::ConfigError;
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

/// Athena - a multi-agent software orchestrator.
#[derive(Debug, Parser)]
#[command(
    name = "ath",
    version,
    about = "Multi-agent AI orchestrator",
    after_help = "Examples:\n  ath run \"build a todo API\"\n  ath run --spec project.md\n  ath report"
)]
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

#[derive(Debug, Subcommand)]
enum Commands {
    Run(run::RunArgs),
    Init(init::InitArgs),
    Report(report::ReportArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlobalArgs {
    pub(crate) verbose: bool,
    pub(crate) no_color: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Respect --no-color flag and NO_COLOR env var.
    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    if let Err(err) = dispatch(cli).await {
        display_error(&err);
        std::process::exit(1);
    }
}

async fn dispatch(cli: Cli) -> Result<()> {
    let global = GlobalArgs {
        verbose: cli.verbose,
        no_color: cli.no_color,
    };

    match cli.command {
        Some(Commands::Run(args)) => run::run_command(args, global).await,
        Some(Commands::Init(args)) => init::init_command(args, global),
        Some(Commands::Report(args)) => report::report_command(args, global),
        None => {
            print!("{}", root_help_text());
            Ok(())
        }
    }
}

fn root_help_text() -> String {
    let mut command = Cli::command();
    command.render_help().to_string()
}

fn render_subcommand_help(name: &str) -> String {
    let mut command = Cli::command();
    let subcommand = command
        .find_subcommand_mut(name)
        .unwrap_or_else(|| panic!("missing subcommand: {name}"));
    subcommand.render_help().to_string()
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

#[cfg(test)]
mod cli_surface {
    use super::*;

    mod tests {
        use super::*;

        #[test]
        fn clap_shape_is_valid() {
            Cli::command().debug_assert();
        }

        #[test]
        fn root_help_shows_usage_and_examples() {
            let help = root_help_text();

            assert!(help.contains("Usage:"), "help should contain usage");
            assert!(help.contains("Examples:"), "help should contain examples");
            assert!(help.contains("ath run"), "help should mention run");
        }

        #[test]
        fn run_help_mentions_inputs_and_dry_run() {
            let help = render_subcommand_help("run");

            assert!(help.contains("--spec"), "run help should mention --spec");
            assert!(
                help.contains("--codebase"),
                "run help should mention --codebase"
            );
            assert!(
                help.contains("--dry-run"),
                "run help should mention --dry-run"
            );
            assert!(
                help.contains("[DESCRIPTION]"),
                "run help should mention the description arg"
            );
        }

        #[test]
        fn init_and_report_have_distinct_help_text() {
            let init_help = render_subcommand_help("init");
            let report_help = render_subcommand_help("report");

            assert!(
                init_help.contains("interactive"),
                "init help should describe interactive setup"
            );
            assert!(
                report_help.contains("latest run"),
                "report help should mention latest-run behavior"
            );
        }

        #[test]
        fn parse_run_dry_run_flag() {
            let cli = Cli::try_parse_from(["ath", "run", "--dry-run"]).expect("parse run");

            match cli.command {
                Some(Commands::Run(args)) => assert!(args.dry_run),
                other => panic!("expected run command, got {other:?}"),
            }
        }

        #[test]
        fn init_placeholder_mentions_example_spec() {
            let message = init::init_placeholder_message();
            assert!(
                message.contains("example spec"),
                "init placeholder should mention the example spec flow"
            );
        }

        #[test]
        fn report_target_defaults_to_latest_run() {
            let target = report::resolve_target(None);
            assert!(matches!(target, report::ReportTarget::LatestRun));
        }

        #[test]
        fn report_target_respects_explicit_target() {
            let target = report::resolve_target(Some("run-42"));
            assert_eq!(target, report::ReportTarget::Explicit("run-42".into()));
        }
    }
}
