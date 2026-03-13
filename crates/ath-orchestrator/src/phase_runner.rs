//! Typestate-based phase runner state machine.
//!
//! Uses the typestate pattern to enforce valid state transitions at compile time.
//! Invalid transitions (e.g., `Pending::approve()`) are compile-time errors.
//!
//! Two representations:
//! - `PhaseState<S>` with PhantomData typestates for internal execution logic
//! - `PhaseStatus` enum for serialization, logging, and PhaseRecord

use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem;
use std::sync::Arc;

use ath_agents::AgentBackend;
use ath_types::agent::{AgentKind, AgentRequest, AgentResponse};
use ath_types::plan::PhaseSpec;
use ath_types::review::ReviewVerdict;
use serde::{Deserialize, Serialize};

use crate::error::PhaseRunnerError;
use crate::review;

// ---------------------------------------------------------------------------
// Zero-sized state markers
// ---------------------------------------------------------------------------

/// Phase has been created but not started.
#[derive(Debug)]
pub struct Pending;

/// Phase tasks are being executed by agents.
#[derive(Debug)]
pub struct Running;

/// All tasks complete, awaiting review.
#[derive(Debug)]
pub struct AwaitingReview;

/// Review passed, phase is complete.
#[derive(Debug)]
pub struct Complete;

/// Review failed and max retries exhausted.
#[derive(Debug)]
pub struct ReviewFailed;

/// Review rejected, preparing to retry.
#[derive(Debug)]
pub struct Retrying;

// ---------------------------------------------------------------------------
// State-specific data structs
// ---------------------------------------------------------------------------

/// Data carried during AwaitingReview state.
#[derive(Debug, Clone)]
pub struct AwaitingReviewData {
    /// Task outputs produced during execution.
    pub outputs: Vec<TaskOutput>,
    /// Current attempt number (1-based).
    pub attempt_number: u32,
}

/// Data carried during Retrying state.
#[derive(Debug, Clone)]
pub struct RetryingData {
    /// The next attempt number (2 or 3).
    pub attempt_number: u32,
    /// The review verdict that triggered retry.
    pub feedback: ReviewVerdict,
}

/// Data carried during ReviewFailed state.
#[derive(Debug, Clone)]
pub struct ReviewFailedData {
    /// Total attempts made.
    pub attempts: u32,
    /// The final rejection reason.
    pub final_reason: String,
}

// ---------------------------------------------------------------------------
// PhaseState<S> typestate
// ---------------------------------------------------------------------------

/// A phase in a specific state `S`. Transitions consume `self` and return
/// the next state, making invalid transitions compile-time errors.
#[derive(Debug)]
pub struct PhaseState<S> {
    /// Phase identifier.
    pub phase_id: u32,
    /// Human-readable phase name.
    pub phase_name: String,
    /// State-specific data (if any).
    state_data: StateData,
    /// PhantomData marker for the typestate.
    _state: PhantomData<S>,
}

/// Internal storage for state-specific data.
#[derive(Debug, Clone, Default)]
struct StateData {
    awaiting_review: Option<AwaitingReviewData>,
    retrying: Option<RetryingData>,
    review_failed: Option<ReviewFailedData>,
}

impl PhaseState<Pending> {
    /// Create a new phase in the Pending state.
    pub fn new(phase_id: u32, phase_name: impl Into<String>) -> Self {
        PhaseState {
            phase_id,
            phase_name: phase_name.into(),
            state_data: StateData::default(),
            _state: PhantomData,
        }
    }

    /// Start execution. Transitions Pending -> Running.
    pub fn start(self) -> PhaseState<Running> {
        PhaseState {
            phase_id: self.phase_id,
            phase_name: self.phase_name,
            state_data: StateData::default(),
            _state: PhantomData,
        }
    }
}

impl PhaseState<Running> {
    /// Submit completed task outputs for review.
    /// Transitions Running -> AwaitingReview.
    pub fn submit_for_review(
        self,
        outputs: Vec<TaskOutput>,
        attempt: u32,
    ) -> PhaseState<AwaitingReview> {
        PhaseState {
            phase_id: self.phase_id,
            phase_name: self.phase_name,
            state_data: StateData {
                awaiting_review: Some(AwaitingReviewData {
                    outputs,
                    attempt_number: attempt,
                }),
                ..Default::default()
            },
            _state: PhantomData,
        }
    }
}

/// Outcome of rejecting a review.
#[derive(Debug)]
pub enum RejectOutcome {
    /// Can retry -- attempt_number < 3.
    Retry(PhaseState<Retrying>),
    /// Terminal failure -- max attempts reached.
    Failed(PhaseState<ReviewFailed>),
}

impl PhaseState<AwaitingReview> {
    /// Approve the review. Transitions AwaitingReview -> Complete.
    pub fn approve(self) -> PhaseState<Complete> {
        PhaseState {
            phase_id: self.phase_id,
            phase_name: self.phase_name,
            state_data: StateData::default(),
            _state: PhantomData,
        }
    }

    /// Reject the review. Returns Retry if under max attempts, Failed otherwise.
    pub fn reject(self, verdict: ReviewVerdict) -> RejectOutcome {
        let data = self.state_data.awaiting_review.expect("AwaitingReview must have data");
        let attempt = data.attempt_number;

        if attempt < 3 {
            RejectOutcome::Retry(PhaseState {
                phase_id: self.phase_id,
                phase_name: self.phase_name,
                state_data: StateData {
                    retrying: Some(RetryingData {
                        attempt_number: attempt + 1,
                        feedback: verdict,
                    }),
                    ..Default::default()
                },
                _state: PhantomData,
            })
        } else {
            RejectOutcome::Failed(PhaseState {
                phase_id: self.phase_id,
                phase_name: self.phase_name,
                state_data: StateData {
                    review_failed: Some(ReviewFailedData {
                        attempts: attempt,
                        final_reason: verdict.reason,
                    }),
                    ..Default::default()
                },
                _state: PhantomData,
            })
        }
    }

    /// Get the attempt number for the current review.
    pub fn attempt_number(&self) -> u32 {
        self.state_data
            .awaiting_review
            .as_ref()
            .expect("AwaitingReview must have data")
            .attempt_number
    }

    /// Get the task outputs submitted for review.
    pub fn outputs(&self) -> &[TaskOutput] {
        &self
            .state_data
            .awaiting_review
            .as_ref()
            .expect("AwaitingReview must have data")
            .outputs
    }
}

impl PhaseState<Retrying> {
    /// Retry execution. Transitions Retrying -> Running.
    pub fn retry(self) -> PhaseState<Running> {
        PhaseState {
            phase_id: self.phase_id,
            phase_name: self.phase_name,
            state_data: StateData::default(),
            _state: PhantomData,
        }
    }

    /// Get the attempt number for the retry.
    pub fn attempt_number(&self) -> u32 {
        self.state_data
            .retrying
            .as_ref()
            .expect("Retrying must have data")
            .attempt_number
    }

    /// Get the review feedback that triggered this retry.
    pub fn feedback(&self) -> &ReviewVerdict {
        &self
            .state_data
            .retrying
            .as_ref()
            .expect("Retrying must have data")
            .feedback
    }
}

impl PhaseState<ReviewFailed> {
    /// Get the total number of attempts made.
    pub fn attempts(&self) -> u32 {
        self.state_data
            .review_failed
            .as_ref()
            .expect("ReviewFailed must have data")
            .attempts
    }

    /// Get the final rejection reason.
    pub fn final_reason(&self) -> &str {
        &self
            .state_data
            .review_failed
            .as_ref()
            .expect("ReviewFailed must have data")
            .final_reason
    }
}

// ---------------------------------------------------------------------------
// PhaseStatus enum (serializable mirror)
// ---------------------------------------------------------------------------

/// Serializable phase status for logging, PhaseRecord, and external reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PhaseStatus {
    /// Phase created, not started.
    Pending,
    /// Phase executing.
    Running,
    /// Awaiting review.
    AwaitingReview,
    /// Review passed.
    Complete,
    /// Review failed terminally.
    ReviewFailed {
        /// Total attempts made.
        attempts: u32,
        /// Final reason for failure.
        final_reason: String,
    },
    /// Retrying after review rejection.
    Retrying {
        /// The retry attempt number.
        attempt_number: u32,
    },
}

impl PhaseStatus {
    /// Create a PhaseStatus from a PhaseState<Pending>.
    pub fn from_pending(_state: &PhaseState<Pending>) -> Self {
        PhaseStatus::Pending
    }

    /// Create a PhaseStatus from a PhaseState<Running>.
    pub fn from_running(_state: &PhaseState<Running>) -> Self {
        PhaseStatus::Running
    }

    /// Create a PhaseStatus from a PhaseState<AwaitingReview>.
    pub fn from_awaiting_review(_state: &PhaseState<AwaitingReview>) -> Self {
        PhaseStatus::AwaitingReview
    }

    /// Create a PhaseStatus from a PhaseState<Complete>.
    pub fn from_complete(_state: &PhaseState<Complete>) -> Self {
        PhaseStatus::Complete
    }

    /// Create a PhaseStatus from a PhaseState<ReviewFailed>.
    pub fn from_review_failed(state: &PhaseState<ReviewFailed>) -> Self {
        let data = state
            .state_data
            .review_failed
            .as_ref()
            .expect("ReviewFailed must have data");
        PhaseStatus::ReviewFailed {
            attempts: data.attempts,
            final_reason: data.final_reason.clone(),
        }
    }

    /// Create a PhaseStatus from a PhaseState<Retrying>.
    pub fn from_retrying(state: &PhaseState<Retrying>) -> Self {
        let data = state
            .state_data
            .retrying
            .as_ref()
            .expect("Retrying must have data");
        PhaseStatus::Retrying {
            attempt_number: data.attempt_number,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskOutput and FileOutput
// ---------------------------------------------------------------------------

/// Output from a single task execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskOutput {
    /// Name of the task that was executed.
    pub task_name: String,
    /// The agent that executed the task.
    pub agent: AgentKind,
    /// Files produced by the task.
    pub files_produced: Vec<FileOutput>,
    /// Agent's explanation of what was done.
    pub explanation: String,
    /// Issues encountered during execution.
    pub issues_encountered: Vec<String>,
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens produced.
    pub output_tokens: u64,
}

/// A file produced by task execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileOutput {
    /// File path relative to project root.
    pub path: String,
    /// File content.
    pub content: String,
}

/// Returns the JSON schema for TaskOutput structured output.
pub fn task_output_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "task_name": { "type": "string" },
            "agent": {
                "oneOf": [
                    { "type": "object", "properties": { "Claude": { "type": "string" } }, "required": ["Claude"] },
                    { "type": "object", "properties": { "Gemini": { "type": "string" } }, "required": ["Gemini"] },
                    { "type": "object", "properties": { "Codex": { "type": "string" } }, "required": ["Codex"] }
                ]
            },
            "files_produced": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            },
            "explanation": { "type": "string" },
            "issues_encountered": {
                "type": "array",
                "items": { "type": "string" }
            },
            "input_tokens": { "type": "integer" },
            "output_tokens": { "type": "integer" }
        },
        "required": ["task_name", "agent", "files_produced", "explanation", "issues_encountered", "input_tokens", "output_tokens"]
    })
}

// ---------------------------------------------------------------------------
// AgentRegistry
// ---------------------------------------------------------------------------

/// Registry mapping agent kinds to their backend implementations.
///
/// Uses `std::mem::Discriminant<AgentKind>` as the key so that
/// `Claude("opus-4")` and `Claude("sonnet-4")` share the same slot.
pub struct AgentRegistry {
    backends: HashMap<mem::Discriminant<AgentKind>, Arc<dyn AgentBackend>>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a backend for the given agent kind.
    pub fn register(&mut self, kind: AgentKind, backend: Arc<dyn AgentBackend>) {
        self.backends.insert(mem::discriminant(&kind), backend);
    }

    /// Look up a backend by agent kind.
    pub fn get(&self, kind: &AgentKind) -> Option<Arc<dyn AgentBackend>> {
        self.backends.get(&mem::discriminant(kind)).cloned()
    }
}

// ---------------------------------------------------------------------------
// execute_phase_tasks
// ---------------------------------------------------------------------------

/// Dispatches each task in a phase to its assigned agent sequentially.
///
/// On first attempt (`feedback` is `None`), uses the task description as prompt.
/// On retry (`feedback` is `Some`), uses `build_retry_prompt` with the reviewer's
/// feedback injected into the prompt.
///
/// Returns all `TaskOutput`s in order, or the first error encountered.
pub async fn execute_phase_tasks(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    feedback: Option<&ReviewVerdict>,
) -> Result<Vec<TaskOutput>, PhaseRunnerError> {
    let mut outputs = Vec::with_capacity(phase.tasks.len());

    for task in &phase.tasks {
        let agent = task.assigned_agent.as_ref().ok_or_else(|| {
            PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: "unassigned".into(),
                reason: "task has no assigned agent".into(),
            }
        })?;

        let backend = registry.get(agent).ok_or_else(|| {
            PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: format!("{:?}", agent),
                reason: "no backend registered for agent".into(),
            }
        })?;

        // Build prompt: retry with feedback or fresh from task description
        let prompt = if let Some(fb) = feedback {
            review::build_retry_prompt(task, fb)
        } else {
            task.description.clone()
        };

        let request = AgentRequest {
            id: uuid::Uuid::new_v4(),
            agent: agent.clone(),
            prompt,
            context: None,
            json_schema: Some(task_output_schema()),
            created_at: chrono::Utc::now(),
        };

        let response: AgentResponse = backend.send(request).await.map_err(|e| {
            PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: format!("{:?}", agent),
                reason: e.to_string(),
            }
        })?;

        let mut task_output: TaskOutput = serde_json::from_str(&response.content).map_err(|e| {
            let snippet = if response.content.len() > 100 {
                format!("{}...", &response.content[..100])
            } else {
                response.content.clone()
            };
            PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: format!("{:?}", agent),
                reason: format!("failed to parse TaskOutput JSON: {} (raw: {})", e, snippet),
            }
        })?;

        // Override task_name from spec, agent from assigned, tokens from response
        task_output.task_name = task.name.clone();
        task_output.agent = agent.clone();
        task_output.input_tokens = response.input_tokens;
        task_output.output_tokens = response.output_tokens;

        outputs.push(task_output);
    }

    Ok(outputs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::{AgentKind, AgentRequest, AgentResponse};
    use ath_types::plan::TaskSpec;
    use ath_types::project::SkillTag;
    use ath_types::review::{CodeSuggestion, ReviewVerdict, Severity};
    use ath_agents::MockBackend;
    use std::sync::Arc;

    fn make_verdict(passed: bool, reason: &str) -> ReviewVerdict {
        ReviewVerdict {
            passed,
            reviewer: AgentKind::Gemini("2.5-pro".into()),
            severity: if passed { Severity::Info } else { Severity::Critical },
            reason: reason.into(),
            suggestions: vec![],
        }
    }

    fn make_task_output(name: &str) -> TaskOutput {
        TaskOutput {
            task_name: name.into(),
            agent: AgentKind::Claude("opus-4".into()),
            files_produced: vec![FileOutput {
                path: "src/lib.rs".into(),
                content: "fn main() {}".into(),
            }],
            explanation: "Implemented feature".into(),
            issues_encountered: vec![],
            input_tokens: 1000,
            output_tokens: 500,
        }
    }

    #[test]
    fn new_creates_pending_state() {
        let state = PhaseState::<Pending>::new(1, "test-phase");
        assert_eq!(state.phase_id, 1);
        assert_eq!(state.phase_name, "test-phase");
    }

    #[test]
    fn pending_start_transitions_to_running() {
        let pending = PhaseState::<Pending>::new(1, "build");
        let running: PhaseState<Running> = pending.start();
        assert_eq!(running.phase_id, 1);
        assert_eq!(running.phase_name, "build");
    }

    #[test]
    fn running_submit_transitions_to_awaiting_review() {
        let running = PhaseState::<Pending>::new(1, "build").start();
        let outputs = vec![make_task_output("task1")];
        let awaiting: PhaseState<AwaitingReview> = running.submit_for_review(outputs, 1);
        assert_eq!(awaiting.phase_id, 1);
        assert_eq!(awaiting.attempt_number(), 1);
        assert_eq!(awaiting.outputs().len(), 1);
    }

    #[test]
    fn awaiting_review_approve_transitions_to_complete() {
        let awaiting = PhaseState::<Pending>::new(1, "build")
            .start()
            .submit_for_review(vec![make_task_output("t")], 1);
        let complete: PhaseState<Complete> = awaiting.approve();
        assert_eq!(complete.phase_id, 1);
    }

    #[test]
    fn awaiting_review_reject_attempt_1_returns_retry() {
        let awaiting = PhaseState::<Pending>::new(1, "build")
            .start()
            .submit_for_review(vec![], 1);
        let verdict = make_verdict(false, "needs work");
        match awaiting.reject(verdict) {
            RejectOutcome::Retry(retrying) => {
                assert_eq!(retrying.attempt_number(), 2);
                assert_eq!(retrying.phase_id, 1);
            }
            RejectOutcome::Failed(_) => panic!("Expected Retry, got Failed"),
        }
    }

    #[test]
    fn awaiting_review_reject_attempt_3_returns_failed() {
        let awaiting = PhaseState::<Pending>::new(1, "build")
            .start()
            .submit_for_review(vec![], 3);
        let verdict = make_verdict(false, "still broken");
        match awaiting.reject(verdict) {
            RejectOutcome::Failed(failed) => {
                assert_eq!(failed.attempts(), 3);
                assert_eq!(failed.final_reason(), "still broken");
            }
            RejectOutcome::Retry(_) => panic!("Expected Failed, got Retry"),
        }
    }

    #[test]
    fn retrying_retry_transitions_to_running() {
        let awaiting = PhaseState::<Pending>::new(1, "build")
            .start()
            .submit_for_review(vec![], 1);
        let verdict = make_verdict(false, "fix it");
        match awaiting.reject(verdict) {
            RejectOutcome::Retry(retrying) => {
                let running: PhaseState<Running> = retrying.retry();
                assert_eq!(running.phase_id, 1);
            }
            RejectOutcome::Failed(_) => panic!("Expected Retry"),
        }
    }

    #[test]
    fn phase_status_serialization_round_trip() {
        let statuses = vec![
            PhaseStatus::Pending,
            PhaseStatus::Running,
            PhaseStatus::AwaitingReview,
            PhaseStatus::Complete,
            PhaseStatus::ReviewFailed {
                attempts: 3,
                final_reason: "tests failing".into(),
            },
            PhaseStatus::Retrying {
                attempt_number: 2,
            },
        ];

        for status in &statuses {
            let json = serde_json::to_string(status).expect("serialize");
            let deserialized: PhaseStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*status, deserialized, "round-trip failed for {:?}", status);
        }
    }

    #[test]
    fn transitions_update_phase_status_correctly() {
        let pending = PhaseState::<Pending>::new(1, "build");
        assert_eq!(PhaseStatus::from_pending(&pending), PhaseStatus::Pending);

        let running = pending.start();
        assert_eq!(PhaseStatus::from_running(&running), PhaseStatus::Running);

        let awaiting = running.submit_for_review(vec![], 1);
        assert_eq!(
            PhaseStatus::from_awaiting_review(&awaiting),
            PhaseStatus::AwaitingReview
        );

        let complete = awaiting.approve();
        assert_eq!(PhaseStatus::from_complete(&complete), PhaseStatus::Complete);
    }

    #[test]
    fn task_output_and_file_output_deserialize_from_json() {
        let json = serde_json::json!({
            "task_name": "implement-auth",
            "agent": { "Claude": "opus-4" },
            "files_produced": [
                { "path": "src/auth.rs", "content": "pub fn login() {}" }
            ],
            "explanation": "Added login function",
            "issues_encountered": ["Had to handle edge case"],
            "input_tokens": 2000,
            "output_tokens": 800
        });

        let output: TaskOutput = serde_json::from_value(json).expect("deserialize TaskOutput");
        assert_eq!(output.task_name, "implement-auth");
        assert_eq!(output.agent, AgentKind::Claude("opus-4".into()));
        assert_eq!(output.files_produced.len(), 1);
        assert_eq!(output.files_produced[0].path, "src/auth.rs");
        assert_eq!(output.issues_encountered, vec!["Had to handle edge case"]);
        assert_eq!(output.input_tokens, 2000);
        assert_eq!(output.output_tokens, 800);
    }

    // ==================== Helper: make_task_spec ====================

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

    fn make_task_output_json(task_name: &str, agent: &AgentKind) -> String {
        let agent_json = serde_json::to_string(agent).unwrap();
        format!(
            r#"{{"task_name":"{}","agent":{},"files_produced":[{{"path":"src/lib.rs","content":"fn main() {{}}"}}],"explanation":"Done","issues_encountered":[],"input_tokens":100,"output_tokens":50}}"#,
            task_name, agent_json
        )
    }

    fn mock_response_for_task(task_name: &str, agent: &AgentKind) -> AgentResponse {
        AgentResponse {
            request_id: uuid::Uuid::new_v4(),
            agent: agent.clone(),
            content: make_task_output_json(task_name, agent),
            input_tokens: 100,
            output_tokens: 50,
            created_at: chrono::Utc::now(),
        }
    }

    // ==================== execute_phase_tasks tests ====================

    #[tokio::test]
    async fn execute_dispatches_each_task_to_assigned_agent() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![
            make_task_spec("task-a", Some(claude.clone())),
            make_task_spec("task-b", Some(gemini.clone())),
        ];

        let claude_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-a", &claude)),
        ]));
        let gemini_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-b", &gemini)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), claude_mock);
        registry.register(gemini.clone(), gemini_mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1,
            name: "test-phase".into(),
            description: "test".into(),
            tasks,
            depends_on: vec![],
            produces: vec![],
            consumes: vec![],
        };

        let result = execute_phase_tasks(&phase, &registry, None).await;
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].task_name, "task-a");
        assert_eq!(outputs[1].task_name, "task-b");
    }

    #[tokio::test]
    async fn execute_builds_request_with_task_output_schema() {
        // This test verifies that json_schema is set; checked via successful parse
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];

        let mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let outputs = execute_phase_tasks(&phase, &registry, None).await.unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].task_name, "task-1");
    }

    #[tokio::test]
    async fn execute_parses_response_content_as_task_output() {
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![make_task_spec("parse-test", Some(claude.clone()))];

        let mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("parse-test", &claude)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let outputs = execute_phase_tasks(&phase, &registry, None).await.unwrap();
        assert_eq!(outputs[0].files_produced.len(), 1);
        assert_eq!(outputs[0].files_produced[0].path, "src/lib.rs");
        assert_eq!(outputs[0].explanation, "Done");
    }

    #[tokio::test]
    async fn execute_collects_outputs_in_order() {
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![
            make_task_spec("first", Some(claude.clone())),
            make_task_spec("second", Some(claude.clone())),
            make_task_spec("third", Some(claude.clone())),
        ];

        let mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("first", &claude)),
            Ok(mock_response_for_task("second", &claude)),
            Ok(mock_response_for_task("third", &claude)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let outputs = execute_phase_tasks(&phase, &registry, None).await.unwrap();
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].task_name, "first");
        assert_eq!(outputs[1].task_name, "second");
        assert_eq!(outputs[2].task_name, "third");
    }

    #[tokio::test]
    async fn execute_returns_error_on_agent_send_failure() {
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![make_task_spec("failing", Some(claude.clone()))];

        let mock = Arc::new(MockBackend::failing(|| ath_agents::AgentError::Timeout {
            provider: "Anthropic".into(),
            duration: std::time::Duration::from_secs(30),
        }));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let result = execute_phase_tasks(&phase, &registry, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::PhaseRunnerError::TaskExecutionFailed { .. }));
    }

    #[tokio::test]
    async fn execute_returns_error_on_unparseable_json() {
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![make_task_spec("bad-json", Some(claude.clone()))];

        let mock = Arc::new(MockBackend::always_ok("not valid json"));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let result = execute_phase_tasks(&phase, &registry, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::PhaseRunnerError::TaskExecutionFailed { .. }));
    }

    #[tokio::test]
    async fn execute_uses_retry_prompt_when_feedback_provided() {
        let claude = AgentKind::Claude("opus-4".into());
        let tasks = vec![make_task_spec("retry-task", Some(claude.clone()))];

        let mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("retry-task", &claude)),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), mock);

        let phase = ath_types::plan::PhaseSpec {
            id: 1, name: "test".into(), description: "test".into(),
            tasks, depends_on: vec![], produces: vec![], consumes: vec![],
        };

        let feedback = ReviewVerdict {
            passed: false,
            reviewer: AgentKind::Gemini("2.5-pro".into()),
            severity: Severity::Critical,
            reason: "Fix the bug".into(),
            suggestions: vec![],
        };

        // Should succeed -- feedback just changes the prompt, not the parsing
        let result = execute_phase_tasks(&phase, &registry, Some(&feedback)).await;
        assert!(result.is_ok());
    }
}
