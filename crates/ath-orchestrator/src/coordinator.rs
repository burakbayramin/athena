//! AgentCoordinator: top-level entry point for driving an ExecutionPlan
//! through sequential phase dispatch with review-gated execution.
//!
//! Iterates `execution_order` from the plan, dispatching each phase through
//! `run_phase` (which handles task execution, cross-agent review, and retry).
//! Files are written to `output_dir` on successful review. Fails fast on
//! the first phase that errors.

use std::path::PathBuf;

use ath_types::plan::ExecutionPlan;
use ath_types::phase::PhaseRecord;

use crate::error::PhaseRunnerError;
use crate::phase_runner::{run_phase_with_progress, AgentRegistry, FileOutput};
use crate::progress::{emit_progress, ProgressEvent, SharedProgressObserver};

/// Drives a full `ExecutionPlan` through sequential phase dispatch.
///
/// Each phase is run through the review-gated pipeline via `run_phase`.
/// Files produced by each phase are written to `output_dir`.
/// Execution halts at the first phase that fails (fail-fast).
pub struct AgentCoordinator {
    registry: AgentRegistry,
    output_dir: PathBuf,
    git: Option<ath_git::async_ops::AsyncGitLayer>,
}

impl AgentCoordinator {
    /// Create a new coordinator.
    pub fn new(
        registry: AgentRegistry,
        output_dir: PathBuf,
        git: Option<ath_git::async_ops::AsyncGitLayer>,
    ) -> Self {
        Self {
            registry,
            output_dir,
            git,
        }
    }

    /// Execute all phases in `plan.execution_order` sequentially.
    ///
    /// Returns a `Vec<PhaseRecord>` on success (one per phase).
    /// On failure, returns the error from the failing phase.
    pub async fn run_plan(
        &self,
        plan: &ExecutionPlan,
    ) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
        self.run_plan_with_progress(plan, None).await
    }

    /// Execute all phases in `plan.execution_order` sequentially with optional progress events.
    pub async fn run_plan_with_progress(
        &self,
        plan: &ExecutionPlan,
        observer: Option<SharedProgressObserver>,
    ) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
        let mut results: Vec<PhaseRecord> = Vec::new();
        let total_phases = plan.execution_order.len();

        for (phase_index, &phase_id) in plan.execution_order.iter().enumerate() {
            let phase = plan
                .phases
                .iter()
                .find(|p| p.id == phase_id)
                .ok_or_else(|| PhaseRunnerError::TaskExecutionFailed {
                    task_name: format!("phase-{}", phase_id),
                    agent: "coordinator".into(),
                    reason: format!("phase id {} not found in plan.phases", phase_id),
                })?;

            emit_progress(
                observer.as_ref(),
                ProgressEvent::PhaseStarted {
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    phase_index: phase_index + 1,
                    total_phases,
                    tasks: phase.tasks.iter().map(|task| task.name.clone()).collect(),
                },
            );

            let output_dir = self.output_dir.clone();
            let write_files = move |files: &[FileOutput]| -> Result<(), PhaseRunnerError> {
                for file in files {
                    let path = output_dir.join(&file.path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            PhaseRunnerError::AtomicWriteFailed {
                                path: path.display().to_string(),
                                reason: e.to_string(),
                            }
                        })?;
                    }
                    std::fs::write(&path, &file.content).map_err(|e| {
                        PhaseRunnerError::AtomicWriteFailed {
                            path: path.display().to_string(),
                            reason: e.to_string(),
                        }
                    })?;
                }
                Ok(())
            };

            let available = |kind: &ath_types::agent::AgentKind| -> bool {
                self.registry.get(kind).is_some()
            };

            let record = run_phase_with_progress(
                phase,
                &self.registry,
                available,
                write_files,
                self.git.as_ref(),
                observer.clone(),
            )
            .await?;

            emit_progress(
                observer.as_ref(),
                ProgressEvent::PhaseCompleted {
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    phase_index: phase_index + 1,
                    total_phases,
                },
            );

            results.push(record);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_agents::MockBackend;
    use ath_types::agent::{AgentKind, AgentResponse};
    use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;
    use std::sync::Arc;

    // ==================== Helpers ====================

    fn make_task_spec(name: &str, agent: Option<AgentKind>) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("Implement {name}"),
            skill_tags: vec![SkillTag("rust".into())],
            expected_output_files: vec![],
            acceptance_criteria: vec![],
            goal_indices: vec![],
            assigned_agent: agent,
        }
    }

    fn make_task_output_json(task_name: &str, agent: &AgentKind, file_path: &str) -> String {
        let agent_json = serde_json::to_string(agent).unwrap();
        format!(
            r#"{{"task_name":"{}","agent":{},"files_produced":[{{"path":"{}","content":"fn main() {{}}"}}],"explanation":"Done","issues_encountered":[],"input_tokens":100,"output_tokens":50}}"#,
            task_name, agent_json, file_path
        )
    }

    fn mock_response(content: &str, agent: &AgentKind) -> AgentResponse {
        AgentResponse {
            request_id: uuid::Uuid::new_v4(),
            agent: agent.clone(),
            content: content.to_string(),
            input_tokens: 100,
            output_tokens: 50,
            created_at: chrono::Utc::now(),
        }
    }

    fn passing_verdict_json() -> String {
        r#"{"passed":true,"severity":"info","reason":"All good","suggestions":[]}"#.into()
    }

    fn failing_verdict_json(reason: &str) -> String {
        format!(
            r#"{{"passed":false,"severity":"critical","reason":"{}","suggestions":[]}}"#,
            reason
        )
    }

    fn make_phase(id: u32, name: &str, tasks: Vec<TaskSpec>) -> PhaseSpec {
        PhaseSpec {
            id,
            name: name.into(),
            description: format!("{name} description"),
            tasks,
            depends_on: vec![],
            produces: vec![],
            consumes: vec![],
        }
    }

    fn make_plan(phases: Vec<PhaseSpec>, execution_order: Vec<u32>) -> ExecutionPlan {
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups: vec![],
            critical_path_length: 0,
        }
    }

    // ==================== Test 1: Single phase, review passes ====================

    #[tokio::test]
    async fn run_plan_single_phase_returns_one_record() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
            &make_task_output_json("task-1", &claude, "src/main.rs"),
            &claude,
        ))]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let result = coordinator.run_plan(&plan).await;
        assert!(result.is_ok());
        let records = result.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phase_name, "phase-1");
        assert!(records[0].completed_at.is_some());
    }

    // ==================== Test 2: Two sequential phases ====================

    #[tokio::test]
    async fn run_plan_two_phases_returns_two_records_in_order() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec("task-a", Some(claude.clone()))];
        let tasks2 = vec![make_task_spec("task-b", Some(claude.clone()))];
        let phase1 = make_phase(1, "phase-1", tasks1);
        let phase2 = make_phase(2, "phase-2", tasks2);
        let plan = make_plan(vec![phase1, phase2], vec![1, 2]);

        // Two task responses + always-pass reviewer
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-a", &claude, "src/a.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-b", &claude, "src/b.rs"),
                &claude,
            )),
        ]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let records = coordinator.run_plan(&plan).await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].phase_name, "phase-1");
        assert_eq!(records[1].phase_name, "phase-2");
    }

    // ==================== Test 3: Second phase fails after 3 retries ====================

    #[tokio::test]
    async fn run_plan_halts_on_failing_phase() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec("task-a", Some(claude.clone()))];
        let tasks2 = vec![make_task_spec("task-b", Some(claude.clone()))];
        let phase1 = make_phase(1, "phase-1", tasks1);
        let phase2 = make_phase(2, "phase-2", tasks2);
        let plan = make_plan(vec![phase1, phase2], vec![1, 2]);

        // Phase 1: 1 task exec + pass review
        // Phase 2: 3 task exec (retry loop) + 3 fail reviews
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-a", &claude, "src/a.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-b", &claude, "src/b.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-b", &claude, "src/b.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-b", &claude, "src/b.rs"),
                &claude,
            )),
        ]));
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            // Phase 1 review: pass
            Ok(mock_response(&passing_verdict_json(), &gemini)),
            // Phase 2 reviews: fail x3
            Ok(mock_response(&failing_verdict_json("fail-1"), &gemini)),
            Ok(mock_response(&failing_verdict_json("fail-2"), &gemini)),
            Ok(mock_response(&failing_verdict_json("fail-3"), &gemini)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let result = coordinator.run_plan(&plan).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PhaseRunnerError::MaxRetriesExceeded { .. }));
    }

    // ==================== Test 4: Respects execution_order ====================

    #[tokio::test]
    async fn run_plan_respects_execution_order() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        // Phases with ids 10, 20 but execution_order is [20, 10]
        let tasks1 = vec![make_task_spec("task-ten", Some(claude.clone()))];
        let tasks2 = vec![make_task_spec("task-twenty", Some(claude.clone()))];
        let phase10 = make_phase(10, "phase-ten", tasks1);
        let phase20 = make_phase(20, "phase-twenty", tasks2);
        let plan = make_plan(vec![phase10, phase20], vec![20, 10]);

        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-twenty", &claude, "src/twenty.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-ten", &claude, "src/ten.rs"),
                &claude,
            )),
        ]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let records = coordinator.run_plan(&plan).await.unwrap();
        assert_eq!(records.len(), 2);
        // Phase 20 should be first (execution_order = [20, 10])
        assert_eq!(records[0].phase_name, "phase-twenty");
        assert_eq!(records[1].phase_name, "phase-ten");
    }

    // ==================== Test 5: Files written to output_dir ====================

    #[tokio::test]
    async fn run_plan_writes_files_to_output_dir() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
            &make_task_output_json("task-1", &claude, "src/main.rs"),
            &claude,
        ))]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        coordinator.run_plan(&plan).await.unwrap();

        // Verify file was written
        let file_path = tmp.path().join("src/main.rs");
        assert!(file_path.exists(), "Expected file src/main.rs in output_dir");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "fn main() {}");
    }

    // ==================== Integration Tests ====================

    // Integration Test 1: Full pipeline with 2 phases, each with 2 tasks,
    // multi-agent (Claude + Gemini), cross-agent review
    #[tokio::test]
    async fn full_pipeline_passes() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());
        let codex = AgentKind::Codex("o3".into());

        // Phase 1: 2 Claude tasks, Phase 2: 2 Gemini tasks
        let phase1 = make_phase(
            1,
            "foundation",
            vec![
                make_task_spec("setup-project", Some(claude.clone())),
                make_task_spec("add-types", Some(claude.clone())),
            ],
        );
        let phase2 = make_phase(
            2,
            "implementation",
            vec![
                make_task_spec("build-api", Some(gemini.clone())),
                make_task_spec("add-tests", Some(gemini.clone())),
            ],
        );
        let plan = make_plan(vec![phase1, phase2], vec![1, 2]);

        // Phase 1: Claude tasks (2 calls) -> Gemini reviews (1 call)
        // Phase 2: Gemini tasks (2 calls) -> Claude reviews (1 call)
        // Claude mock = 2 task responses + 1 review response (reviewer for phase 2)
        let claude_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("setup-project", &claude, "src/lib.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("add-types", &claude, "src/types.rs"),
                &claude,
            )),
            // Claude reviews phase 2 (pass)
            Ok(mock_response(&passing_verdict_json(), &claude)),
        ]));

        // Gemini mock = 1 review for phase 1 + 2 task responses for phase 2
        let gemini_mock = Arc::new(MockBackend::new(vec![
            // Gemini reviews phase 1 (pass)
            Ok(mock_response(&passing_verdict_json(), &gemini)),
            // Gemini tasks for phase 2
            Ok(mock_response(
                &make_task_output_json("build-api", &gemini, "src/api.rs"),
                &gemini,
            )),
            Ok(mock_response(
                &make_task_output_json("add-tests", &gemini, "tests/api_test.rs"),
                &gemini,
            )),
        ]));

        // Register Codex too so it's "available" but won't be chosen (lower priority)
        let codex_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), claude_mock);
        registry.register(gemini.clone(), gemini_mock);
        registry.register(codex.clone(), codex_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let records = coordinator.run_plan(&plan).await.unwrap();

        // Assert: 2 PhaseRecords returned
        assert_eq!(records.len(), 2);

        // Both have completed_at
        assert!(records[0].completed_at.is_some());
        assert!(records[1].completed_at.is_some());

        // Each has 1 review attempt that passed
        assert_eq!(records[0].review_attempts.len(), 1);
        assert!(records[0].review_attempts[0].verdict.passed);
        assert_eq!(records[1].review_attempts.len(), 1);
        assert!(records[1].review_attempts[0].verdict.passed);

        // Files exist in output_dir
        assert!(tmp.path().join("src/lib.rs").exists());
        assert!(tmp.path().join("src/types.rs").exists());
        assert!(tmp.path().join("src/api.rs").exists());
        assert!(tmp.path().join("tests/api_test.rs").exists());
    }

    // Integration Test 2: Pipeline retry-then-pass
    #[tokio::test]
    async fn pipeline_retry_then_pass() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(
            1,
            "retry-phase",
            vec![make_task_spec("task-1", Some(claude.clone()))],
        );
        let plan = make_plan(vec![phase], vec![1]);

        // Claude: 2 task executions (attempt 1 + retry)
        let claude_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
        ]));

        // Gemini reviews: fail first, then pass
        let gemini_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(&failing_verdict_json("Missing error handling"), &gemini)),
            Ok(mock_response(&passing_verdict_json(), &gemini)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), claude_mock);
        registry.register(gemini.clone(), gemini_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let records = coordinator.run_plan(&plan).await.unwrap();
        assert_eq!(records.len(), 1);

        let record = &records[0];
        assert_eq!(record.review_attempts.len(), 2);
        assert!(!record.review_attempts[0].verdict.passed);
        assert!(record.review_attempts[1].verdict.passed);
    }

    // Integration Test 3: Pipeline halts on max retries
    #[tokio::test]
    async fn pipeline_halts_on_max_retries() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(
            1,
            "failing-phase",
            vec![make_task_spec("task-1", Some(claude.clone()))],
        );
        let plan = make_plan(vec![phase], vec![1]);

        // Claude: 3 task executions (all attempts)
        let claude_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
        ]));

        // Gemini reviews: fail all 3
        let gemini_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(&failing_verdict_json("Still broken 1"), &gemini)),
            Ok(mock_response(&failing_verdict_json("Still broken 2"), &gemini)),
            Ok(mock_response(&failing_verdict_json("Still broken 3"), &gemini)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), claude_mock);
        registry.register(gemini.clone(), gemini_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let result = coordinator.run_plan(&plan).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, PhaseRunnerError::MaxRetriesExceeded { .. }));

        // Verify error message contains phase name and attempt count
        let msg = err.to_string();
        assert!(msg.contains("failing-phase"), "error should contain phase name");
        assert!(msg.contains("3"), "error should contain attempt count");
    }
}
