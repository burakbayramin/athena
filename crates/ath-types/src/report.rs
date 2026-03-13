//! Run-report types for persisted Athena execution artifacts.
//!
//! `RunReport` combines the routed execution plan with the completed phase
//! records so report rendering does not need to re-run planning or execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agent::AgentKind;
use crate::phase::PhaseRecord;
use crate::plan::ExecutionPlan;

/// Aggregated usage totals for a saved run report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportTotals {
    /// Total input tokens across the full run.
    pub input_tokens: u64,
    /// Total output tokens across the full run.
    pub output_tokens: u64,
    /// Total estimated cost in USD.
    pub estimated_cost_usd: Option<f64>,
}

impl Default for ReportTotals {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: Some(0.0),
        }
    }
}

/// Usage totals for one agent inside a saved phase summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTotals {
    pub agent: AgentKind,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
    pub files_produced: Vec<String>,
}

/// Aggregated usage summary for one completed phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseSummary {
    pub phase_id: u32,
    pub phase_name: String,
    pub review_attempts: u32,
    pub totals: ReportTotals,
    pub agent_totals: Vec<AgentTotals>,
}

/// Persisted report artifact for a completed Athena run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunReport {
    /// Stable identifier for the saved run artifact.
    pub run_id: String,
    /// When the report artifact was written.
    pub generated_at: DateTime<Utc>,
    /// Routed execution plan used for the run.
    pub plan: ExecutionPlan,
    /// Completed phase records from execution.
    pub phase_records: Vec<PhaseRecord>,
    /// Aggregated phase summaries for report rendering.
    #[serde(default)]
    pub phase_summaries: Vec<PhaseSummary>,
    /// Aggregated totals across the run.
    #[serde(default)]
    pub totals: ReportTotals,
}

pub type RunTotals = ReportTotals;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    use crate::agent::AgentKind;
    use crate::phase::{AgentContribution, TokenUsage};

    #[test]
    fn run_report_round_trip() {
        let report = RunReport {
            run_id: "run-123".into(),
            generated_at: Utc::now(),
            plan: ExecutionPlan {
                phases: vec![],
                execution_order: vec![],
                parallel_groups: vec![],
                critical_path_length: 0,
            },
            phase_records: vec![PhaseRecord {
                id: Uuid::new_v4(),
                phase_id: 7,
                phase_name: "foundation".into(),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                contributions: vec![AgentContribution {
                    agent: AgentKind::Claude("opus-4".into()),
                    tokens: TokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                        estimated_cost_usd: 0.0,
                    },
                    files_produced: vec!["src/lib.rs".into()],
                }],
                review_attempts: vec![],
            }],
            phase_summaries: vec![PhaseSummary {
                phase_id: 7,
                phase_name: "foundation".into(),
                review_attempts: 0,
                totals: ReportTotals {
                    input_tokens: 10,
                    output_tokens: 5,
                    estimated_cost_usd: Some(0.42),
                },
                agent_totals: vec![AgentTotals {
                    agent: AgentKind::Claude("opus-4".into()),
                    input_tokens: 10,
                    output_tokens: 5,
                    estimated_cost_usd: Some(0.42),
                    files_produced: vec!["src/lib.rs".into()],
                }],
            }],
            totals: ReportTotals::default(),
        };

        let json = serde_json::to_string(&report).expect("serialize");
        let deserialized: RunReport = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.phase_records[0].phase_id, 7);
        assert_eq!(report, deserialized);
    }

    #[test]
    fn run_totals_default_to_zero() {
        assert_eq!(
            ReportTotals::default(),
            ReportTotals {
                input_tokens: 0,
                output_tokens: 0,
                estimated_cost_usd: Some(0.0),
            }
        );
    }
}
