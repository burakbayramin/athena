use std::path::PathBuf;

use anyhow::{anyhow, Result};
use ath_agents::ClaudeHandle;
use ath_config::ConfigStore;
use ath_planner::decompose::{decompose_project_spec, display_execution_plan};
use ath_planner::input::{display_project_spec_summary, parse_input, resolve_input_mode};
use clap::Args;
use colored::Colorize;

use crate::GlobalArgs;

#[derive(Debug, Args, Clone, PartialEq, Eq)]
#[command(about = "Conduct a multi-agent run on the current project.")]
pub(crate) struct RunArgs {
    /// Project description in natural language.
    #[arg(value_name = "DESCRIPTION")]
    pub(crate) description: Option<String>,

    /// Path to a markdown spec file.
    #[arg(long, value_name = "FILE")]
    pub(crate) spec: Option<PathBuf>,

    /// Path to an existing codebase to analyze.
    #[arg(long, value_name = "DIR")]
    pub(crate) codebase: Option<PathBuf>,

    /// Show the locally available execution plan without running agents.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

pub(crate) async fn run_command(args: RunArgs, global: GlobalArgs) -> Result<()> {
    if args.dry_run {
        anyhow::bail!("{}", dry_run_placeholder_message());
    }

    let config = ConfigStore::load().map_err(anyhow::Error::new)?;

    if global.verbose {
        print_provider_status(&config);
    }

    let mode = resolve_input_mode(
        args.description.as_deref(),
        args.spec.as_deref(),
        args.codebase.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;

    let backend = ClaudeHandle::new(&config).map_err(|e| anyhow!("{e}"))?;

    let project_spec = parse_input(mode, &backend)
        .await
        .map_err(|e| anyhow!("{e}"))?;

    display_project_spec_summary(&project_spec);

    let (plan, warnings) = decompose_project_spec(&project_spec, &backend)
        .await
        .map_err(|e| anyhow!("{e}"))?;

    display_execution_plan(&plan, &warnings);
    println!("{}", execution_placeholder_message().dimmed());

    Ok(())
}

pub(crate) fn execution_placeholder_message() -> &'static str {
    "Phase plan ready. Execution wiring is not implemented yet."
}

pub(crate) fn dry_run_placeholder_message() -> &'static str {
    "`ath run --dry-run` is recognized, but the local no-cost plan preview is not wired yet."
}

fn print_provider_status(config: &ConfigStore) {
    let status = |configured: bool| -> &str {
        if configured {
            "configured"
        } else {
            "not configured"
        }
    };

    println!(
        "Providers: Anthropic ({}), Google ({}), OpenAI ({})",
        status(config.anthropic_api_key.is_some()),
        status(config.google_api_key.is_some()),
        status(config.openai_api_key.is_some()),
    );
}
