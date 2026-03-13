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
    let project_dir = std::env::current_dir()?;
    let target = resolve_target(args.target.as_deref());
    let report = load_report(&project_dir, &target)?;
    println!("{}", render_report(&report));
    Ok(())
}

pub(crate) fn resolve_target(target: Option<&str>) -> ReportTarget {
    match target {
        Some(target) => ReportTarget::Explicit(target.to_string()),
        None => ReportTarget::LatestRun,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::phase::PhaseRecord;
    use ath_types::report::RunReport;

    #[test]
    fn writes_run_report_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = sample_report("run-1");

        let path = write_run_report(&temp.path().to_path_buf(), &report).expect("write report");

        assert!(path.exists(), "report.json should exist");
        assert!(
            temp.path().join(".ath/runs/latest.txt").exists(),
            "latest-run pointer should exist"
        );
    }

    #[test]
    fn latest_run_default_resolves() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_run_report(&temp.path().to_path_buf(), &sample_report("run-1")).expect("first");
        write_run_report(&temp.path().to_path_buf(), &sample_report("run-2")).expect("second");

        let report = load_report(temp.path(), &ReportTarget::LatestRun).expect("load latest");
        assert_eq!(report.run_id, "run-2");
    }

    #[test]
    fn explicit_target_loads_saved_report() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_run_report(&temp.path().to_path_buf(), &sample_report("run-7")).expect("write");

        let report =
            load_report(temp.path(), &ReportTarget::Explicit("run-7".into())).expect("load");
        assert_eq!(report.run_id, "run-7");
    }

    #[test]
    fn render_report_mentions_review_outcomes() {
        let output = render_report(&sample_report("run-9"));
        assert!(output.contains("Run Report"));
        assert!(output.contains("foundation"));
        assert!(output.contains("Review attempts: 1"));
    }

    fn sample_report(run_id: &str) -> RunReport {
        RunReport {
            run_id: run_id.into(),
            generated_at: chrono::Utc::now(),
            plan: ath_types::plan::ExecutionPlan {
                phases: vec![],
                execution_order: vec![],
                parallel_groups: vec![],
                critical_path_length: 0,
            },
            phase_records: vec![PhaseRecord {
                id: uuid::Uuid::new_v4(),
                phase_id: 1,
                phase_name: "foundation".into(),
                started_at: chrono::Utc::now(),
                completed_at: Some(chrono::Utc::now()),
                contributions: vec![],
                review_attempts: vec![],
            }],
            totals: ath_types::report::RunTotals::default(),
        }
    }
}
