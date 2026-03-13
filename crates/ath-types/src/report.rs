#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use crate::agent::AgentKind;
    use crate::phase::{AgentContribution, PhaseRecord, TokenUsage};
    use crate::plan::ExecutionPlan;

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
            totals: RunTotals::default(),
        };

        let json = serde_json::to_string(&report).expect("serialize");
        let deserialized: RunReport = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.phase_records[0].phase_id, 7);
        assert_eq!(report, deserialized);
    }
}
