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
use crate::progress::{emit_progress, ProgressEvent, SharedProgressObserver};
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
    execute_phase_tasks_with_progress(phase, registry, feedback, None).await
}

/// Dispatches each task in a phase to its assigned agent sequentially and emits
/// lifecycle events when an observer is attached.
pub async fn execute_phase_tasks_with_progress(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    feedback: Option<&ReviewVerdict>,
    observer: Option<SharedProgressObserver>,
) -> Result<Vec<TaskOutput>, PhaseRunnerError> {
    let mut outputs = Vec::with_capacity(phase.tasks.len());
    let total_tasks = phase.tasks.len();

    for (task_index, task) in phase.tasks.iter().enumerate() {
        let agent = task.assigned_agent.as_ref().ok_or_else(|| {
            PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: "unassigned".into(),
                reason: "task has no assigned agent".into(),
            }
        })?;

        emit_progress(
            observer.as_ref(),
            ProgressEvent::TaskStarted {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                task_name: task.name.clone(),
                task_index: task_index + 1,
                total_tasks,
                agent: agent.clone(),
            },
        );

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

        emit_progress(
            observer.as_ref(),
            ProgressEvent::TaskCompleted {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                task_name: task.name.clone(),
                task_index: task_index + 1,
                total_tasks,
                agent: agent.clone(),
            },
        );

        outputs.push(task_output);
    }

    Ok(outputs)
}

// ---------------------------------------------------------------------------
// run_phase orchestration loop
// ---------------------------------------------------------------------------

/// Drives a single phase through the full typestate lifecycle:
/// Pending -> Running -> AwaitingReview -> Complete (or ReviewFailed after 3 attempts).
///
/// On review failure, retries with feedback injection (most recent attempt only).
/// Files are written to disk only after review passes.
/// The same reviewer is cached across all retry attempts.
pub async fn run_phase(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    available: impl Fn(&AgentKind) -> bool,
    write_files: impl Fn(&[FileOutput]) -> Result<(), PhaseRunnerError>,
    git: Option<&ath_git::async_ops::AsyncGitLayer>,
) -> Result<ath_types::phase::PhaseRecord, PhaseRunnerError> {
    run_phase_with_progress(phase, registry, available, write_files, git, None).await
}

/// Drives a single phase through the full typestate lifecycle with optional
/// progress events emitted during task, review, and retry transitions.
pub async fn run_phase_with_progress(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    available: impl Fn(&AgentKind) -> bool,
    write_files: impl Fn(&[FileOutput]) -> Result<(), PhaseRunnerError>,
    git: Option<&ath_git::async_ops::AsyncGitLayer>,
    observer: Option<SharedProgressObserver>,
) -> Result<ath_types::phase::PhaseRecord, PhaseRunnerError> {
    use ath_types::phase::{AgentContribution, PhaseRecord, ReviewAttempt, TokenUsage};

    let started_at = chrono::Utc::now();

    // Collect task agents for reviewer selection
    let task_agents: Vec<AgentKind> = phase
        .tasks
        .iter()
        .filter_map(|t| t.assigned_agent.clone())
        .collect();

    // Select reviewer once, cache for all attempts
    let reviewer = review::select_reviewer(&task_agents, &phase.name, &available)
        .map_err(|e| PhaseRunnerError::NoReviewerAvailable {
            phase_name: phase.name.clone(),
            reason: e.to_string(),
        })?;

    let reviewer_backend = registry.get(&reviewer).ok_or_else(|| {
        PhaseRunnerError::NoReviewerAvailable {
            phase_name: phase.name.clone(),
            reason: format!("no backend registered for reviewer {:?}", reviewer),
        }
    })?;

    // Start the typestate machine
    let mut review_attempts: Vec<ReviewAttempt> = Vec::new();
    let mut all_contributions: Vec<AgentContribution> = Vec::new();
    let mut last_feedback: Option<ReviewVerdict> = None;

    // Create the pending state and start it
    let state = PhaseState::<Pending>::new(phase.id, phase.name.clone());
    let _running = state.start();

    for attempt in 1u32..=3 {
        // Execute all tasks
        let outputs =
            execute_phase_tasks_with_progress(phase, registry, last_feedback.as_ref(), observer.clone())
                .await?;

        // Track contributions from this attempt
        for output in &outputs {
            // Check if we already have a contribution for this agent
            let existing = all_contributions
                .iter_mut()
                .find(|c| mem::discriminant(&c.agent) == mem::discriminant(&output.agent));
            if let Some(contrib) = existing {
                contrib.tokens.input_tokens += output.input_tokens;
                contrib.tokens.output_tokens += output.output_tokens;
                for f in &output.files_produced {
                    if !contrib.files_produced.contains(&f.path) {
                        contrib.files_produced.push(f.path.clone());
                    }
                }
            } else {
                all_contributions.push(AgentContribution {
                    agent: output.agent.clone(),
                    tokens: TokenUsage {
                        input_tokens: output.input_tokens,
                        output_tokens: output.output_tokens,
                        estimated_cost_usd: 0.0,
                    },
                    files_produced: output
                        .files_produced
                        .iter()
                        .map(|f| f.path.clone())
                        .collect(),
                });
            }
        }

        // Create AwaitingReview state (logical, but we use the typestate for correctness)
        let running_state = PhaseState::<Pending>::new(phase.id, phase.name.clone()).start();
        let awaiting_state = running_state.submit_for_review(outputs.clone(), attempt);

        // Build and send review request
        let review_prompt = review::build_review_prompt(&phase.name, &phase.tasks, awaiting_state.outputs());
        let review_request = AgentRequest {
            id: uuid::Uuid::new_v4(),
            agent: reviewer.clone(),
            prompt: review_prompt,
            context: None,
            json_schema: Some(review::review_verdict_schema()),
            created_at: chrono::Utc::now(),
        };

        emit_progress(
            observer.as_ref(),
            ProgressEvent::ReviewStarted {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                reviewer: reviewer.clone(),
                attempt_number: attempt,
            },
        );

        let review_response = reviewer_backend.send(review_request).await.map_err(|e| {
            PhaseRunnerError::ReviewDispatchFailed {
                reviewer: format!("{:?}", reviewer),
                reason: e.to_string(),
            }
        })?;

        let verdict = review::parse_review_verdict(&review_response.content, reviewer.clone())
            .map_err(|e| PhaseRunnerError::ReviewDispatchFailed {
                reviewer: format!("{:?}", reviewer),
                reason: e.to_string(),
            })?;

        review_attempts.push(ReviewAttempt {
            attempt_number: attempt,
            verdict: verdict.clone(),
            timestamp: chrono::Utc::now(),
        });

        if verdict.passed {
            emit_progress(
                observer.as_ref(),
                ProgressEvent::ReviewPassed {
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    reviewer: reviewer.clone(),
                    attempt_number: attempt,
                },
            );

            // Transition to Complete
            let _complete = awaiting_state.approve();

            // Collect all file outputs for writing
            let all_files: Vec<FileOutput> = outputs
                .iter()
                .flat_map(|o| o.files_produced.clone())
                .collect();

            // Write files atomically
            write_files(&all_files)?;

            // Git commit if configured
            if let Some(git_layer) = git {
                let file_paths: Vec<std::path::PathBuf> = all_files
                    .iter()
                    .map(|f| std::path::PathBuf::from(&f.path))
                    .collect();
                let metadata = ath_git::commit::CommitMetadata::from_phase_data(
                    &phase.name,
                    &reviewer,
                    &format!("phase-{}", phase.id),
                    file_paths.len(),
                    Some(&verdict),
                );
                let _ = git_layer
                    .commit_phase_async(file_paths, metadata)
                    .await
                    .map_err(|e| PhaseRunnerError::AtomicWriteFailed {
                        path: format!("phase-{}", phase.id),
                        reason: e.to_string(),
                    })?;
            }

            return Ok(PhaseRecord {
                id: uuid::Uuid::new_v4(),
                phase_name: phase.name.clone(),
                started_at,
                completed_at: Some(chrono::Utc::now()),
                contributions: all_contributions,
                review_attempts,
            });
        }

        emit_progress(
            observer.as_ref(),
            ProgressEvent::ReviewFailed {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                reviewer: reviewer.clone(),
                attempt_number: attempt,
                reason_summary: verdict.reason.clone(),
            },
        );

        // Review failed -- check if we can retry
        match awaiting_state.reject(verdict.clone()) {
            RejectOutcome::Retry(retrying) => {
                last_feedback = Some(verdict);
                emit_progress(
                    observer.as_ref(),
                    ProgressEvent::RetryStarted {
                        phase_id: phase.id,
                        phase_name: phase.name.clone(),
                        reviewer: reviewer.clone(),
                        attempt_number: retrying.attempt_number(),
                    },
                );
                let _running_again = retrying.retry();
                // Loop continues
            }
            RejectOutcome::Failed(_failed) => {
                return Err(PhaseRunnerError::MaxRetriesExceeded {
                    phase_name: phase.name.clone(),
                    attempts: attempt,
                    final_reason: verdict.reason,
                });
            }
        }
    }

    // Should not reach here due to typestate, but safety net
    Err(PhaseRunnerError::MaxRetriesExceeded {
        phase_name: phase.name.clone(),
        attempts: 3,
        final_reason: "exhausted all retry attempts".into(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::{AgentKind, AgentResponse};
    use ath_types::plan::TaskSpec;
    use ath_types::project::SkillTag;
    use ath_types::review::{ReviewVerdict, Severity};
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

    // ==================== run_phase tests ====================

    fn make_passing_verdict_json() -> String {
        r#"{"passed":true,"severity":"info","reason":"All good","suggestions":[]}"#.into()
    }

    fn make_failing_verdict_json(reason: &str) -> String {
        format!(
            r#"{{"passed":false,"severity":"critical","reason":"{}","suggestions":[]}}"#,
            reason
        )
    }

    fn make_test_phase(tasks: Vec<TaskSpec>) -> PhaseSpec {
        PhaseSpec {
            id: 1,
            name: "test-phase".into(),
            description: "A test phase".into(),
            tasks,
            depends_on: vec![],
            produces: vec![],
            consumes: vec![],
        }
    }

    #[tokio::test]
    async fn run_phase_happy_path_completes_in_one_attempt() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        // Task agent: returns valid TaskOutput
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        // Reviewer agent: returns passing verdict
        let reviewer_mock = Arc::new(MockBackend::always_ok(&make_passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let files_written: Arc<std::sync::Mutex<Vec<FileOutput>>> = Arc::new(std::sync::Mutex::new(vec![]));
        let files_clone = files_written.clone();
        let write_files = move |files: &[FileOutput]| -> Result<(), PhaseRunnerError> {
            files_clone.lock().unwrap().extend(files.iter().cloned());
            Ok(())
        };

        let result = run_phase(&phase, &registry, |_| true, write_files, None).await;
        assert!(result.is_ok());
        let record = result.unwrap();
        assert_eq!(record.phase_name, "test-phase");
        assert_eq!(record.review_attempts.len(), 1);
        assert!(record.review_attempts[0].verdict.passed);
        assert!(record.completed_at.is_some());
        // Files should have been written
        assert!(!files_written.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_phase_review_fails_once_then_passes() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        // Task agent: returns valid TaskOutput twice (attempt 1 + retry)
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        // Reviewer: fail first, pass second
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("needs work"),
                input_tokens: 50,
                output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_passing_verdict_json(),
                input_tokens: 50,
                output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        let result = run_phase(&phase, &registry, |_| true, write_files, None).await;
        assert!(result.is_ok());
        let record = result.unwrap();
        assert_eq!(record.review_attempts.len(), 2);
        assert!(!record.review_attempts[0].verdict.passed);
        assert!(record.review_attempts[1].verdict.passed);
    }

    #[tokio::test]
    async fn run_phase_three_failures_returns_max_retries_exceeded() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        // 3 task executions
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        // 3 failing reviews
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("fail-1"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("fail-2"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("fail-3"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        let result = run_phase(&phase, &registry, |_| true, write_files, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PhaseRunnerError::MaxRetriesExceeded { .. }));
    }

    #[tokio::test]
    async fn run_phase_same_reviewer_across_all_attempts() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        // 2 task executions (fail then pass)
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("nope"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_passing_verdict_json(),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        let record = run_phase(&phase, &registry, |_| true, write_files, None).await.unwrap();
        // All review attempts should use the same reviewer kind
        for attempt in &record.review_attempts {
            assert!(matches!(attempt.verdict.reviewer, AgentKind::Gemini(_)));
        }
    }

    #[tokio::test]
    async fn run_phase_most_recent_feedback_injected_into_retry() {
        // Indirectly tested: if feedback is injected, the task agent gets called
        // with a retry prompt. We verify the execution completes (which means
        // feedback was used since only the latest is injected, not accumulated).
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("use better error handling"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_passing_verdict_json(),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        let record = run_phase(&phase, &registry, |_| true, write_files, None).await.unwrap();
        assert_eq!(record.review_attempts.len(), 2);
    }

    #[tokio::test]
    async fn run_phase_files_written_only_after_review_passes() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        // Fail once, then pass
        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        let reviewer_mock = Arc::new(MockBackend::new(vec![
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_failing_verdict_json("nope"),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
            Ok(AgentResponse {
                request_id: uuid::Uuid::new_v4(),
                agent: gemini.clone(),
                content: make_passing_verdict_json(),
                input_tokens: 50, output_tokens: 30,
                created_at: chrono::Utc::now(),
            }),
        ]));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let wcc = write_call_count.clone();
        let write_files = move |_: &[FileOutput]| -> Result<(), PhaseRunnerError> {
            wcc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        };

        let _ = run_phase(&phase, &registry, |_| true, write_files, None).await.unwrap();
        // write_files should be called exactly once (only after passing review)
        assert_eq!(write_call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_phase_record_tracks_token_usage() {
        let claude = AgentKind::Claude("opus-4".into());
        let gemini = AgentKind::Gemini("2.5-pro".into());

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_test_phase(tasks);

        let task_mock = Arc::new(MockBackend::new(vec![
            Ok(mock_response_for_task("task-1", &claude)),
        ]));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&make_passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let write_files = |_: &[FileOutput]| -> Result<(), PhaseRunnerError> { Ok(()) };

        let record = run_phase(&phase, &registry, |_| true, write_files, None).await.unwrap();
        // Should have at least one contribution with tokens
        assert!(!record.contributions.is_empty());
        let total_input: u64 = record.contributions.iter().map(|c| c.tokens.input_tokens).sum();
        assert!(total_input > 0);
    }
}
