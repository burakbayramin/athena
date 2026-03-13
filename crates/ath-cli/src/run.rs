use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use ath_agents::{AgentBackend, ClaudeHandle, CodexHandle, GeminiHandle};
use ath_config::ConfigStore;
use ath_planner::decompose::{decompose_project_spec, display_execution_plan};
use ath_planner::input::{display_project_spec_summary, parse_input, resolve_input_mode};
use ath_orchestrator::coordinator::AgentCoordinator;
use ath_orchestrator::isolation::check_isolation;
use ath_orchestrator::phase_runner::AgentRegistry;
use ath_orchestrator::progress::SharedProgressObserver;
use ath_orchestrator::router::assign_all_tasks;
use ath_orchestrator::taxonomy;
use ath_types::agent::AgentKind;
use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};
use clap::Args;

use crate::progress::TerminalProgressReporter;
use crate::verbose::{CliObserver, VerboseTranscriptSink};
use crate::GlobalArgs;

#[derive(Debug, Args, Clone, PartialEq, Eq)]
#[command(about = "Conduct a multi-agent run on the current project.")]
pub(crate) struct RunArgs {
    /// Project description in natural language.
    #[arg(value_name = "DESCRIPTION")]
    pub(crate) description: Option<String>,

    /// Path to a markdown spec file.
    #[arg(long, value_name = "FILE")]
    pub(crate) spec: Option<PathBuf>,

    /// Path to an existing codebase to analyze.
    #[arg(long, value_name = "DIR")]
    pub(crate) codebase: Option<PathBuf>,

    /// Show the locally available execution plan without running agents.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

pub(crate) async fn run_command(args: RunArgs, global: GlobalArgs) -> Result<()> {
    if args.dry_run {
        anyhow::bail!("{}", dry_run_placeholder_message());
    }

    let config = ConfigStore::load().map_err(anyhow::Error::new)?;

    if global.verbose {
        print_provider_status(&config);
    }

    let mode = resolve_input_mode(
        args.description.as_deref(),
        args.spec.as_deref(),
        args.codebase.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;

    let backend = build_planning_backend(&config)?;

    let project_spec = parse_input(mode, backend.as_ref())
        .await
        .map_err(|e| anyhow!("{e}"))?;

    display_project_spec_summary(&project_spec);

    let (plan, warnings) = decompose_project_spec(&project_spec, backend.as_ref())
        .await
        .map_err(|e| anyhow!("{e}"))?;

    let mut plan = plan;
    let registry = build_agent_registry(&config)?;
    assign_agents_and_check_isolation(&mut plan, |kind| registry.get(kind).is_some())?;

    display_execution_plan(&plan, &warnings);

    let reporter = Arc::new(TerminalProgressReporter::new(global));
    let verbose_sink = if global.verbose {
        Some(Arc::new(VerboseTranscriptSink::new(
            reporter.clone(),
            configured_secrets(&config),
        )))
    } else {
        None
    };
    let observer: SharedProgressObserver =
        Arc::new(CliObserver::new(reporter.clone(), verbose_sink));
    let output_dir = std::env::current_dir().map_err(|e| anyhow!("{e}"))?;
    let coordinator = AgentCoordinator::new(registry, output_dir, None);

    let records = coordinator
        .run_plan_with_progress(&plan, Some(observer))
        .await
        .map_err(|e| anyhow!("{e}"))?;

    reporter.finish(&format!(
        "Run complete: {} phase(s) executed and reviewed.",
        records.len()
    ));

    Ok(())
}

pub(crate) fn dry_run_placeholder_message() -> &'static str {
    "`ath run --dry-run` is recognized, but the local no-cost plan preview is not wired yet."
}

fn build_planning_backend(config: &ConfigStore) -> Result<Arc<dyn AgentBackend>> {
    if config.anthropic_api_key.is_some() {
        return ClaudeHandle::new(config)
            .map(|handle| Arc::new(handle) as Arc<dyn AgentBackend>)
            .map_err(|e| anyhow!("{e}"));
    }

    if config.google_api_key.is_some() {
        return GeminiHandle::new(config)
            .map(|handle| Arc::new(handle) as Arc<dyn AgentBackend>)
            .map_err(|e| anyhow!("{e}"));
    }

    if config.openai_api_key.is_some() {
        return CodexHandle::new(config)
            .map(|handle| Arc::new(handle) as Arc<dyn AgentBackend>)
            .map_err(|e| anyhow!("{e}"));
    }

    anyhow::bail!("No AI providers are configured. Add an API key before running `ath run`.");
}

fn build_agent_registry(config: &ConfigStore) -> Result<AgentRegistry> {
    let mut registry = AgentRegistry::new();

    if config.anthropic_api_key.is_some() {
        let handle = ClaudeHandle::new(config).map_err(|e| anyhow!("{e}"))?;
        registry.register(
            AgentKind::Claude(config.claude_model.clone()),
            Arc::new(handle),
        );
    }

    if config.google_api_key.is_some() {
        let handle = GeminiHandle::new(config).map_err(|e| anyhow!("{e}"))?;
        registry.register(
            AgentKind::Gemini(config.gemini_model.clone()),
            Arc::new(handle),
        );
    }

    if config.openai_api_key.is_some() {
        let handle = CodexHandle::new(config).map_err(|e| anyhow!("{e}"))?;
        registry.register(
            AgentKind::Codex(config.codex_model.clone()),
            Arc::new(handle),
        );
    }

    if registry.get(&AgentKind::Claude("placeholder".into())).is_none()
        && registry.get(&AgentKind::Gemini("placeholder".into())).is_none()
        && registry.get(&AgentKind::Codex("placeholder".into())).is_none()
    {
        anyhow::bail!("No execution providers are available. Configure at least one provider.");
    }

    Ok(registry)
}

fn configured_secrets(config: &ConfigStore) -> Vec<String> {
    [
        config.anthropic_api_key.clone(),
        config.google_api_key.clone(),
        config.openai_api_key.clone(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) fn assign_agents_and_check_isolation(
    plan: &mut ExecutionPlan,
    available: impl Fn(&AgentKind) -> bool + Copy,
) -> Result<()> {
    let table = taxonomy::build_routing_table();

    for phase in &mut plan.phases {
        assign_all_tasks(phase, &table, available).map_err(|e| anyhow!("{e}"))?;
    }

    check_isolation(plan).map_err(|e| anyhow!("{e}"))?;

    Ok(())
}

fn print_provider_status(config: &ConfigStore) {
    let status = |configured: bool| -> &str {
        if configured {
            "configured"
        } else {
            "not configured"
        }
    };

    println!(
        "Providers: Anthropic ({}), Google ({}), OpenAI ({})",
        status(config.anthropic_api_key.is_some()),
        status(config.google_api_key.is_some()),
        status(config.openai_api_key.is_some()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::project::SkillTag;

    fn make_task(name: &str, skill_tags: &[&str], expected_files: &[&str]) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("{name} description"),
            skill_tags: skill_tags
                .iter()
                .map(|tag| SkillTag((*tag).to_string()))
                .collect(),
            expected_output_files: expected_files.iter().map(|path| (*path).to_string()).collect(),
            acceptance_criteria: vec![],
            goal_indices: vec![],
            assigned_agent: None,
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

    #[test]
    fn routes_tasks_before_execution() {
        let mut plan = ExecutionPlan {
            phases: vec![make_phase(
                1,
                "phase-1",
                vec![
                    make_task("reasoning", &["rust"], &["src/lib.rs"]),
                    make_task("docs", &["docs"], &["README.md"]),
                ],
            )],
            execution_order: vec![1],
            parallel_groups: vec![vec![1]],
            critical_path_length: 1,
        };

        assign_agents_and_check_isolation(&mut plan, |_| true).expect("routing succeeds");

        assert!(matches!(
            plan.phases[0].tasks[0].assigned_agent,
            Some(AgentKind::Claude(_))
        ));
        assert!(matches!(
            plan.phases[0].tasks[1].assigned_agent,
            Some(AgentKind::Gemini(_))
        ));
    }

    #[test]
    fn checks_isolation_before_execution() {
        let mut plan = ExecutionPlan {
            phases: vec![
                make_phase(
                    1,
                    "phase-1",
                    vec![make_task("task-a", &["rust"], &["src/main.rs"])],
                ),
                make_phase(
                    2,
                    "phase-2",
                    vec![make_task("task-b", &["docs"], &["src/main.rs"])],
                ),
            ],
            execution_order: vec![1, 2],
            parallel_groups: vec![vec![1, 2]],
            critical_path_length: 1,
        };

        let err = assign_agents_and_check_isolation(&mut plan, |_| true).unwrap_err();
        assert!(
            err.to_string().contains("src/main.rs"),
            "isolation error should mention the conflicting file"
        );
        assert!(plan.phases.iter().all(|phase| phase
            .tasks
            .iter()
            .all(|task| task.assigned_agent.is_some())));
    }
}
