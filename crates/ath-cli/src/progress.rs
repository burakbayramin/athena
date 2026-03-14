use std::io::{self, IsTerminal};
use std::sync::Mutex;
use std::time::Duration;

use ath_orchestrator::progress::{ProgressEvent, ProgressObserver};
use ath_types::agent::AgentId;
use indicatif::{ProgressBar, ProgressStyle};

use crate::GlobalArgs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Interactive,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStatus {
    Pending,
    Running,
    Done,
}

#[derive(Debug, Clone)]
struct TaskEntry {
    name: String,
    status: TaskStatus,
    agent: Option<AgentId>,
}

#[derive(Debug, Default)]
struct ReporterState {
    phase_name: Option<String>,
    phase_index: usize,
    total_phases: usize,
    tasks: Vec<TaskEntry>,
    active_task: Option<String>,
    active_agent: Option<AgentId>,
    snapshot: String,
    milestone_lines: Vec<String>,
    plain_lines: Vec<String>,
}

pub(crate) struct TerminalProgressReporter {
    mode: OutputMode,
    emit_stdout: bool,
    progress_bar: Option<ProgressBar>,
    state: Mutex<ReporterState>,
}

impl TerminalProgressReporter {
    pub(crate) fn new(global: GlobalArgs) -> Self {
        let interactive =
            io::stdout().is_terminal() && !global.no_color && std::env::var("NO_COLOR").is_err();
        Self::with_mode(interactive, true)
    }

    fn with_mode(interactive: bool, emit_stdout: bool) -> Self {
        let progress_bar = if interactive && emit_stdout {
            Some(build_progress_bar())
        } else {
            None
        };

        Self {
            mode: if interactive {
                OutputMode::Interactive
            } else {
                OutputMode::Plain
            },
            emit_stdout,
            progress_bar,
            state: Mutex::new(ReporterState::default()),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(interactive: bool) -> Self {
        Self::with_mode(interactive, false)
    }

    pub(crate) fn finish(&self, summary: &str) {
        match &self.progress_bar {
            Some(progress_bar) => {
                progress_bar.finish_and_clear();
                if self.emit_stdout {
                    println!("{summary}");
                }
            }
            None => {
                if self.emit_stdout {
                    println!("{summary}");
                }
            }
        }
    }

    pub(crate) fn print_durable_block(&self, block: &str) {
        match &self.progress_bar {
            Some(progress_bar) => {
                if self.emit_stdout {
                    progress_bar.println(block);
                }
            }
            None => {
                if self.emit_stdout {
                    println!("{block}");
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> String {
        self.state.lock().unwrap().snapshot.clone()
    }

    #[cfg(test)]
    pub(crate) fn milestone_lines(&self) -> Vec<String> {
        self.state.lock().unwrap().milestone_lines.clone()
    }

    #[cfg(test)]
    pub(crate) fn plain_lines(&self) -> Vec<String> {
        self.state.lock().unwrap().plain_lines.clone()
    }
}

impl ProgressObserver for TerminalProgressReporter {
    fn on_event(&self, event: ProgressEvent) {
        let (snapshot, milestone, plain_line) = {
            let mut state = self.state.lock().unwrap();
            apply_event(&mut state, &event);

            let milestone = milestone_line(&event);
            if let Some(line) = &milestone {
                state.milestone_lines.push(line.clone());
            }

            let plain_line = plain_line(&event);
            if let Some(line) = &plain_line {
                state.plain_lines.push(line.clone());
            }

            (state.snapshot.clone(), milestone, plain_line)
        };

        match self.mode {
            OutputMode::Interactive => {
                if let Some(progress_bar) = &self.progress_bar {
                    progress_bar.set_message(snapshot);
                    if let Some(line) = milestone {
                        progress_bar.println(line);
                    }
                }
            }
            OutputMode::Plain => {
                if self.emit_stdout {
                    if let Some(line) = plain_line {
                        println!("{line}");
                    }
                }
            }
        }
    }
}

fn build_progress_bar() -> ProgressBar {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(120));
    let style = ProgressStyle::with_template("{spinner} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_chars("-\\|/ ");
    progress_bar.set_style(style);
    progress_bar
}

fn apply_event(state: &mut ReporterState, event: &ProgressEvent) {
    match event {
        ProgressEvent::PhaseStarted {
            phase_name,
            phase_index,
            total_phases,
            tasks,
            ..
        } => {
            state.phase_name = Some(phase_name.clone());
            state.phase_index = *phase_index;
            state.total_phases = *total_phases;
            state.tasks = tasks
                .iter()
                .map(|task| TaskEntry {
                    name: task.clone(),
                    status: TaskStatus::Pending,
                    agent: None,
                })
                .collect();
            state.active_task = None;
            state.active_agent = None;
        }
        ProgressEvent::TaskStarted {
            task_name, agent, ..
        } => {
            update_task(state, task_name, TaskStatus::Running, Some(agent.clone()));
            state.active_task = Some(task_name.clone());
            state.active_agent = Some(agent.clone());
        }
        ProgressEvent::TaskCompleted {
            task_name, agent, ..
        } => {
            update_task(state, task_name, TaskStatus::Done, Some(agent.clone()));
            state.active_task = None;
            state.active_agent = None;
        }
        ProgressEvent::ReviewStarted { reviewer, .. } => {
            state.active_task = Some("review".into());
            state.active_agent = Some(reviewer.clone());
        }
        ProgressEvent::ReviewPassed { .. } => {
            state.active_task = None;
            state.active_agent = None;
        }
        ProgressEvent::ReviewFailed { reviewer, .. } => {
            state.active_task = Some("review failed".into());
            state.active_agent = Some(reviewer.clone());
        }
        ProgressEvent::RetryStarted {
            reviewer,
            attempt_number,
            ..
        } => {
            state.active_task = Some(format!("retry attempt {attempt_number}"));
            state.active_agent = Some(reviewer.clone());
        }
        ProgressEvent::PhaseCompleted { .. } => {
            state.active_task = None;
            state.active_agent = None;
        }
        ProgressEvent::PhaseRestored {
            phase_name,
            phase_index,
            total_phases,
            ..
        } => {
            state.phase_name = Some(phase_name.clone());
            state.phase_index = *phase_index;
            state.total_phases = *total_phases;
            state.tasks.clear();
            state.active_task = Some("restored from checkpoint".into());
            state.active_agent = None;
        }
        ProgressEvent::Transcript(_) => {}
    }

    state.snapshot = render_snapshot(state);
}

fn update_task(
    state: &mut ReporterState,
    task_name: &str,
    status: TaskStatus,
    agent: Option<AgentId>,
) {
    if let Some(task) = state.tasks.iter_mut().find(|task| task.name == task_name) {
        task.status = status;
        task.agent = agent;
    }
}

fn render_snapshot(state: &ReporterState) -> String {
    if state.phase_name.is_none() {
        return "Waiting for execution...".into();
    }

    let phase_name = state.phase_name.as_deref().unwrap_or("unknown");
    let mut lines = vec![format!(
        "Phase {}/{}: {}",
        state.phase_index, state.total_phases, phase_name
    )];

    let active_line = match (&state.active_task, &state.active_agent) {
        (Some(task), Some(agent)) => format!("Active: {} via {}", task, format_agent(agent)),
        (Some(task), None) => format!("Active: {task}"),
        _ => "Active: waiting".into(),
    };
    lines.push(active_line);
    lines.push("Tasks:".into());

    for task in &state.tasks {
        let marker = match task.status {
            TaskStatus::Pending => "[ ]",
            TaskStatus::Running => "[>]",
            TaskStatus::Done => "[x]",
        };
        let agent_suffix = task
            .agent
            .as_ref()
            .map(|agent| format!(" - {}", format_agent(agent)))
            .unwrap_or_default();
        lines.push(format!("  {} {}{}", marker, task.name, agent_suffix));
    }

    lines.join("\n")
}

fn milestone_line(event: &ProgressEvent) -> Option<String> {
    match event {
        ProgressEvent::ReviewStarted {
            reviewer,
            attempt_number,
            ..
        } => Some(format!(
            "Review started (attempt {}) by {}",
            attempt_number,
            format_agent(reviewer)
        )),
        ProgressEvent::ReviewPassed {
            reviewer,
            attempt_number,
            ..
        } => Some(format!(
            "Review passed (attempt {}) by {}",
            attempt_number,
            format_agent(reviewer)
        )),
        ProgressEvent::ReviewFailed {
            reviewer,
            attempt_number,
            reason_summary,
            ..
        } => Some(format!(
            "Review failed (attempt {}) by {}: {}",
            attempt_number,
            format_agent(reviewer),
            reason_summary
        )),
        ProgressEvent::RetryStarted {
            phase_name,
            attempt_number,
            ..
        } => Some(format!(
            "Retrying {} (attempt {})",
            phase_name, attempt_number
        )),
        ProgressEvent::PhaseCompleted { phase_name, .. } => {
            Some(format!("Phase complete: {phase_name}"))
        }
        ProgressEvent::PhaseRestored { phase_name, .. } => {
            Some(format!("Phase restored from checkpoint: {phase_name}"))
        }
        ProgressEvent::Transcript(_) => None,
        _ => None,
    }
}

fn plain_line(event: &ProgressEvent) -> Option<String> {
    match event {
        ProgressEvent::PhaseStarted {
            phase_name,
            phase_index,
            total_phases,
            ..
        } => Some(format!(
            "Phase {}/{} started: {}",
            phase_index, total_phases, phase_name
        )),
        ProgressEvent::TaskStarted {
            phase_name,
            task_name,
            task_index,
            total_tasks,
            agent,
            ..
        } => Some(format!(
            "{} task {}/{} running: {} via {}",
            phase_name,
            task_index,
            total_tasks,
            task_name,
            format_agent(agent)
        )),
        ProgressEvent::TaskCompleted {
            phase_name,
            task_name,
            task_index,
            total_tasks,
            agent,
            ..
        } => Some(format!(
            "{} task {}/{} complete: {} via {}",
            phase_name,
            task_index,
            total_tasks,
            task_name,
            format_agent(agent)
        )),
        ProgressEvent::ReviewStarted {
            phase_name,
            reviewer,
            attempt_number,
            ..
        } => Some(format!(
            "{} review started (attempt {}) by {}",
            phase_name,
            attempt_number,
            format_agent(reviewer)
        )),
        ProgressEvent::ReviewPassed {
            phase_name,
            reviewer,
            attempt_number,
            ..
        } => Some(format!(
            "{} review passed (attempt {}) by {}",
            phase_name,
            attempt_number,
            format_agent(reviewer)
        )),
        ProgressEvent::ReviewFailed {
            phase_name,
            reviewer,
            attempt_number,
            reason_summary,
            ..
        } => Some(format!(
            "{} review failed (attempt {}) by {}: {}",
            phase_name,
            attempt_number,
            format_agent(reviewer),
            reason_summary
        )),
        ProgressEvent::RetryStarted {
            phase_name,
            attempt_number,
            ..
        } => Some(format!(
            "Retrying {} (attempt {})",
            phase_name, attempt_number
        )),
        ProgressEvent::PhaseCompleted {
            phase_name,
            phase_index,
            total_phases,
            ..
        } => Some(format!(
            "Phase {}/{} complete: {}",
            phase_index, total_phases, phase_name
        )),
        ProgressEvent::PhaseRestored {
            phase_name,
            phase_index,
            total_phases,
            ..
        } => Some(format!(
            "Phase {}/{} restored from checkpoint: {}",
            phase_index, total_phases, phase_name
        )),
        ProgressEvent::Transcript(_) => None,
    }
}

fn format_agent(agent: &AgentId) -> String {
    format!("{}/{}", agent.provider_name(), agent.model())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporter_renders_phase_position_task_and_agent() {
        let reporter = TerminalProgressReporter::new_for_test(true);
        reporter.on_event(ProgressEvent::PhaseStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            phase_index: 1,
            total_phases: 2,
            tasks: vec!["route".into(), "review".into()],
        });
        reporter.on_event(ProgressEvent::TaskStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            task_name: "route".into(),
            task_index: 1,
            total_tasks: 2,
            agent: AgentId::claude("opus-4"),
        });

        let snapshot = reporter.snapshot();
        assert!(snapshot.contains("Phase 1/2: foundation"));
        assert!(snapshot.contains("Active: route via Anthropic/opus-4"));
        assert!(snapshot.contains("[>] route - Anthropic/opus-4"));
    }

    #[test]
    fn reporter_keeps_review_failures_and_retries_as_durable_lines() {
        let reporter = TerminalProgressReporter::new_for_test(true);
        reporter.on_event(ProgressEvent::ReviewStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            reviewer: AgentId::gemini("2.5-pro"),
            attempt_number: 1,
        });
        reporter.on_event(ProgressEvent::ReviewFailed {
            phase_id: 1,
            phase_name: "foundation".into(),
            reviewer: AgentId::gemini("2.5-pro"),
            attempt_number: 1,
            reason_summary: "missing error handling".into(),
        });
        reporter.on_event(ProgressEvent::RetryStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            reviewer: AgentId::gemini("2.5-pro"),
            attempt_number: 2,
        });

        let milestones = reporter.milestone_lines();
        assert!(milestones
            .iter()
            .any(|line| line.contains("Review started")));
        assert!(milestones.iter().any(|line| line.contains("Review failed")));
        assert!(milestones
            .iter()
            .any(|line| line.contains("Retrying foundation")));
    }

    #[test]
    fn plain_mode_falls_back_to_plain_text_lines() {
        let reporter = TerminalProgressReporter::new_for_test(false);
        reporter.on_event(ProgressEvent::PhaseStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            phase_index: 1,
            total_phases: 2,
            tasks: vec!["route".into()],
        });
        reporter.on_event(ProgressEvent::TaskStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            task_name: "route".into(),
            task_index: 1,
            total_tasks: 1,
            agent: AgentId::claude("opus-4"),
        });

        let lines = reporter.plain_lines();
        assert!(lines.iter().any(|line| line.contains("Phase 1/2 started")));
        assert!(lines
            .iter()
            .any(|line| line.contains("running: route via Anthropic/opus-4")));
        assert!(lines.iter().all(|line| !line.contains('\u{1b}')));
    }
}
