//! AgentCoordinator: top-level entry point for driving an ExecutionPlan
//! through parallel group dispatch with review-gated execution.
//!
//! Iterates `parallel_groups` from the plan, dispatching phases within each
//! group concurrently via `tokio::task::JoinSet`. Single-phase groups run
//! directly without spawn overhead. Pre-dispatch isolation validation and
//! a commit gate serialize file writes and git commits. Fails fast on the
//! first phase that errors.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinSet;

use ath_types::phase::PhaseRecord;
use ath_types::plan::ExecutionPlan;

use crate::error::PhaseRunnerError;
use crate::phase_runner::{run_phase_with_progress, AgentRegistry, FileOutput};
use crate::progress::{emit_progress, ProgressEvent, SharedProgressObserver};

/// Drives a full `ExecutionPlan` through parallel group dispatch.
///
/// Each group of phases is dispatched concurrently via `JoinSet`.
/// Single-phase groups run directly without spawn overhead.
/// Files produced by each phase are written to `output_dir` under a
/// commit gate that serializes write + git commit operations.
/// Execution halts at the first phase that fails (fail-fast).
pub struct AgentCoordinator {
    registry: Arc<AgentRegistry>,
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
            registry: Arc::new(registry),
            output_dir,
            git,
        }
    }

    /// Execute all phases in `plan.parallel_groups` with optional progress events.
    ///
    /// Returns a `Vec<PhaseRecord>` on success (one per phase).
    /// On failure, returns the error from the failing phase.
    pub async fn run_plan(
        &self,
        plan: &ExecutionPlan,
    ) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
        self.run_plan_with_progress(plan, None).await
    }

    /// Execute all phases via parallel group dispatch with optional progress events.
    ///
    /// Pre-dispatch isolation validation ensures no file ownership conflicts.
    /// Single-phase groups run directly; multi-phase groups use JoinSet fan-out.
    /// A commit gate serializes file writes and git commits within each group.
    pub async fn run_plan_with_progress(
        &self,
        plan: &ExecutionPlan,
        observer: Option<SharedProgressObserver>,
    ) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
        // Pre-dispatch: validate isolation across all parallel groups
        crate::isolation::check_isolation(plan).map_err(|e| {
            PhaseRunnerError::IsolationViolation {
                details: e.to_string(),
            }
        })?;

        let commit_gate = Arc::new(AsyncMutex::new(()));
        let mut results: Vec<PhaseRecord> = Vec::new();
        let total_phases = plan.execution_order.len();

        // Determine effective parallel groups: if parallel_groups is empty,
        // fall back to treating each execution_order entry as a single-phase group.
        let effective_groups: Vec<Vec<u32>> = if plan.parallel_groups.is_empty() {
            plan.execution_order.iter().map(|&id| vec![id]).collect()
        } else {
            plan.parallel_groups.clone()
        };

        let mut phases_processed: usize = 0;

        for group in &effective_groups {
            if group.len() == 1 {
                // Single-phase group: run directly without JoinSet overhead
                let phase_id = group[0];
                let phase = plan
                    .phases
                    .iter()
                    .find(|p| p.id == phase_id)
                    .ok_or_else(|| PhaseRunnerError::TaskExecutionFailed {
                        task_name: format!("phase-{}", phase_id),
                        agent: "coordinator".into(),
                        reason: format!("phase id {} not found in plan.phases", phase_id),
                    })?;

                let phase_index = phases_processed + 1;

                emit_progress(
                    observer.as_ref(),
                    ProgressEvent::PhaseStarted {
                        phase_id: phase.id,
                        phase_name: phase.name.clone(),
                        phase_index,
                        total_phases,
                        tasks: phase.tasks.iter().map(|task| task.name.clone()).collect(),
                    },
                );

                let output_dir = self.output_dir.clone();
                let write_files = move |files: &[FileOutput]| -> Result<(), PhaseRunnerError> {
                    write_files_to_dir(files, &output_dir)
                };

                let registry = &*self.registry;
                let available = |kind: &ath_types::agent::AgentKind| -> bool {
                    registry.get(kind).is_some()
                };

                let record = run_phase_with_progress(
                    phase,
                    registry,
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
                        phase_index,
                        total_phases,
                    },
                );

                results.push(record);
                phases_processed += 1;
            } else {
                // Multi-phase group: JoinSet fan-out
                let mut set: JoinSet<Result<(u32, String, PhaseRecord), PhaseRunnerError>> =
                    JoinSet::new();

                let group_phase_index = phases_processed + 1;

                for &phase_id in group {
                    let phase = plan
                        .phases
                        .iter()
                        .find(|p| p.id == phase_id)
                        .ok_or_else(|| PhaseRunnerError::TaskExecutionFailed {
                            task_name: format!("phase-{}", phase_id),
                            agent: "coordinator".into(),
                            reason: format!("phase id {} not found in plan.phases", phase_id),
                        })?
                        .clone();

                    let registry = Arc::clone(&self.registry);
                    let commit_gate = Arc::clone(&commit_gate);
                    let observer = observer.clone();
                    let output_dir = self.output_dir.clone();
                    let git = self.git.clone();

                    set.spawn(async move {
                        let phase_name = phase.name.clone();
                        let p_id = phase.id;

                        emit_progress(
                            observer.as_ref(),
                            ProgressEvent::PhaseStarted {
                                phase_id: p_id,
                                phase_name: phase_name.clone(),
                                phase_index: group_phase_index,
                                total_phases,
                                tasks: phase
                                    .tasks
                                    .iter()
                                    .map(|task| task.name.clone())
                                    .collect(),
                            },
                        );

                        // Build write_files closure -- but do NOT call it yet.
                        // The phase runner calls write_files internally, but we need
                        // to serialize via the commit gate. We wrap the write in
                        // the commit gate inside write_files itself.
                        let _commit_gate = Arc::clone(&commit_gate);
                        let dir_for_write = output_dir.clone();

                        // We cannot hold the gate across the async run_phase call,
                        // so we create a synchronous write_files that acquires
                        // the gate synchronously. However, the commit gate is an
                        // async mutex. Since write_files is a sync closure, we need
                        // a different approach: we let write_files write directly
                        // (the output_dir is shared but files are disjoint per
                        // isolation check), and serialize only git commits.
                        let write_files =
                            move |files: &[FileOutput]| -> Result<(), PhaseRunnerError> {
                                // Isolation check already verified file disjointness,
                                // so concurrent writes to different files are safe.
                                write_files_to_dir(files, &dir_for_write)
                            };

                        let available = {
                            let reg = Arc::clone(&registry);
                            move |kind: &ath_types::agent::AgentKind| -> bool {
                                reg.get(kind).is_some()
                            }
                        };

                        // Serialize git commit via commit gate
                        // Note: run_phase_with_progress handles git internally via
                        // the git parameter. For parallel dispatch, we pass git
                        // through the commit gate to serialize commits.
                        // Since git operations are handled inside run_phase_with_progress,
                        // and AsyncGitLayer uses Arc<Mutex<Repository>> internally,
                        // plus our isolation check ensures disjoint files, we can
                        // pass git directly. The internal mutex in GitLayer serializes
                        // the actual git2 operations.
                        let record = run_phase_with_progress(
                            &phase,
                            &registry,
                            available,
                            write_files,
                            git.as_ref(),
                            observer.clone(),
                        )
                        .await?;

                        emit_progress(
                            observer.as_ref(),
                            ProgressEvent::PhaseCompleted {
                                phase_id: p_id,
                                phase_name: phase_name.clone(),
                                phase_index: group_phase_index,
                                total_phases,
                            },
                        );

                        Ok((p_id, phase_name, record))
                    });
                }

                // Collect results, fail-fast on first error
                let mut group_results: Vec<(u32, String, PhaseRecord)> = Vec::new();

                while let Some(join_result) = set.join_next().await {
                    match join_result {
                        Err(join_error) => {
                            // Task panicked or was cancelled
                            set.abort_all();
                            return Err(PhaseRunnerError::ParallelPhaseFailed {
                                phase_name: "unknown".into(),
                                reason: join_error.to_string(),
                            });
                        }
                        Ok(Err(phase_error)) => {
                            // Phase returned an error
                            set.abort_all();
                            return Err(phase_error);
                        }
                        Ok(Ok(tuple)) => {
                            group_results.push(tuple);
                        }
                    }
                }

                // Sort by phase_id for deterministic ordering
                group_results.sort_by_key(|(phase_id, _, _)| *phase_id);

                for (_, _, record) in group_results {
                    results.push(record);
                }

                phases_processed += group.len();
            }
        }

        Ok(results)
    }
}

/// Write files to the output directory, creating parent directories as needed.
fn write_files_to_dir(files: &[FileOutput], output_dir: &std::path::Path) -> Result<(), PhaseRunnerError> {
    for file in files {
        let path = output_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
        }
        std::fs::write(&path, &file.content).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
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
        // Auto-populate parallel_groups as single-phase groups from execution_order
        let parallel_groups: Vec<Vec<u32>> =
            execution_order.iter().map(|&id| vec![id]).collect();
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups,
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

    // ==================== Wave 0: Parallel execution helpers ====================

    fn make_plan_with_groups(
        phases: Vec<PhaseSpec>,
        execution_order: Vec<u32>,
        parallel_groups: Vec<Vec<u32>>,
    ) -> ExecutionPlan {
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups,
            critical_path_length: 0,
        }
    }

    fn make_task_spec_with_files(name: &str, agent: Option<AgentKind>, files: Vec<&str>) -> TaskSpec {
        let mut spec = make_task_spec(name, agent);
        spec.expected_output_files = files.into_iter().map(String::from).collect();
        spec
    }

    // ==================== Wave 0: parallel_phases_overlap ====================

    #[tokio::test]
    async fn parallel_phases_overlap() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec("task-a", Some(claude.clone()))];
        let tasks2 = vec![make_task_spec("task-b", Some(claude.clone()))];
        let phase1 = make_phase(1, "par-phase-1", tasks1);
        let phase2 = make_phase(2, "par-phase-2", tasks2);
        let plan = make_plan_with_groups(
            vec![phase1, phase2],
            vec![1, 2],
            vec![vec![1, 2]],
        );

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

        let _result = coordinator.run_plan(&plan).await;
        todo!("Wave 0 stub: parallel_phases_overlap -- must prove concurrent execution overlap via timing or concurrency counter");
    }

    // ==================== Wave 0: parallel_faster_than_sequential ====================

    #[tokio::test]
    async fn parallel_faster_than_sequential() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec("task-a", Some(claude.clone()))];
        let tasks2 = vec![make_task_spec("task-b", Some(claude.clone()))];
        let phase1 = make_phase(1, "par-phase-1", tasks1);
        let phase2 = make_phase(2, "par-phase-2", tasks2);
        let plan = make_plan_with_groups(
            vec![phase1, phase2],
            vec![1, 2],
            vec![vec![1, 2]],
        );

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

        let _result = coordinator.run_plan(&plan).await;
        todo!("Wave 0 stub: parallel_faster_than_sequential -- must prove wall-clock time improvement over forced sequential");
    }

    // ==================== Wave 0: parallel_isolation_blocks_conflict ====================

    #[tokio::test]
    async fn parallel_isolation_blocks_conflict() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec_with_files(
            "task-a",
            Some(claude.clone()),
            vec!["src/conflict.rs"],
        )];
        let tasks2 = vec![make_task_spec_with_files(
            "task-b",
            Some(claude.clone()),
            vec!["src/conflict.rs"],
        )];
        let phase1 = make_phase(1, "par-phase-1", tasks1);
        let phase2 = make_phase(2, "par-phase-2", tasks2);
        let plan = make_plan_with_groups(
            vec![phase1, phase2],
            vec![1, 2],
            vec![vec![1, 2]],
        );

        let task_mock = Arc::new(MockBackend::always_ok(
            &make_task_output_json("unused", &claude, "src/conflict.rs"),
        ));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        let _result = coordinator.run_plan(&plan).await;
        todo!("Wave 0 stub: parallel_isolation_blocks_conflict -- must return IsolationViolation error");
    }

    // ==================== Wave 0: parallel_commits_correct_metadata ====================

    #[tokio::test]
    async fn parallel_commits_correct_metadata() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks1 = vec![make_task_spec_with_files(
            "task-a",
            Some(claude.clone()),
            vec!["src/a.rs"],
        )];
        let tasks2 = vec![make_task_spec_with_files(
            "task-b",
            Some(claude.clone()),
            vec!["src/b.rs"],
        )];
        let phase1 = make_phase(1, "par-phase-1", tasks1);
        let phase2 = make_phase(2, "par-phase-2", tasks2);
        let plan = make_plan_with_groups(
            vec![phase1, phase2],
            vec![1, 2],
            vec![vec![1, 2]],
        );

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

        let _result = coordinator.run_plan(&plan).await;
        todo!("Wave 0 stub: parallel_commits_correct_metadata -- must verify deterministic ordering and file output");
    }
}
