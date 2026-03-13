//! Typestate-based phase runner state machine.
//!
//! Uses the typestate pattern to enforce valid state transitions at compile time.
//! Invalid transitions (e.g., `Pending::approve()`) are compile-time errors.
//!
//! Two representations:
//! - `PhaseState<S>` with PhantomData typestates for internal execution logic
//! - `PhaseStatus` enum for serialization, logging, and PhaseRecord

use std::marker::PhantomData;

use ath_types::agent::AgentKind;
use ath_types::review::ReviewVerdict;
use serde::{Deserialize, Serialize};

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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::agent::AgentKind;
    use ath_types::review::{ReviewVerdict, Severity};

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
}
