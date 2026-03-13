use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use ath_types::phase::PhaseRecord;
use ath_types::plan::ExecutionPlan;
use ath_types::report::{AgentTotals, PhaseSummary, ReportTotals, RunReport, RunTotals};
use clap::Args;

use crate::GlobalArgs;

const ATH_DIR: &str = ".ath";
const RUNS_DIR: &str = "runs";
const REPORT_FILE: &str = "report.json";
const LATEST_FILE: &str = "latest.txt";

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

pub(crate) fn build_run_report(plan: &ExecutionPlan, phase_records: &[PhaseRecord]) -> RunReport {
    let phase_summaries = build_phase_summaries(phase_records);
    RunReport {
        run_id: next_run_id(),
        generated_at: chrono::Utc::now(),
        plan: plan.clone(),
        phase_records: phase_records.to_vec(),
        totals: aggregate_report_totals(&phase_summaries),
        phase_summaries,
    }
}

pub(crate) fn write_run_report(project_dir: &Path, report: &RunReport) -> Result<PathBuf> {
    let path = report_path(project_dir, &report.run_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| anyhow!("{e}"))?;
    }

    let json = serde_json::to_string_pretty(report).map_err(|e| anyhow!("{e}"))?;
    std::fs::write(&path, json).map_err(|e| anyhow!("{e}"))?;
    std::fs::write(latest_run_pointer_path(project_dir), &report.run_id)
        .map_err(|e| anyhow!("{e}"))?;
    Ok(path)
}

pub(crate) fn load_report(project_dir: &Path, target: &ReportTarget) -> Result<RunReport> {
    let path = resolve_report_path(project_dir, target)?;
    let raw = std::fs::read_to_string(&path).map_err(|e| anyhow!("{e}"))?;
    serde_json::from_str(&raw).map_err(|e| anyhow!("{e}"))
}

pub(crate) fn render_report(report: &RunReport) -> String {
    let phase_summaries = phase_summaries(report);
    let totals = if report.phase_summaries.is_empty() {
        aggregate_report_totals(&phase_summaries)
    } else {
        report.totals.clone()
    };
    let mut out = String::new();
    out.push_str("Run Report\n");
    out.push_str(&format!("Run ID: {}\n", report.run_id));
    out.push_str(&format!(
        "Generated: {}\n",
        report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));
    out.push_str(&format!(
        "Phases: {} | Planned waves: {}\n",
        phase_summaries.len(),
        report.plan.parallel_groups.len()
    ));
    out.push('\n');

    for phase in &phase_summaries {
        out.push_str(&format!("Phase {}: {}\n", phase.phase_id, phase.phase_name));
        out.push_str(&format!("Review attempts: {}\n", phase.review_attempts));
        out.push_str(&format!("Input tokens: {}\n", phase.totals.input_tokens));
        out.push_str(&format!("Output tokens: {}\n", phase.totals.output_tokens));
        out.push_str(&format!(
            "Estimated cost: {}\n",
            format_cost(phase.totals.estimated_cost_usd)
        ));

        for agent in &phase.agent_totals {
            out.push_str(&format!(
                "  - {}/{}: {} input, {} output, {}\n",
                agent.agent.provider_name(),
                agent.agent.model(),
                agent.input_tokens,
                agent.output_tokens,
                format_cost(agent.estimated_cost_usd)
            ));
        }
        out.push('\n');
    }

    out.push_str("Run totals\n");
    out.push_str(&format!("Total input tokens: {}\n", totals.input_tokens));
    out.push_str(&format!("Total output tokens: {}\n", totals.output_tokens));
    out.push_str(&format!(
        "Total estimated cost: {}\n",
        format_cost(totals.estimated_cost_usd)
    ));

    out
}

fn phase_summaries(report: &RunReport) -> Vec<PhaseSummary> {
    if report.phase_summaries.is_empty() {
        build_phase_summaries(&report.phase_records)
    } else {
        report.phase_summaries.clone()
    }
}

fn build_phase_summaries(phase_records: &[PhaseRecord]) -> Vec<PhaseSummary> {
    phase_records.iter().map(build_phase_summary).collect()
}

fn build_phase_summary(record: &PhaseRecord) -> PhaseSummary {
    let agent_totals: Vec<AgentTotals> = record
        .contributions
        .iter()
        .map(|contribution| AgentTotals {
            agent: contribution.agent.clone(),
            input_tokens: contribution.tokens.input_tokens,
            output_tokens: contribution.tokens.output_tokens,
            estimated_cost_usd: crate::cost::estimate_cost(
                &contribution.agent,
                contribution.tokens.input_tokens,
                contribution.tokens.output_tokens,
            )
            .ok(),
            files_produced: contribution.files_produced.clone(),
        })
        .collect();

    PhaseSummary {
        phase_id: record.phase_id,
        phase_name: record.phase_name.clone(),
        review_attempts: record.review_attempts.len() as u32,
        totals: aggregate_agent_totals(&agent_totals),
        agent_totals,
    }
}

fn aggregate_agent_totals(agent_totals: &[AgentTotals]) -> ReportTotals {
    ReportTotals {
        input_tokens: agent_totals.iter().map(|agent| agent.input_tokens).sum(),
        output_tokens: agent_totals.iter().map(|agent| agent.output_tokens).sum(),
        estimated_cost_usd: sum_costs(agent_totals.iter().map(|agent| agent.estimated_cost_usd)),
    }
}

fn aggregate_report_totals(phase_summaries: &[PhaseSummary]) -> RunTotals {
    RunTotals {
        input_tokens: phase_summaries
            .iter()
            .map(|phase| phase.totals.input_tokens)
            .sum(),
        output_tokens: phase_summaries
            .iter()
            .map(|phase| phase.totals.output_tokens)
            .sum(),
        estimated_cost_usd: sum_costs(
            phase_summaries
                .iter()
                .map(|phase| phase.totals.estimated_cost_usd),
        ),
    }
}

fn sum_costs(costs: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    let mut total = 0.0;
    for cost in costs {
        total += cost?;
    }
    Some(total)
}

fn format_cost(cost: Option<f64>) -> String {
    match cost {
        Some(amount) => format!("${amount:.4}"),
        None => "n/a".into(),
    }
}

fn next_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("run-{nanos}")
}

fn resolve_report_path(project_dir: &Path, target: &ReportTarget) -> Result<PathBuf> {
    match target {
        ReportTarget::LatestRun => {
            let latest =
                std::fs::read_to_string(latest_run_pointer_path(project_dir)).map_err(|_| {
                    anyhow!(
                    "`ath report` requires a previously saved run report. Run `ath run` once first."
                )
                })?;
            let run_id = latest.trim();
            if run_id.is_empty() {
                anyhow::bail!(
                    "`ath report` requires a previously saved run report. Run `ath run` once first."
                );
            }
            Ok(report_path(project_dir, run_id))
        }
        ReportTarget::Explicit(target) => {
            let explicit = PathBuf::from(target);
            if explicit.is_absolute() {
                return Ok(normalize_explicit_target(explicit));
            }

            let project_relative = project_dir.join(&explicit);
            if project_relative.exists() {
                return Ok(normalize_explicit_target(project_relative));
            }

            Ok(report_path(project_dir, target))
        }
    }
}

fn normalize_explicit_target(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path.join(REPORT_FILE)
    } else {
        path
    }
}

fn report_path(project_dir: &Path, run_id: &str) -> PathBuf {
    runs_dir(project_dir).join(run_id).join(REPORT_FILE)
}

fn runs_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(ATH_DIR).join(RUNS_DIR)
}

fn latest_run_pointer_path(project_dir: &Path) -> PathBuf {
    runs_dir(project_dir).join(LATEST_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::phase::PhaseRecord;
    use ath_types::report::{ReportTotals, RunReport};
    use ath_types::review::{ReviewVerdict, Severity};

    #[test]
    fn writes_run_report_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = sample_report("run-1");

        let path = write_run_report(temp.path(), &report).expect("write report");

        assert!(path.exists(), "report.json should exist");
        assert!(
            temp.path().join(".ath/runs/latest.txt").exists(),
            "latest-run pointer should exist"
        );
    }

    #[test]
    fn latest_run_default_resolves() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_run_report(temp.path(), &sample_report("run-1")).expect("first");
        write_run_report(temp.path(), &sample_report("run-2")).expect("second");

        let report = load_report(temp.path(), &ReportTarget::LatestRun).expect("load latest");
        assert_eq!(report.run_id, "run-2");
    }

    #[test]
    fn explicit_target_loads_saved_report() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_run_report(temp.path(), &sample_report("run-7")).expect("write");

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

    #[test]
    fn computes_per_phase_and_run_costs() {
        let report = build_run_report(
            &sample_report("run-9").plan,
            &sample_report("run-9").phase_records,
        );

        assert_eq!(report.phase_summaries.len(), 1);
        assert_eq!(report.phase_summaries[0].totals.input_tokens, 125);
        assert_eq!(report.phase_summaries[0].totals.output_tokens, 50);
        assert!(report.phase_summaries[0]
            .totals
            .estimated_cost_usd
            .is_some());
        assert!(
            (report.phase_summaries[0].agent_totals[0]
                .estimated_cost_usd
                .expect("known author pricing")
                - 0.003)
                .abs()
                < f64::EPSILON
        );
        assert!(report.totals.estimated_cost_usd.is_some());
    }

    #[test]
    fn render_report_surfaces_phase_and_run_usage_totals() {
        let built = build_run_report(
            &sample_report("run-9").plan,
            &sample_report("run-9").phase_records,
        );
        let output = render_report(&built);

        assert!(output.contains("Phase 1: foundation"));
        assert!(output.contains("Input tokens: 125"));
        assert!(output.contains("Output tokens: 50"));
        assert!(output.contains("Estimated cost:"));
        assert!(output.contains("Anthropic/opus-4"));
        assert!(output.contains("Total estimated cost:"));
    }

    #[test]
    fn render_report_marks_unknown_pricing_as_not_available() {
        let mut report = sample_report("run-unknown");
        report.phase_records[0].contributions[0].agent =
            ath_types::agent::AgentKind::Codex("unknown-model".into());

        let built = build_run_report(&report.plan, &report.phase_records);
        let output = render_report(&built);

        assert!(output.contains("n/a"));
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
                contributions: vec![
                    ath_types::phase::AgentContribution {
                        agent: ath_types::agent::AgentKind::Claude("opus-4".into()),
                        tokens: ath_types::phase::TokenUsage {
                            input_tokens: 100,
                            output_tokens: 20,
                            estimated_cost_usd: 0.0,
                        },
                        files_produced: vec!["src/lib.rs".into()],
                    },
                    ath_types::phase::AgentContribution {
                        agent: ath_types::agent::AgentKind::Gemini("2.5-pro".into()),
                        tokens: ath_types::phase::TokenUsage {
                            input_tokens: 25,
                            output_tokens: 30,
                            estimated_cost_usd: 0.0,
                        },
                        files_produced: vec![],
                    },
                ],
                review_attempts: vec![ath_types::phase::ReviewAttempt {
                    attempt_number: 1,
                    verdict: ReviewVerdict {
                        passed: true,
                        reviewer: ath_types::agent::AgentKind::Gemini("2.5-pro".into()),
                        severity: Severity::Info,
                        reason: "Looks good".into(),
                        suggestions: vec![],
                    },
                    tokens: ath_types::phase::TokenUsage {
                        input_tokens: 25,
                        output_tokens: 30,
                        estimated_cost_usd: 0.0,
                    },
                    timestamp: chrono::Utc::now(),
                }],
            }],
            phase_summaries: vec![],
            totals: ReportTotals::default(),
        }
    }
}
