use anyhow::Result;
use clap::Args;

use crate::GlobalArgs;

#[derive(Debug, Args, Clone, PartialEq, Eq, Default)]
#[command(about = "Show a human-readable report for the latest run or a specific target.")]
pub(crate) struct ReportArgs {
    /// Optional run identifier or path to report on.
    #[arg(value_name = "TARGET")]
    pub(crate) target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReportTarget {
    LatestRun,
    Explicit(String),
}

pub(crate) fn report_command(args: ReportArgs, _global: GlobalArgs) -> Result<()> {
    let target = resolve_target(args.target.as_deref());
    println!("{}", report_placeholder_message(&target));
    Ok(())
}

pub(crate) fn resolve_target(target: Option<&str>) -> ReportTarget {
    match target {
        Some(target) => ReportTarget::Explicit(target.to_string()),
        None => ReportTarget::LatestRun,
    }
}

pub(crate) fn report_placeholder_message(target: &ReportTarget) -> String {
    match target {
        ReportTarget::LatestRun => {
            "Report generation is not wired yet. When implemented, `ath report` will summarize the latest run in the current project by default.".to_string()
        }
        ReportTarget::Explicit(target) => format!(
            "Report generation is not wired yet. When implemented, `ath report {target}` will summarize that explicit run target."
        ),
    }
}
