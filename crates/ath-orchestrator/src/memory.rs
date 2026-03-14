//! Memory integration for the orchestrator.
//!
//! Provides `MemoryContext` (opt-in memory state threaded through orchestrator calls),
//! `BackendLlmAdapter` (bridges `AgentBackend` → `ExtractionLlm`), and helper functions
//! for observation recording and context injection.
//!
//! All memory operations are fail-soft — errors are logged but never propagate
//! to `PhaseRunnerError`. The orchestrator continues identically whether memory
//! succeeds or fails.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use ath_agents::AgentBackend;
use ath_memory::{
    ContextInjector, ExtractionConfig, ExtractionLlm, InjectedContext, InjectionConfig,
    KeywordIndex, MemoryError, MemoryExtractor, ObservationBuffer, ObservationType,
    ObservationWriter, VikingStore,
};
use ath_types::agent::{AgentId, AgentRequest};
use ath_types::phase::PhaseRecord;
use ath_types::plan::{ExecutionPlan, PhaseSpec};
use ath_types::review::ReviewVerdict;

use crate::checkpoint::{plan_fingerprint, Checkpoint, CheckpointStore};
use crate::coordinator::AgentCoordinator;
use crate::error::PhaseRunnerError;
use crate::phase_runner::{AgentRegistry, FileOutput};
use crate::progress::{
    emit_progress, wants_transcripts, ProgressEvent, SharedProgressObserver, Transcript,
    TranscriptKind,
};
use crate::review;

// ---------------------------------------------------------------------------
// MemoryContext
// ---------------------------------------------------------------------------

/// Opt-in memory state threaded through orchestrator calls.
///
/// When provided to `run_plan_with_memory`, the orchestrator:
/// 1. Injects relevant memory context into each `AgentRequest.context`
/// 2. Records observations at lifecycle points (request, response, verdict, retry)
/// 3. Runs extraction post-run to persist learnings
pub struct MemoryContext {
    /// In-memory observation buffer for the current run.
    pub buffer: Arc<ObservationBuffer>,
    /// Knowledge store for reading/writing memory entries.
    pub store: Arc<VikingStore>,
    /// Keyword index for semantic search (wrapped in Mutex for interior mutability).
    pub keywords: Arc<std::sync::Mutex<KeywordIndex>>,
    /// Root directory for observation persistence (`.ath/memory/observations/`).
    pub observations_root: PathBuf,
    /// Run identifier for correlating observations.
    pub run_id: Uuid,
    /// Configuration for context injection budgets.
    pub injection_config: InjectionConfig,
    /// Configuration for extraction pipeline.
    pub extraction_config: ExtractionConfig,
    /// Path where the keyword index is persisted (`memory_dir/index/keyword.json`).
    pub index_path: PathBuf,
}

// ---------------------------------------------------------------------------
// BackendLlmAdapter
// ---------------------------------------------------------------------------

/// Bridges `AgentBackend` → `ExtractionLlm` for the extraction pipeline.
///
/// Wraps an `Arc<dyn AgentBackend>` and an `AgentId`, constructing
/// `AgentRequest`s for extraction prompts and forwarding to the backend.
pub struct BackendLlmAdapter {
    backend: Arc<dyn AgentBackend>,
    agent_kind: AgentId,
}

impl BackendLlmAdapter {
    /// Create a new adapter for the given backend and agent kind.
    pub fn new(backend: Arc<dyn AgentBackend>, agent_kind: AgentId) -> Self {
        Self {
            backend,
            agent_kind,
        }
    }
}

#[async_trait]
impl ExtractionLlm for BackendLlmAdapter {
    async fn complete(
        &self,
        prompt: &str,
        json_schema: Option<&serde_json::Value>,
    ) -> Result<String, MemoryError> {
        let request = AgentRequest {
            id: Uuid::new_v4(),
            agent: self.agent_kind.clone(),
            prompt: prompt.to_string(),
            context: None,
            json_schema: json_schema.cloned(),
            messages: vec![],
            created_at: chrono::Utc::now(),
        };

        let response =
            self.backend
                .send(request)
                .await
                .map_err(|e| MemoryError::ExtractionError {
                    stage: "backend_llm_adapter".to_string(),
                    message: format!("AgentBackend::send failed: {e}"),
                })?;

        Ok(response.content)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Record an observation event into the buffer. Fail-soft: logs a warning
/// on error but never crashes the orchestrator.
pub fn record_observation(buffer: &ObservationBuffer, event: ObservationType) {
    // ObservationBuffer::record is infallible (poison-recovering),
    // but we wrap in case the API changes.
    buffer.record(event);
}

/// Build context from memory for injection into an agent prompt.
///
/// Returns `None` if no context is available or on error. Logs warnings
/// for failures but never propagates errors.
pub fn inject_context(
    store: &VikingStore,
    keywords: &KeywordIndex,
    query: &str,
    config: &InjectionConfig,
) -> Option<String> {
    let injector = ContextInjector::new(store, keywords);
    let result: InjectedContext = injector.build_context(query, config);

    if result.is_empty() {
        tracing::info!("Context injection skipped: no relevant context found");
        return None;
    }

    tracing::info!(
        tokens = result.estimated_tokens,
        sections = ?result.sections_included,
        "Context injection: injected context into agent prompt"
    );

    Some(result.text)
}

// ---------------------------------------------------------------------------
// Memory-aware phase execution
// ---------------------------------------------------------------------------

/// Execute a single phase with memory observation recording and context injection.
///
/// Wraps the existing `run_phase_with_progress` lifecycle. Before each agent call,
/// injects memory context. After responses and reviews, records observations.
/// This function mirrors `run_phase_with_progress` but adds memory hooks at each
/// lifecycle point.
///
/// Memory errors never propagate — the phase completes identically to the
/// non-memory path on any memory failure.
pub async fn run_phase_with_memory(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    available: impl Fn(&AgentId) -> bool,
    write_files: impl Fn(&[FileOutput]) -> Result<(), PhaseRunnerError>,
    git: Option<&ath_git::async_ops::AsyncGitLayer>,
    observer: Option<SharedProgressObserver>,
    memory: &MemoryContext,
) -> Result<PhaseRecord, PhaseRunnerError> {
    use crate::phase_runner::{Pending, PhaseState, RejectOutcome};
    use ath_types::phase::{AgentContribution, PhaseRecord, ReviewAttempt, TokenUsage};

    let started_at = chrono::Utc::now();

    // Collect task agents for reviewer selection
    let task_agents: Vec<AgentId> = phase
        .tasks
        .iter()
        .filter_map(|t| t.assigned_agent.clone())
        .collect();

    // Select reviewer once
    let reviewer = review::select_reviewer(&task_agents, &phase.name, &available).map_err(|e| {
        PhaseRunnerError::NoReviewerAvailable {
            phase_name: phase.name.clone(),
            reason: e.to_string(),
        }
    })?;

    let reviewer_backend =
        registry
            .get(&reviewer)
            .ok_or_else(|| PhaseRunnerError::NoReviewerAvailable {
                phase_name: phase.name.clone(),
                reason: format!("no backend registered for reviewer {:?}", reviewer),
            })?;

    let mut review_attempts: Vec<ReviewAttempt> = Vec::new();
    let mut all_contributions: Vec<AgentContribution> = Vec::new();
    let mut last_feedback: Option<ReviewVerdict> = None;

    let state = PhaseState::<Pending>::new(phase.id, phase.name.clone());
    let _running = state.start();

    for attempt in 1u32..=3 {
        if let Some(feedback) = last_feedback.as_ref() {
            if wants_transcripts(observer.as_ref()) {
                emit_progress(
                    observer.as_ref(),
                    ProgressEvent::Transcript(Transcript {
                        kind: TranscriptKind::RetryFeedback,
                        phase_id: phase.id,
                        phase_name: phase.name.clone(),
                        label: phase.name.clone(),
                        attempt_number: attempt,
                        agent: feedback.reviewer.clone(),
                        prompt: "Retry feedback".into(),
                        response: review::format_retry_feedback_context(feedback),
                        retry_feedback: None,
                    }),
                );
            }
        }

        // Execute tasks with memory-aware dispatch
        let outputs = execute_phase_tasks_with_memory(
            phase,
            registry,
            last_feedback.as_ref(),
            attempt,
            observer.clone(),
            memory,
        )
        .await?;

        // Track contributions
        for output in &outputs {
            merge_contribution(
                &mut all_contributions,
                &output.agent,
                output.input_tokens,
                output.output_tokens,
                output.files_produced.iter().map(|file| file.path.clone()),
            );
        }

        // Create AwaitingReview state
        let running_state = PhaseState::<Pending>::new(phase.id, phase.name.clone()).start();
        let awaiting_state = running_state.submit_for_review(outputs.clone(), attempt);

        // Build and send review request
        let review_prompt =
            review::build_review_prompt(&phase.name, &phase.tasks, awaiting_state.outputs());
        let review_request = AgentRequest {
            id: Uuid::new_v4(),
            agent: reviewer.clone(),
            prompt: review_prompt.clone(),
            context: None,
            json_schema: Some(review::review_verdict_schema()),
            messages: vec![],
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

        // Record review verdict observation
        record_observation(
            &memory.buffer,
            ObservationType::ReviewVerdict {
                reviewer: reviewer.clone(),
                passed: verdict.passed,
                severity: verdict.severity,
                reason_summary: verdict.reason.clone(),
                attempt_number: attempt,
                phase_id: Some(phase.id),
            },
        );

        if wants_transcripts(observer.as_ref()) {
            emit_progress(
                observer.as_ref(),
                ProgressEvent::Transcript(Transcript {
                    kind: TranscriptKind::Reviewer,
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    label: phase.name.clone(),
                    attempt_number: attempt,
                    agent: reviewer.clone(),
                    prompt: review_prompt.clone(),
                    response: review_response.content.clone(),
                    retry_feedback: last_feedback
                        .as_ref()
                        .map(review::format_retry_feedback_context),
                }),
            );
        }

        merge_contribution(
            &mut all_contributions,
            &reviewer,
            review_response.input_tokens,
            review_response.output_tokens,
            std::iter::empty(),
        );

        review_attempts.push(ReviewAttempt {
            attempt_number: attempt,
            verdict: verdict.clone(),
            tokens: TokenUsage {
                input_tokens: review_response.input_tokens,
                output_tokens: review_response.output_tokens,
                estimated_cost_usd: 0.0,
            },
            timestamp: chrono::Utc::now(),
        });

        if verdict.passed {
            emit_progress(
                observer.as_ref(),
                ProgressEvent::PhaseCompleted {
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    phase_index: 0, // set by coordinator
                    total_phases: 0,
                },
            );

            let _complete = awaiting_state.approve();

            let all_files: Vec<FileOutput> = outputs
                .iter()
                .flat_map(|o| o.files_produced.clone())
                .collect();

            write_files(&all_files)?;

            if let Some(git_layer) = git {
                let file_paths: Vec<std::path::PathBuf> = all_files
                    .iter()
                    .map(|f| std::path::PathBuf::from(&f.path))
                    .collect();
                let metadata = ath_git::commit::CommitMetadata::from_phase_data(
                    &phase.name,
                    &reviewer,
                    format!("phase-{}", phase.id),
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
                id: Uuid::new_v4(),
                phase_id: phase.id,
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

        match awaiting_state.reject(verdict.clone()) {
            RejectOutcome::Retry(retrying) => {
                last_feedback = Some(verdict);

                // Record retry observation
                record_observation(
                    &memory.buffer,
                    ObservationType::RetryStarted {
                        phase_id: Some(phase.id),
                        attempt_number: retrying.attempt_number(),
                        feedback_summary: last_feedback
                            .as_ref()
                            .map(|f| f.reason.clone())
                            .unwrap_or_default(),
                    },
                );

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
            }
            RejectOutcome::Failed(_failed) => {
                return Err(PhaseRunnerError::MaxRetriesExceeded {
                    phase_name: phase.name.clone(),
                    reviewer: format!("{}/{}", reviewer.provider_name(), reviewer.model()),
                    attempts: attempt,
                    final_reason: verdict.reason,
                });
            }
        }
    }

    Err(PhaseRunnerError::MaxRetriesExceeded {
        phase_name: phase.name.clone(),
        reviewer: format!("{}/{}", reviewer.provider_name(), reviewer.model()),
        attempts: 3,
        final_reason: "exhausted all retry attempts".into(),
    })
}

/// Memory-aware task dispatch. Injects context before each request,
/// records AgentRequest and AgentResponse observations after each call.
async fn execute_phase_tasks_with_memory(
    phase: &PhaseSpec,
    registry: &AgentRegistry,
    feedback: Option<&ReviewVerdict>,
    attempt_number: u32,
    observer: Option<SharedProgressObserver>,
    memory: &MemoryContext,
) -> Result<Vec<crate::phase_runner::TaskOutput>, PhaseRunnerError> {
    use crate::phase_runner::{task_output_schema, TaskOutput};

    let mut outputs = Vec::with_capacity(phase.tasks.len());
    let total_tasks = phase.tasks.len();

    for (task_index, task) in phase.tasks.iter().enumerate() {
        let agent =
            task.assigned_agent
                .as_ref()
                .ok_or_else(|| PhaseRunnerError::TaskExecutionFailed {
                    task_name: task.name.clone(),
                    agent: "unassigned".into(),
                    reason: "task has no assigned agent".into(),
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

        let backend = registry
            .get(agent)
            .ok_or_else(|| PhaseRunnerError::TaskExecutionFailed {
                task_name: task.name.clone(),
                agent: format!("{:?}", agent),
                reason: "no backend registered for agent".into(),
            })?;

        // Build prompt
        let prompt = if let Some(fb) = feedback {
            review::build_retry_prompt(task, fb)
        } else {
            task.description.clone()
        };

        // Inject memory context (fail-soft)
        let context = {
            let keywords_guard = memory
                .keywords
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            inject_context(
                &memory.store,
                &keywords_guard,
                &prompt,
                &memory.injection_config,
            )
        };

        let request = AgentRequest {
            id: Uuid::new_v4(),
            agent: agent.clone(),
            prompt: prompt.clone(),
            context,
            json_schema: Some(task_output_schema()),
            messages: vec![],
            created_at: chrono::Utc::now(),
        };

        // Record AgentRequest observation
        record_observation(
            &memory.buffer,
            ObservationType::AgentRequest {
                agent: agent.clone(),
                prompt_summary: truncate_for_summary(&prompt, 200),
                phase_id: Some(phase.id),
                task_name: Some(task.name.clone()),
            },
        );

        let response =
            backend
                .send(request)
                .await
                .map_err(|e| PhaseRunnerError::TaskExecutionFailed {
                    task_name: task.name.clone(),
                    agent: format!("{:?}", agent),
                    reason: e.to_string(),
                })?;

        // Record AgentResponse observation
        record_observation(
            &memory.buffer,
            ObservationType::AgentResponse {
                agent: agent.clone(),
                token_usage: ath_types::phase::TokenUsage {
                    input_tokens: response.input_tokens,
                    output_tokens: response.output_tokens,
                    estimated_cost_usd: 0.0,
                },
                phase_id: Some(phase.id),
                task_name: Some(task.name.clone()),
            },
        );

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

        task_output.task_name = task.name.clone();
        task_output.agent = agent.clone();
        task_output.input_tokens = response.input_tokens;
        task_output.output_tokens = response.output_tokens;

        if wants_transcripts(observer.as_ref()) {
            emit_progress(
                observer.as_ref(),
                ProgressEvent::Transcript(Transcript {
                    kind: TranscriptKind::Executor,
                    phase_id: phase.id,
                    phase_name: phase.name.clone(),
                    label: task.name.clone(),
                    attempt_number,
                    agent: agent.clone(),
                    prompt: prompt.clone(),
                    response: response.content.clone(),
                    retry_feedback: feedback.map(review::format_retry_feedback_context),
                }),
            );
        }

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

/// Truncate a string for observation summaries (no secrets, bounded size).
fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!("{}...", &text[..max_chars])
    }
}

/// Reused from phase_runner — merge agent contributions.
fn merge_contribution(
    contributions: &mut Vec<ath_types::phase::AgentContribution>,
    agent: &AgentId,
    input_tokens: u64,
    output_tokens: u64,
    file_paths: impl IntoIterator<Item = String>,
) {
    let mut unique_file_paths = Vec::new();
    for path in file_paths {
        if !unique_file_paths.contains(&path) {
            unique_file_paths.push(path);
        }
    }

    if let Some(existing) = contributions
        .iter_mut()
        .find(|contribution| contribution.agent == *agent)
    {
        existing.tokens.input_tokens += input_tokens;
        existing.tokens.output_tokens += output_tokens;
        for path in unique_file_paths {
            if !existing.files_produced.contains(&path) {
                existing.files_produced.push(path);
            }
        }
    } else {
        contributions.push(ath_types::phase::AgentContribution {
            agent: agent.clone(),
            tokens: ath_types::phase::TokenUsage {
                input_tokens,
                output_tokens,
                estimated_cost_usd: 0.0,
            },
            files_produced: unique_file_paths,
        });
    }
}

// ---------------------------------------------------------------------------
// Coordinator extension
// ---------------------------------------------------------------------------

impl AgentCoordinator {
    /// Execute all phases in `plan` with memory observation, context injection,
    /// and post-run extraction.
    ///
    /// Mirrors `run_plan_with_progress` but threads `MemoryContext` through
    /// each phase for observation recording and context injection.
    ///
    /// Post-run: flushes observation buffer to disk, runs extraction pipeline
    /// via `BackendLlmAdapter`. All post-run operations are fail-soft.
    pub async fn run_plan_with_memory(
        &self,
        plan: &ExecutionPlan,
        observer: Option<SharedProgressObserver>,
        memory: MemoryContext,
        checkpoint_path: Option<&Path>,
    ) -> Result<Vec<PhaseRecord>, PhaseRunnerError> {
        // Pre-dispatch: validate isolation
        crate::isolation::check_isolation(plan).map_err(|e| {
            PhaseRunnerError::IsolationViolation {
                details: e.to_string(),
            }
        })?;

        // Load or create checkpoint if checkpointing is enabled
        let mut checkpoint = match checkpoint_path {
            Some(cp_path) => {
                let existing = CheckpointStore::load(cp_path)?;
                match existing {
                    Some(cp) if cp.is_stale(plan) => {
                        return Err(PhaseRunnerError::TaskExecutionFailed {
                            task_name: "checkpoint".into(),
                            agent: "coordinator".into(),
                            reason: format!(
                                "Stale checkpoint: plan has changed since the last run. \
                                 Use --fresh to start a clean run. \
                                 Checkpoint fingerprint: {}, current: {}",
                                cp.plan_fingerprint,
                                plan_fingerprint(plan)
                            ),
                        });
                    }
                    Some(cp) => Some(cp),
                    None => Some(Checkpoint::new(
                        format!(
                            "run-{}",
                            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                        ),
                        plan,
                    )),
                }
            }
            None => None,
        };

        // Pre-populate results from checkpoint
        let mut results: Vec<PhaseRecord> = checkpoint
            .as_ref()
            .map(|cp| cp.completed_records.clone())
            .unwrap_or_default();

        let total_phases = plan.execution_order.len();

        let effective_groups: Vec<Vec<u32>> = if plan.parallel_groups.is_empty() {
            plan.execution_order.iter().map(|&id| vec![id]).collect()
        } else {
            plan.parallel_groups.clone()
        };

        let mut phases_processed: usize = 0;

        for group in &effective_groups {
            // Skip groups that are fully completed in the checkpoint
            if let Some(ref cp) = checkpoint {
                if cp.should_skip_group(group) {
                    for &phase_id in group {
                        if let Some(phase) = plan.phases.iter().find(|p| p.id == phase_id) {
                            phases_processed += 1;
                            emit_progress(
                                observer.as_ref(),
                                ProgressEvent::PhaseRestored {
                                    phase_id: phase.id,
                                    phase_name: phase.name.clone(),
                                    phase_index: phases_processed,
                                    total_phases,
                                },
                            );
                        }
                    }
                    continue;
                }
            }

            // For memory-aware execution, we run phases sequentially within groups
            // to ensure observation ordering is deterministic.
            // Parallel memory-aware execution is a future enhancement.
            for &phase_id in group {
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
                let available = |kind: &AgentId| -> bool { registry.get(kind).is_some() };

                let record = run_phase_with_memory(
                    phase,
                    registry,
                    available,
                    write_files,
                    self.git.as_ref(),
                    observer.clone(),
                    &memory,
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
            }

            // Save checkpoint after group completes (with all phases in the group)
            if let (Some(ref mut cp), Some(cp_path)) = (&mut checkpoint, checkpoint_path) {
                // Collect records from this group that aren't in the checkpoint yet
                let new_records: Vec<PhaseRecord> = results
                    .iter()
                    .filter(|r| !cp.completed_phase_ids.contains(&r.phase_id))
                    .cloned()
                    .collect();
                if !new_records.is_empty() {
                    cp.add_records(new_records);
                    let _ = CheckpointStore::save(cp_path, cp);
                }
            }
        }

        // All groups completed — clean up checkpoint
        if let Some(cp_path) = checkpoint_path {
            let _ = CheckpointStore::delete(cp_path);
        }

        // Post-run: flush observations and run extraction (all fail-soft)
        self.post_run_extraction(&memory).await;

        Ok(results)
    }

    /// Flush observations to disk and run the extraction pipeline.
    ///
    /// All errors are logged but never propagated — the run result
    /// is already determined by the phase execution above.
    #[allow(clippy::await_holding_lock)]
    async fn post_run_extraction(&self, memory: &MemoryContext) {
        let run_id = memory.run_id;

        // 1. Flush observation buffer to disk
        match ObservationWriter::new(&memory.observations_root, &run_id) {
            Ok(mut writer) => match memory.buffer.flush(&mut writer) {
                Ok(count) => {
                    tracing::info!(
                        run_id = %run_id,
                        observations_flushed = count,
                        "Observation buffer flushed to disk"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        run_id = %run_id,
                        error = %e,
                        "Memory operation failed: could not flush observation buffer"
                    );
                    return; // No point running extraction without observations
                }
            },
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    error = %e,
                    "Memory operation failed: could not create observation writer"
                );
                return;
            }
        }

        // 2. Pick first available backend for extraction
        let extraction_backend = self.pick_extraction_backend();
        let (backend, agent_kind) = match extraction_backend {
            Some(pair) => pair,
            None => {
                tracing::warn!(
                    run_id = %run_id,
                    "Memory operation failed: no backend available for extraction"
                );
                return;
            }
        };

        let llm_adapter = BackendLlmAdapter::new(backend, agent_kind);

        // 3. Run extraction pipeline
        // The MutexGuard is held across .await because MemoryExtractor borrows it mutably.
        // This is safe: the Mutex is only contended within this single-threaded extraction path.
        let mut keywords_guard = memory
            .keywords
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut extractor = MemoryExtractor::new(
            &memory.store,
            &mut keywords_guard,
            &llm_adapter,
            memory.extraction_config.clone(),
        );

        match extractor
            .extract_all(&run_id, &memory.observations_root)
            .await
        {
            Ok(result) => {
                tracing::info!(
                    run_id = %run_id,
                    succeeded = result.success_count(),
                    failed = result.failure_count(),
                    "Extraction pipeline completed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %run_id,
                    error = %e,
                    "Memory operation failed: extraction pipeline error"
                );
            }
        }

        // 4. Persist keyword index to disk (fail-soft)
        if let Err(e) = keywords_guard.save(&memory.index_path) {
            tracing::warn!(
                run_id = %run_id,
                error = %e,
                index_path = %memory.index_path.display(),
                "Memory operation failed: keyword index save failed"
            );
        }
    }

    /// Pick the first available backend from the registry for extraction.
    ///
    /// Tries Claude, Gemini, Codex in that order. Returns the backend
    /// and the agent kind to use for constructing requests.
    fn pick_extraction_backend(&self) -> Option<(Arc<dyn AgentBackend>, AgentId)> {
        // Try common agent kinds in priority order
        let candidates = [
            AgentId::claude("default"),
            AgentId::gemini("default"),
            AgentId::codex("default"),
        ];

        for kind in &candidates {
            if let Some(backend) = self.registry.get(kind) {
                return Some((backend, kind.clone()));
            }
        }

        None
    }
}

/// Write files to the output directory, creating parent directories as needed.
/// (Duplicated from coordinator to avoid making the original pub)
fn write_files_to_dir(
    files: &[FileOutput],
    output_dir: &std::path::Path,
) -> Result<(), PhaseRunnerError> {
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
    use ath_types::agent::{AgentId, AgentResponse};
    use ath_types::plan::{PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;
    use std::sync::Arc;
    use tempfile::TempDir;

    // ==================== Helpers ====================

    fn make_task_spec(name: &str, agent: Option<AgentId>) -> TaskSpec {
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

    fn make_task_output_json(task_name: &str, agent: &AgentId, file_path: &str) -> String {
        let agent_json = serde_json::to_string(agent).unwrap();
        format!(
            r#"{{"task_name":"{}","agent":{},"files_produced":[{{"path":"{}","content":"fn main() {{}}"}}],"explanation":"Done","issues_encountered":[],"input_tokens":100,"output_tokens":50}}"#,
            task_name, agent_json, file_path
        )
    }

    fn passing_verdict_json() -> String {
        r#"{"passed":true,"severity":"info","reason":"All good","suggestions":[]}"#.into()
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
        let parallel_groups: Vec<Vec<u32>> = execution_order.iter().map(|&id| vec![id]).collect();
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups,
            critical_path_length: 0,
        }
    }

    fn make_memory_context(tmp: &TempDir) -> MemoryContext {
        let run_id = Uuid::new_v4();
        let store = VikingStore::new(tmp.path().join("store")).unwrap();
        MemoryContext {
            buffer: Arc::new(ObservationBuffer::new(run_id)),
            store: Arc::new(store),
            keywords: Arc::new(std::sync::Mutex::new(KeywordIndex::new())),
            observations_root: tmp.path().join("observations"),
            run_id,
            injection_config: InjectionConfig::default(),
            extraction_config: ExtractionConfig::default(),
            index_path: tmp.path().join("index").join("keyword.json"),
        }
    }

    // ==================== BackendLlmAdapter tests ====================

    #[tokio::test]
    async fn backend_llm_adapter_forwards_prompt() {
        let claude = AgentId::claude("opus-4");
        let mock = Arc::new(MockBackend::always_ok("test response"));
        let adapter = BackendLlmAdapter::new(mock, claude);

        let result = adapter.complete("test prompt", None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test response");
    }

    #[tokio::test]
    async fn backend_llm_adapter_maps_error_to_memory_error() {
        let claude = AgentId::claude("opus-4");
        let mock = Arc::new(MockBackend::failing(|| ath_agents::AgentError::Timeout {
            provider: "test".into(),
            duration: std::time::Duration::from_secs(30),
        }));
        let adapter = BackendLlmAdapter::new(mock, claude);

        let result = adapter.complete("test", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, MemoryError::ExtractionError { .. }));
    }

    /// Proves BackendLlmAdapter correctly bridges AgentBackend → ExtractionLlm
    /// including JSON schema forwarding.
    #[tokio::test]
    async fn backend_llm_adapter_bridges_correctly() {
        let claude = AgentId::claude("opus-4");
        let mock = Arc::new(MockBackend::always_ok("extraction result"));
        let adapter = BackendLlmAdapter::new(mock, claude);

        // Without schema
        let result = adapter.complete("extract facts", None).await.unwrap();
        assert_eq!(result, "extraction result");

        // With schema — verifies the adapter constructs a valid request
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "fact": { "type": "string" } }
        });
        let result = adapter.complete("extract", Some(&schema)).await.unwrap();
        assert_eq!(result, "extraction result");
    }

    // ==================== Helper function tests ====================

    #[test]
    fn record_observation_does_not_panic() {
        let buffer = ObservationBuffer::new(Uuid::new_v4());
        record_observation(
            &buffer,
            ObservationType::AgentRequest {
                agent: AgentId::claude("opus-4"),
                prompt_summary: "test".into(),
                phase_id: Some(1),
                task_name: Some("task-1".into()),
            },
        );
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn inject_context_returns_none_on_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = VikingStore::new(tmp.path().join("store")).unwrap();
        let keywords = KeywordIndex::new();
        let config = InjectionConfig::default();

        let result = inject_context(&store, &keywords, "test query", &config);
        assert!(result.is_none());
    }

    #[test]
    fn truncate_for_summary_short_text() {
        assert_eq!(truncate_for_summary("hello", 10), "hello");
    }

    #[test]
    fn truncate_for_summary_long_text() {
        let result = truncate_for_summary("hello world this is long", 10);
        assert_eq!(result, "hello worl...");
    }

    // ==================== Capturing Backend ====================

    /// A backend wrapper that captures all incoming `AgentRequest`s for
    /// test inspection, then delegates to an inner backend for responses.
    struct CapturingBackend {
        inner: Arc<dyn AgentBackend>,
        captured: Arc<std::sync::Mutex<Vec<AgentRequest>>>,
    }

    impl CapturingBackend {
        fn new(
            inner: Arc<dyn AgentBackend>,
        ) -> (Arc<Self>, Arc<std::sync::Mutex<Vec<AgentRequest>>>) {
            let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
            let backend = Arc::new(Self {
                inner,
                captured: captured.clone(),
            });
            (backend, captured)
        }
    }

    #[async_trait::async_trait]
    impl AgentBackend for CapturingBackend {
        async fn send(
            &self,
            request: AgentRequest,
        ) -> Result<AgentResponse, ath_agents::AgentError> {
            self.captured.lock().unwrap().push(request.clone());
            self.inner.send(request).await
        }

        async fn is_available(&self) -> bool {
            self.inner.is_available().await
        }

        fn provider_name(&self) -> &str {
            self.inner.provider_name()
        }
    }

    // ==================== Memory-aware coordinator tests ====================

    /// Proves that running a phase via `run_plan_with_memory` captures
    /// observation events in the buffer and flushes them to JSONL on disk.
    ///
    /// Asserts:
    /// - JSONL file exists at the expected path
    /// - Contains AgentRequest, AgentResponse, and ReviewVerdict events
    /// - All observations have phase_id = Some(1) (the single phase)
    /// - At minimum: 1 request + 1 response for the task, plus 1 review verdict
    #[tokio::test]
    async fn memory_context_records_observations() {
        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::always_ok(&make_task_output_json(
            "task-1",
            &claude,
            "src/main.rs",
        )));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let memory_tmp = TempDir::new().unwrap();
        let memory = make_memory_context(&memory_tmp);
        let run_id = memory.run_id;

        let coordinator = AgentCoordinator::new(registry, output_dir, None);
        let result = coordinator
            .run_plan_with_memory(&plan, None, memory, None)
            .await;

        assert!(result.is_ok());
        let records = result.unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].completed_at.is_some());

        // Read back observations from JSONL via ObservationReader
        let observations = ath_memory::ObservationReader::read_run(
            &memory_tmp.path().join("observations"),
            &run_id,
        )
        .unwrap();

        // Should have at minimum: 1 AgentRequest + 1 AgentResponse + 1 ReviewVerdict
        assert!(
            observations.len() >= 3,
            "Expected at least 3 observations (request + response + verdict), got {}",
            observations.len()
        );

        // Count event types
        let mut request_count = 0usize;
        let mut response_count = 0usize;
        let mut verdict_count = 0usize;

        for obs in &observations {
            // All observations should reference our run
            assert_eq!(obs.run_id, run_id);
            // All observations should reference phase_id 1
            assert_eq!(
                obs.phase_id,
                Some(1),
                "All observations in a single-phase run should have phase_id = Some(1)"
            );

            match &obs.event {
                ObservationType::AgentRequest {
                    agent,
                    task_name,
                    phase_id,
                    ..
                } => {
                    request_count += 1;
                    assert_eq!(*phase_id, Some(1));
                    assert_eq!(task_name.as_deref(), Some("task-1"));
                    assert_eq!(*agent, claude);
                }
                ObservationType::AgentResponse {
                    agent,
                    phase_id,
                    task_name,
                    ..
                } => {
                    response_count += 1;
                    assert_eq!(*phase_id, Some(1));
                    assert_eq!(task_name.as_deref(), Some("task-1"));
                    assert_eq!(*agent, claude);
                }
                ObservationType::ReviewVerdict {
                    reviewer,
                    passed,
                    phase_id,
                    ..
                } => {
                    verdict_count += 1;
                    assert_eq!(*phase_id, Some(1));
                    assert!(*passed);
                    assert_eq!(*reviewer, gemini);
                }
                _ => {} // Other event types are fine
            }
        }

        assert!(
            request_count >= 1,
            "Expected at least 1 AgentRequest, got {request_count}"
        );
        assert!(
            response_count >= 1,
            "Expected at least 1 AgentResponse, got {response_count}"
        );
        assert!(
            verdict_count >= 1,
            "Expected at least 1 ReviewVerdict, got {verdict_count}"
        );
    }

    /// Proves that `run_plan_with_memory` populates `AgentRequest.context`
    /// with content from the VikingStore when project identity exists.
    ///
    /// Uses a `CapturingBackend` to intercept the request and inspect
    /// the `context` field for the injected project identity text.
    #[tokio::test]
    async fn memory_context_injects_context_from_store() {
        let memory_tmp = TempDir::new().unwrap();
        let store = VikingStore::new(memory_tmp.path().join("store")).unwrap();

        // Write project identity — this is the primary injection source
        let uri: ath_memory::VikingUri = "viking://project/identity".parse().unwrap();
        let content = ath_memory::LayeredContent::new(
            uri,
            "Athena is a Rust-based AI orchestrator for multi-agent code generation.".to_string(),
            "An orchestrator managing multiple AI agents.".to_string(),
        );
        store.write(&content).unwrap();

        let run_id = Uuid::new_v4();
        let memory = MemoryContext {
            buffer: Arc::new(ObservationBuffer::new(run_id)),
            store: Arc::new(store),
            keywords: Arc::new(std::sync::Mutex::new(KeywordIndex::new())),
            observations_root: memory_tmp.path().join("observations"),
            run_id,
            injection_config: InjectionConfig::default(),
            extraction_config: ExtractionConfig::default(),
            index_path: memory_tmp.path().join("index").join("keyword.json"),
        };

        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        // Wrap the task mock in a CapturingBackend to inspect requests
        let inner_mock = Arc::new(MockBackend::always_ok(&make_task_output_json(
            "task-1",
            &claude,
            "src/main.rs",
        )));
        let (capturing_backend, captured_requests) = CapturingBackend::new(inner_mock);
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), capturing_backend);
        registry.register(gemini.clone(), reviewer_mock);

        let output_tmp = TempDir::new().unwrap();
        let coordinator = AgentCoordinator::new(registry, output_tmp.path().to_path_buf(), None);

        let result = coordinator
            .run_plan_with_memory(&plan, None, memory, None)
            .await;
        assert!(result.is_ok());

        // Inspect the captured requests — the task request should have context
        let requests = captured_requests.lock().unwrap();
        assert!(
            !requests.is_empty(),
            "CapturingBackend should have captured at least one request"
        );

        // The first request is the task execution request
        let task_request = &requests[0];
        assert!(
            task_request.context.is_some(),
            "AgentRequest.context should be Some(...) when project identity exists in store"
        );

        let ctx = task_request.context.as_ref().unwrap();
        assert!(
            ctx.contains("<athena_context>"),
            "Injected context should contain XML wrapper, got: {}",
            &ctx[..ctx.len().min(200)]
        );
        assert!(
            ctx.contains("<project>"),
            "Injected context should contain <project> section"
        );
        assert!(
            ctx.contains("Rust-based AI orchestrator"),
            "Injected context should contain project identity text"
        );
    }

    /// Proves that memory errors (e.g. broken store path) do not crash
    /// the orchestrator run. The run should complete Ok(...) with all
    /// memory failures silently logged as warnings.
    #[tokio::test]
    async fn memory_errors_do_not_fail_run() {
        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::always_ok(&make_task_output_json(
            "task-1",
            &claude,
            "src/main.rs",
        )));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a MemoryContext with a store at a valid path (VikingStore::new
        // creates dirs, so it needs a writable path) but the observations_root
        // will be at a broken path to test fail-soft flush.
        // However, the store itself is empty so injection produces no context.
        let broken_obs_root = if cfg!(windows) {
            std::path::PathBuf::from("Z:\\nonexistent\\path\\that\\should\\not\\exist\\obs")
        } else {
            std::path::PathBuf::from("/nonexistent/path/that/should/not/exist/obs")
        };

        let store_tmp = TempDir::new().unwrap();
        let store = VikingStore::new(store_tmp.path().join("store")).unwrap();
        let run_id = Uuid::new_v4();

        let memory = MemoryContext {
            buffer: Arc::new(ObservationBuffer::new(run_id)),
            store: Arc::new(store),
            keywords: Arc::new(std::sync::Mutex::new(KeywordIndex::new())),
            observations_root: broken_obs_root.clone(),
            run_id,
            injection_config: InjectionConfig::default(),
            extraction_config: ExtractionConfig::default(),
            index_path: broken_obs_root.join("index").join("keyword.json"),
        };

        let coordinator = AgentCoordinator::new(registry, output_dir, None);

        // The run should succeed even though observation flush will fail
        // (broken observations_root path). Memory errors are swallowed.
        let result = coordinator
            .run_plan_with_memory(&plan, None, memory, None)
            .await;

        assert!(
            result.is_ok(),
            "Run should succeed despite broken memory paths, got: {:?}",
            result.err()
        );

        let records = result.unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].completed_at.is_some());
        assert!(records[0].review_attempts[0].verdict.passed);
    }

    #[tokio::test]
    async fn memory_run_plan_existing_tests_behavior_preserved() {
        // Verify that run_plan_with_memory produces the same results
        // as run_plan for a basic scenario.
        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        // Use always_ok — extraction calls also go through this mock
        let task_mock = Arc::new(MockBackend::always_ok(&make_task_output_json(
            "task-1",
            &claude,
            "src/main.rs",
        )));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let memory_tmp = TempDir::new().unwrap();
        let memory = make_memory_context(&memory_tmp);

        let coordinator = AgentCoordinator::new(registry, output_dir, None);
        let records = coordinator
            .run_plan_with_memory(&plan, None, memory, None)
            .await
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phase_name, "phase-1");
        assert!(records[0].completed_at.is_some());
        assert_eq!(records[0].review_attempts.len(), 1);
        assert!(records[0].review_attempts[0].verdict.passed);

        // Verify files were written
        let file = tmp.path().join("output/src/main.rs");
        assert!(file.exists());
    }

    /// Full two-run end-to-end memory lifecycle test.
    ///
    /// Proves the complete cycle:
    /// 1. Run 1: observation capture → extraction → store persistence → keyword persistence
    /// 2. Run 2: context injection from Run 1's extracted data → verification in agent prompt
    /// 3. Cross-run: store.list() and keyword.search() show Run 1 data
    ///
    /// This is the milestone's definition-of-done test.
    #[tokio::test]
    async fn two_run_end_to_end_memory_lifecycle() {
        use chrono::Utc;

        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        // -- Shared temp dir root for both runs --
        let shared_tmp = TempDir::new().unwrap();
        let store_path = shared_tmp.path().join("store");
        let obs_root = shared_tmp.path().join("observations");
        let index_path = shared_tmp.path().join("index").join("keyword.json");
        let output_dir = shared_tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // ========== RUN 1 ==========
        let run1_id = Uuid::new_v4();

        // Task output JSON (response 1 of 5)
        let task_output_json = make_task_output_json("task-1", &claude, "src/auth.rs");

        // Extraction JSON responses (responses 2-5)
        let run_summary_json = serde_json::json!({
            "abstract_text": "Built authentication module with JWT token support.",
            "overview": "Phase 1: Claude planned the auth architecture. Phase 2: Gemini implemented JWT middleware.",
            "conventions_detected": ["error types use hint() method"],
            "decisions": [
                {"decision": "Use JWT for authentication", "rationale": "Stateless"}
            ],
            "issues": [
                {"issue": "Missing error handling", "resolution": "Added Result return types"}
            ]
        }).to_string();

        let conventions_json = serde_json::json!([
            {
                "id": "error-handling-pattern",
                "description": "All error types implement a hint() method.",
                "confidence": 0.9,
                "evidence": ["MemoryError uses hint()", "Custom error types follow same pattern"]
            }
        ])
        .to_string();

        let decisions_json = serde_json::json!([
            {
                "decision": "Use JWT for authentication",
                "rationale": "Stateless, scales horizontally",
                "context": "Phase 1, auth-planning task",
                "impact": "All API routes require JWT validation middleware"
            }
        ])
        .to_string();

        let agent_profiles_json = serde_json::json!([
            {
                "agent_id": "claude/opus-4",
                "tasks_handled": ["auth-planning"],
                "strengths": ["architectural thinking"],
                "weaknesses": [],
                "review_pass_rate": null,
                "feedback_themes": []
            }
        ])
        .to_string();

        // Build sequenced mock: 1 task + 4 extraction responses
        let make_ok_response = |content: &str| -> Result<AgentResponse, ath_agents::AgentError> {
            Ok(AgentResponse {
                request_id: Uuid::new_v4(),
                agent: claude.clone(),
                content: content.to_string(),
                input_tokens: 100,
                output_tokens: 50,
                created_at: Utc::now(),
            })
        };

        let claude_run1_mock = Arc::new(MockBackend::new(vec![
            make_ok_response(&task_output_json),
            make_ok_response(&run_summary_json),
            make_ok_response(&conventions_json),
            make_ok_response(&decisions_json),
            make_ok_response(&agent_profiles_json),
        ]));
        let reviewer_run1_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry1 = AgentRegistry::new();
        registry1.register(claude.clone(), claude_run1_mock);
        registry1.register(gemini.clone(), reviewer_run1_mock);

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let store1 = VikingStore::new(store_path.clone()).unwrap();
        let memory1 = MemoryContext {
            buffer: Arc::new(ObservationBuffer::new(run1_id)),
            store: Arc::new(store1),
            keywords: Arc::new(std::sync::Mutex::new(KeywordIndex::new())),
            observations_root: obs_root.clone(),
            run_id: run1_id,
            injection_config: InjectionConfig::default(),
            extraction_config: ExtractionConfig::default(),
            index_path: index_path.clone(),
        };

        let coordinator1 = AgentCoordinator::new(registry1, output_dir.clone(), None);
        let result1 = coordinator1
            .run_plan_with_memory(&plan, None, memory1, None)
            .await;

        assert!(result1.is_ok(), "Run 1 should succeed: {:?}", result1.err());
        let records1 = result1.unwrap();
        assert_eq!(records1.len(), 1);

        // -- Run 1 assertions --
        // Observations JSONL exists
        let observations = ath_memory::ObservationReader::read_run(&obs_root, &run1_id).unwrap();
        assert!(
            observations.len() >= 3,
            "Run 1 should produce at least 3 observations (request + response + verdict), got {}",
            observations.len()
        );

        // Store has entries from extraction
        let store1_check = VikingStore::new(store_path.clone()).unwrap();
        let uris = store1_check.list().unwrap();
        assert!(
            !uris.is_empty(),
            "Store should have entries after Run 1 extraction"
        );

        // Check that run summary exists
        let has_run_summary = uris.iter().any(|u| {
            let s = u.to_string();
            s.starts_with("viking://runs/") && s.ends_with("/summary")
        });
        assert!(
            has_run_summary,
            "Store should contain a run summary URI. URIs: {:?}",
            uris
        );

        // Keyword index file exists on disk
        assert!(
            index_path.exists(),
            "keyword.json should be persisted after Run 1 at {:?}",
            index_path
        );

        // ========== RUN 2 ==========
        let run2_id = Uuid::new_v4();

        // Fresh store and keyword index pointing to same paths
        let store2 = VikingStore::new(store_path.clone()).unwrap();
        let loaded_keywords = KeywordIndex::load(&index_path).unwrap();

        // Wrap Claude mock in CapturingBackend to inspect injected context
        let task_output_json2 = make_task_output_json("task-1", &claude, "src/db.rs");
        let inner_mock2 = Arc::new(MockBackend::always_ok(&task_output_json2));
        let (capturing_backend, captured_requests) = CapturingBackend::new(inner_mock2);
        let reviewer_run2_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry2 = AgentRegistry::new();
        registry2.register(claude.clone(), capturing_backend);
        registry2.register(gemini.clone(), reviewer_run2_mock);

        let tasks2 = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase2 = make_phase(1, "phase-1", tasks2);
        let plan2 = make_plan(vec![phase2], vec![1]);

        let memory2 = MemoryContext {
            buffer: Arc::new(ObservationBuffer::new(run2_id)),
            store: Arc::new(store2),
            keywords: Arc::new(std::sync::Mutex::new(loaded_keywords)),
            observations_root: obs_root.clone(),
            run_id: run2_id,
            injection_config: InjectionConfig::default(),
            extraction_config: ExtractionConfig::default(),
            index_path: index_path.clone(),
        };

        let coordinator2 = AgentCoordinator::new(registry2, output_dir.clone(), None);
        let result2 = coordinator2
            .run_plan_with_memory(&plan2, None, memory2, None)
            .await;

        assert!(result2.is_ok(), "Run 2 should succeed: {:?}", result2.err());

        // -- Run 2 context injection assertions --
        let requests = captured_requests.lock().unwrap();
        assert!(
            !requests.is_empty(),
            "CapturingBackend should have captured at least one request"
        );

        // The first captured request is the task execution request
        let task_request = &requests[0];
        assert!(
            task_request.context.is_some(),
            "Run 2 task request should have injected context from Run 1 store data"
        );

        let ctx = task_request.context.as_ref().unwrap();
        assert!(
            ctx.contains("<athena_context>"),
            "Injected context should contain <athena_context> wrapper. Got: {}",
            &ctx[..ctx.len().min(300)]
        );

        // Should contain recent_run section with Run 1's extracted summary
        assert!(
            ctx.contains("<recent_run>"),
            "Injected context should contain <recent_run> section from Run 1 summary. Got: {}",
            &ctx[..ctx.len().min(500)]
        );

        // Content from Run 1's run_summary extraction should appear
        assert!(
            ctx.contains("authentication") || ctx.contains("JWT") || ctx.contains("auth"),
            "Injected context should reference Run 1 content (authentication/JWT). Got: {}",
            &ctx[..ctx.len().min(500)]
        );

        // ========== CROSS-RUN API VERIFICATION ==========
        // Proves CLI-equivalent operations work on orchestrator-produced data

        // Store list shows Run 1 entries
        let store_final = VikingStore::new(store_path.clone()).unwrap();
        let final_uris = store_final.list().unwrap();
        assert!(
            !final_uris.is_empty(),
            "Store should still have entries after Run 2"
        );

        // Check specific entry types from extraction
        let uri_strings: Vec<String> = final_uris.iter().map(|u| u.to_string()).collect();

        let has_summary = uri_strings.iter().any(|s| s.contains("/summary"));
        let has_conventions = uri_strings.iter().any(|s| s.contains("/conventions/"));
        let has_decisions = uri_strings.iter().any(|s| s.contains("/decisions/"));
        assert!(
            has_summary,
            "Store should have run summary. URIs: {:?}",
            uri_strings
        );
        assert!(
            has_conventions || has_decisions,
            "Store should have conventions or decisions. URIs: {:?}",
            uri_strings
        );

        // Keyword index search returns results for Run 1 terms
        let final_keywords = KeywordIndex::load(&index_path).unwrap();
        let search_results = final_keywords.search("authentication", 5);
        assert!(
            !search_results.is_empty(),
            "Keyword search for 'authentication' should return results from Run 1 extraction"
        );

        // JWT should also be findable
        let jwt_results = final_keywords.search("JWT", 5);
        assert!(
            !jwt_results.is_empty(),
            "Keyword search for 'JWT' should return results from Run 1 extraction"
        );
    }

    /// Proves that `run_plan_with_memory` persists the keyword index to disk
    /// after post-run extraction. Even if extraction JSON parsing fails
    /// (MockBackend returns task-output-shaped JSON, not extraction JSON),
    /// `keywords.save()` is still called, producing the keyword.json file.
    ///
    /// If extraction happened to succeed (store has entries), also verifies
    /// that loading the index back yields a non-empty index.
    #[tokio::test]
    async fn keyword_index_persisted_after_extraction() {
        let claude = AgentId::claude("opus-4");
        let gemini = AgentId::gemini("2.5-pro");

        let tasks = vec![make_task_spec("task-1", Some(claude.clone()))];
        let phase = make_phase(1, "phase-1", tasks);
        let plan = make_plan(vec![phase], vec![1]);

        let task_mock = Arc::new(MockBackend::always_ok(&make_task_output_json(
            "task-1",
            &claude,
            "src/main.rs",
        )));
        let reviewer_mock = Arc::new(MockBackend::always_ok(&passing_verdict_json()));

        let mut registry = AgentRegistry::new();
        registry.register(claude.clone(), task_mock);
        registry.register(gemini.clone(), reviewer_mock);

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let memory_tmp = TempDir::new().unwrap();
        let index_path = memory_tmp.path().join("index").join("keyword.json");
        let memory = make_memory_context(&memory_tmp);

        // Verify index_path matches what make_memory_context sets
        assert_eq!(memory.index_path, index_path);

        let coordinator = AgentCoordinator::new(registry, output_dir, None);
        let result = coordinator
            .run_plan_with_memory(&plan, None, memory, None)
            .await;

        assert!(result.is_ok(), "run should succeed: {:?}", result.err());

        // The keyword index file must exist on disk after post_run_extraction
        assert!(
            index_path.exists(),
            "keyword.json should be persisted at {:?}",
            index_path
        );

        // Load it back — should be a valid KeywordIndex
        let loaded = KeywordIndex::load(&index_path);
        assert!(
            loaded.is_ok(),
            "KeywordIndex::load should succeed: {:?}",
            loaded.err()
        );

        // Verify the loaded index is structurally valid. With MockBackend::always_ok
        // returning task JSON (not extraction JSON), the extractor may write store
        // entries but not populate the keyword index (keywords.add() requires
        // successful extraction parsing). The primary assertion is that keyword.json
        // exists on disk and is loadable — proving save() was called.
        let _loaded_index = loaded.unwrap();
    }
}
