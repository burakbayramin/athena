//! Progress events emitted during plan and phase execution.
//!
//! Keeps orchestration semantics separate from CLI presentation. The
//! orchestrator emits structured lifecycle events and the CLI decides how
//! to render them.

use std::sync::Arc;

use ath_types::agent::AgentKind;

/// Structured execution events suitable for terminal progress rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    PhaseStarted {
        phase_id: u32,
        phase_name: String,
        phase_index: usize,
        total_phases: usize,
        tasks: Vec<String>,
    },
    TaskStarted {
        phase_id: u32,
        phase_name: String,
        task_name: String,
        task_index: usize,
        total_tasks: usize,
        agent: AgentKind,
    },
    TaskCompleted {
        phase_id: u32,
        phase_name: String,
        task_name: String,
        task_index: usize,
        total_tasks: usize,
        agent: AgentKind,
    },
    ReviewStarted {
        phase_id: u32,
        phase_name: String,
        reviewer: AgentKind,
        attempt_number: u32,
    },
    ReviewPassed {
        phase_id: u32,
        phase_name: String,
        reviewer: AgentKind,
        attempt_number: u32,
    },
    ReviewFailed {
        phase_id: u32,
        phase_name: String,
        reviewer: AgentKind,
        attempt_number: u32,
        reason_summary: String,
    },
    RetryStarted {
        phase_id: u32,
        phase_name: String,
        reviewer: AgentKind,
        attempt_number: u32,
    },
    PhaseCompleted {
        phase_id: u32,
        phase_name: String,
        phase_index: usize,
        total_phases: usize,
    },
    Transcript(Transcript),
}

/// Transcript capture categories layered onto the same progress seam.
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptKind {
    Executor,
    Reviewer,
    RetryFeedback,
}

/// Prompt/response transcript payload for verbose mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    pub kind: TranscriptKind,
    pub phase_id: u32,
    pub phase_name: String,
    pub label: String,
    pub attempt_number: u32,
    pub agent: AgentKind,
    pub prompt: String,
    pub response: String,
    pub retry_feedback: Option<String>,
}

/// Shared observer contract for progress event sinks.
pub trait ProgressObserver: Send + Sync {
    fn on_event(&self, event: ProgressEvent);

    fn captures_transcripts(&self) -> bool {
        false
    }
}

/// Shared observer handle passed through the execution engine.
pub type SharedProgressObserver = Arc<dyn ProgressObserver>;

/// Emit an event if an observer is attached.
pub fn emit_progress(observer: Option<&SharedProgressObserver>, event: ProgressEvent) {
    if let Some(observer) = observer {
        observer.on_event(event);
    }
}

/// Returns true when the attached observer requested transcript payloads.
pub fn wants_transcripts(observer: Option<&SharedProgressObserver>) -> bool {
    observer.is_some_and(|observer| observer.captures_transcripts())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PhaseRunnerError;
    use crate::coordinator::AgentCoordinator;
    use crate::phase_runner::{run_phase_with_progress, AgentRegistry, FileOutput};
    use ath_agents::MockBackend;
    use ath_types::agent::AgentResponse;
    use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<ProgressEvent>>,
    }

    impl RecordingObserver {
        fn events(&self) -> Vec<ProgressEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl ProgressObserver for RecordingObserver {
        fn on_event(&self, event: ProgressEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

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
        let parallel_groups: Vec<Vec<u32>> =
            execution_order.iter().map(|&id| vec![id]).collect();
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups,
            critical_path_length: 0,
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
            created_at: Utc::now(),
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

    #[tokio::test]
    async fn phase_execution_emits_phase_started_before_any_task_event() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
            &make_task_output_json("task-1", &claude, "src/main.rs"),
            &claude,
        ))]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let observer = Arc::new(RecordingObserver::default());
        let progress: SharedProgressObserver = observer.clone();

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        coordinator
            .run_plan_with_progress(&plan, Some(progress))
            .await
            .expect("plan executes");

        let events = observer.events();
        assert!(matches!(events.first(), Some(ProgressEvent::PhaseStarted { .. })));

        let first_task_event = events
            .iter()
            .position(|event| matches!(event, ProgressEvent::TaskStarted { .. }))
            .expect("task event");
        assert_eq!(first_task_event, 1);
    }

    #[tokio::test]
    async fn each_task_emits_started_then_completed_with_assigned_agent() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(
            1,
            "phase-1",
            vec![
                make_task_spec("task-a", Some(claude.clone())),
                make_task_spec("task-b", Some(claude.clone())),
            ],
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

        let observer = Arc::new(RecordingObserver::default());
        let progress: SharedProgressObserver = observer.clone();
        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
            .await
            .expect("phase executes");

        let events = observer.events();
        let task_events: Vec<_> = events
            .into_iter()
            .filter(|event| {
                matches!(
                    event,
                    ProgressEvent::TaskStarted { .. } | ProgressEvent::TaskCompleted { .. }
                )
            })
            .collect();

        assert!(matches!(
            task_events[0],
            ProgressEvent::TaskStarted {
                ref task_name,
                ref agent,
                ..
            } if task_name == "task-a" && agent == &claude
        ));
        assert!(matches!(
            task_events[1],
            ProgressEvent::TaskCompleted {
                ref task_name,
                ref agent,
                ..
            } if task_name == "task-a" && agent == &claude
        ));
        assert!(matches!(
            task_events[2],
            ProgressEvent::TaskStarted {
                ref task_name,
                ref agent,
                ..
            } if task_name == "task-b" && agent == &claude
        ));
        assert!(matches!(
            task_events[3],
            ProgressEvent::TaskCompleted {
                ref task_name,
                ref agent,
                ..
            } if task_name == "task-b" && agent == &claude
        ));
    }

    #[tokio::test]
    async fn review_dispatch_emits_review_started_before_reviewer_result() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

        let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
            &make_task_output_json("task-1", &claude, "src/main.rs"),
            &claude,
        ))]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let observer = Arc::new(RecordingObserver::default());
        let progress: SharedProgressObserver = observer.clone();
        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
            .await
            .expect("phase executes");

        let events = observer.events();
        let review_started = events
            .iter()
            .position(|event| matches!(event, ProgressEvent::ReviewStarted { .. }))
            .expect("review started event");
        let review_passed = events
            .iter()
            .position(|event| matches!(event, ProgressEvent::ReviewPassed { .. }))
            .expect("review passed event");

        assert!(review_started < review_passed);
    }

    #[tokio::test]
    async fn review_failure_emits_retry_and_review_failed_events() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
            Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            )),
        ]));
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response(&failing_verdict_json("needs work"), &gemini)),
            Ok(mock_response(&passing_verdict_json(), &gemini)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let observer = Arc::new(RecordingObserver::default());
        let progress: SharedProgressObserver = observer.clone();
        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
            .await
            .expect("phase executes");

        let events = observer.events();
        let failed = events
            .iter()
            .position(|event| matches!(event, ProgressEvent::ReviewFailed { .. }))
            .expect("review failed event");
        let retry = events
            .iter()
            .position(|event| matches!(event, ProgressEvent::RetryStarted { .. }))
            .expect("retry started event");

        assert!(failed < retry);
        assert!(matches!(
            events[retry],
            ProgressEvent::RetryStarted {
                attempt_number: 2,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn successful_completion_emits_phase_completed() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let phase = make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
            &make_task_output_json("task-1", &claude, "src/main.rs"),
            &claude,
        ))]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let observer = Arc::new(RecordingObserver::default());
        let progress: SharedProgressObserver = observer.clone();

        let tmp = tempfile::tempdir().unwrap();
        let coordinator = AgentCoordinator::new(registry, tmp.path().to_path_buf(), None);

        coordinator
            .run_plan_with_progress(&plan, Some(progress))
            .await
            .expect("plan executes");

        let events = observer.events();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProgressEvent::PhaseCompleted { .. })),
            "phase completed event should be emitted"
        );
    }
}

#[cfg(test)]
mod verbose {
    use super::*;
    use crate::error::PhaseRunnerError;
    use crate::phase_runner::{run_phase_with_progress, AgentRegistry, FileOutput};
    use ath_agents::MockBackend;
    use ath_types::agent::AgentResponse;
    use ath_types::plan::{PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct TranscriptObserver {
        transcripts: Mutex<Vec<Transcript>>,
    }

    impl TranscriptObserver {
        fn transcripts(&self) -> Vec<Transcript> {
            self.transcripts.lock().unwrap().clone()
        }
    }

    impl ProgressObserver for TranscriptObserver {
        fn on_event(&self, event: ProgressEvent) {
            if let ProgressEvent::Transcript(transcript) = event {
                self.transcripts.lock().unwrap().push(transcript);
            }
        }

        fn captures_transcripts(&self) -> bool {
            true
        }
    }

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
            created_at: Utc::now(),
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

    mod tests {
        use super::*;

        #[tokio::test]
        async fn task_execution_emits_transcript_payloads_when_enabled() {
            let claude = AgentKind::Claude("opus-4".into());
            let gemini = AgentKind::Gemini("2.5-pro".into());
            let phase =
                make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

            let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            ))]));
            let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

            let mut registry = AgentRegistry::new();
            registry.register(claude.clone(), task_mock);
            registry.register(gemini.clone(), reviewer_mock);

            let observer = Arc::new(TranscriptObserver::default());
            let progress: SharedProgressObserver = observer.clone();
            let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

            run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
                .await
                .expect("phase executes");

            let transcripts = observer.transcripts();
            assert!(transcripts.iter().any(|transcript| {
                transcript.kind == TranscriptKind::Executor
                    && transcript.label == "task-1"
                    && transcript.prompt.contains("Implement task-1")
            }));
        }

        #[tokio::test]
        async fn review_dispatch_emits_reviewer_prompt_and_verdict_transcript() {
            let claude = AgentKind::Claude("opus-4".into());
            let gemini = AgentKind::Gemini("2.5-pro".into());
            let phase =
                make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

            let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            ))]));
            let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

            let mut registry = AgentRegistry::new();
            registry.register(claude.clone(), task_mock);
            registry.register(gemini.clone(), reviewer_mock);

            let observer = Arc::new(TranscriptObserver::default());
            let progress: SharedProgressObserver = observer.clone();
            let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

            run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
                .await
                .expect("phase executes");

            let transcripts = observer.transcripts();
            assert!(transcripts.iter().any(|transcript| {
                transcript.kind == TranscriptKind::Reviewer
                    && transcript.prompt.contains("## Phase Review: phase-1")
                    && transcript.response.contains("\"passed\":true")
            }));
        }

        #[tokio::test]
        async fn retry_feedback_is_exposed_as_transcript_context_for_next_attempt() {
            let claude = AgentKind::Claude("opus-4".into());
            let gemini = AgentKind::Gemini("2.5-pro".into());
            let phase =
                make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

            let task_mock = Arc::new(MockBackend::new(vec![
                Ok(mock_response(
                    &make_task_output_json("task-1", &claude, "src/main.rs"),
                    &claude,
                )),
                Ok(mock_response(
                    &make_task_output_json("task-1", &claude, "src/main.rs"),
                    &claude,
                )),
            ]));
            let reviewer_mock = Arc::new(MockBackend::new(vec![
                Ok(mock_response(&failing_verdict_json("needs work"), &gemini)),
                Ok(mock_response(&passing_verdict_json(), &gemini)),
            ]));

            let mut registry = AgentRegistry::new();
            registry.register(claude.clone(), task_mock);
            registry.register(gemini.clone(), reviewer_mock);

            let observer = Arc::new(TranscriptObserver::default());
            let progress: SharedProgressObserver = observer.clone();
            let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

            run_phase_with_progress(&phase, &registry, |_| true, write_files, None, Some(progress))
                .await
                .expect("phase executes");

            let transcripts = observer.transcripts();
            assert!(transcripts.iter().any(|transcript| {
                transcript.kind == TranscriptKind::RetryFeedback
                    && transcript.response.contains("needs work")
            }));
        }

        #[tokio::test]
        async fn normal_mode_runs_without_transcript_capture_enabled() {
            let claude = AgentKind::Claude("opus-4".into());
            let gemini = AgentKind::Gemini("2.5-pro".into());
            let phase =
                make_phase(1, "phase-1", vec![make_task_spec("task-1", Some(claude.clone()))]);

            let task_mock = Arc::new(MockBackend::new(vec![Ok(mock_response(
                &make_task_output_json("task-1", &claude, "src/main.rs"),
                &claude,
            ))]));
            let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

            let mut registry = AgentRegistry::new();
            registry.register(claude.clone(), task_mock);
            registry.register(gemini.clone(), reviewer_mock);

            let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

            let record =
                run_phase_with_progress(&phase, &registry, |_| true, write_files, None, None)
                    .await
                    .expect("phase executes");
            assert_eq!(record.review_attempts.len(), 1);
        }
    }
}
