//! # ath
//!
//! CLI entry point for the Athena multi-agent orchestrator.

mod agents;
mod cost;
mod dry_run;
mod init;
mod memory;
mod progress;
mod report;
mod run;
mod verbose;

use anyhow::Result;
use ath_agents::AgentError;
use ath_config::ConfigError;
use ath_memory::MemoryError;
use ath_orchestrator::error::PhaseRunnerError;
use ath_types::ValidationError;
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
    /// Inspect and manage the Viking memory store.
    Memory(memory::MemoryArgs),
    /// Inspect and manage configured AI agents.
    Agents(agents::AgentsArgs),
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
        Some(Commands::Memory(args)) => memory::memory_command(args, global),
        Some(Commands::Agents(args)) => agents::agents_command(args, global),
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

#[cfg(test)]
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
    eprint!("{}", render_error_output(err));
}

fn render_error_output(err: &anyhow::Error) -> String {
    let mut out = format!("{} {}\n", "Error:".red(), err);

    if let Some(hint) = actionable_hint(err) {
        out.push_str(&format!("  {} {}\n", "Fix:".yellow(), hint));
    }

    let mut chain = err.chain().skip(1);
    if let Some(first) = chain.next() {
        out.push('\n');
        out.push_str(&format!("{}\n", "Caused by:".dimmed()));
        out.push_str(&format!("  0: {first}\n"));
        for (index, cause) in chain.enumerate() {
            out.push_str(&format!("  {}: {}\n", index + 1, cause));
        }
    }

    out
}

fn actionable_hint(err: &anyhow::Error) -> Option<String> {
    if let Some(config_err) = find_cause::<ConfigError>(err) {
        return Some(config_err.hint().to_string());
    }

    if let Some(agent_err) = find_cause::<AgentError>(err) {
        return Some(agent_err.hint());
    }

    if let Some(phase_err) = find_cause::<PhaseRunnerError>(err) {
        return Some(phase_err.hint());
    }

    if let Some(validation_err) = find_cause::<ValidationError>(err) {
        return Some(validation_err.hint().to_string());
    }

    if let Some(memory_err) = find_cause::<MemoryError>(err) {
        return Some(memory_err.hint().to_string());
    }

    None
}

fn find_cause<T: std::error::Error + 'static>(err: &anyhow::Error) -> Option<&T> {
    err.chain().find_map(|cause| cause.downcast_ref::<T>())
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

#[cfg(test)]
mod error_display {
    use super::*;

    mod tests {
        use super::*;

        #[test]
        fn provider_auth_errors_show_configuration_fix() {
            let err = anyhow::Error::new(ath_agents::AgentError::AuthFailed {
                provider: "Anthropic".into(),
                reason: "invalid API key".into(),
            });

            let output = render_error_output(&err);
            assert!(output.contains("Authentication failed for Anthropic"));
            assert!(output.contains("ANTHROPIC_API_KEY"));
            assert!(output.contains("anthropic_api_key"));
        }

        #[test]
        fn review_failures_show_phase_reviewer_attempt_and_reason() {
            let err = anyhow::Error::new(
                ath_orchestrator::error::PhaseRunnerError::MaxRetriesExceeded {
                    phase_name: "build-phase".into(),
                    reviewer: "Google/2.5-pro".into(),
                    attempts: 3,
                    final_reason: "tests still failing".into(),
                },
            );

            let output = render_error_output(&err);
            assert!(output.contains("build-phase"));
            assert!(output.contains("Google/2.5-pro"));
            assert!(output.contains("attempt 3"));
            assert!(output.contains("tests still failing"));
        }

        #[test]
        fn validation_errors_show_field_and_fix_guidance() {
            let err = anyhow::Error::new(ath_types::ValidationError::invalid_value_with_received(
                "phase_id",
                "must be numeric",
                "\"abc\"",
                "Use an integer phase identifier",
            ));

            let output = render_error_output(&err);
            assert!(output.contains("phase_id"));
            assert!(output.contains("\"abc\""));
            assert!(output.contains("Use an integer phase identifier"));
        }

        #[test]
        fn config_error_fix_behavior_still_works() {
            let err = anyhow::Error::new(ConfigError::NoConfigDir);
            let output = render_error_output(&err);
            assert!(output.contains("Could not determine config directory"));
            assert!(output.contains("HOME"));
        }

        #[test]
        fn memory_error_shows_actionable_hint() {
            let err = anyhow::Error::new(MemoryError::InvalidUri {
                input: "bad://uri".into(),
                reason: "wrong scheme".into(),
            });
            let output = render_error_output(&err);
            assert!(output.contains("bad://uri"), "should show the invalid URI");
            assert!(output.contains("Fix:"), "should show a fix hint");
            assert!(
                output.contains("viking://"),
                "hint should mention the correct scheme"
            );
        }

        #[test]
        fn memory_io_error_shows_fix_hint() {
            let err = anyhow::Error::new(MemoryError::IoError {
                path: "/tmp/missing".into(),
                message: "not found".into(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "gone"),
            });
            let output = render_error_output(&err);
            assert!(output.contains("Fix:"));
            assert!(output.contains(".ath/memory/"));
        }
    }
}
