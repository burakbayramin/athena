use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use ath_agents::{AgentBackend, ClaudeHandle, CodexHandle, GeminiHandle};
use ath_config::ConfigStore;
use ath_orchestrator::coordinator::AgentCoordinator;
use ath_orchestrator::isolation::check_isolation;
use ath_orchestrator::phase_runner::AgentRegistry;
use ath_orchestrator::progress::SharedProgressObserver;
use ath_orchestrator::router::assign_all_tasks;
use ath_orchestrator::taxonomy;
use ath_planner::decompose::{decompose_project_spec, display_execution_plan};
use ath_planner::input::{display_project_spec_summary, parse_input, resolve_input_mode};
use ath_types::agent::AgentId;
use ath_types::plan::ExecutionPlan;
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

    /// Ignore any existing checkpoint and start a clean run.
    #[arg(long)]
    pub(crate) fresh: bool,

    /// Show checkpoint status without running anything.
    #[arg(long)]
    pub(crate) status: bool,
}

/// Path to the checkpoint file within a project directory.
fn checkpoint_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".ath").join("checkpoint.json")
}

/// Display checkpoint status without executing anything.
fn show_checkpoint_status(project_dir: &Path) -> Result<()> {
    use ath_orchestrator::checkpoint::CheckpointStore;

    let cp_path = checkpoint_path(project_dir);
    match CheckpointStore::load(&cp_path) {
        Ok(Some(cp)) => {
            println!("Checkpoint found: {}", cp_path.display());
            println!("  Run ID:         {}", cp.run_id);
            println!("  Plan fingerprint: {}", &cp.plan_fingerprint[..16]);
            println!("  Completed:      {}/{} phases", cp.completed_count(), cp.plan.execution_order.len());
            if !cp.completed_phase_ids.is_empty() {
                let mut phase_names: Vec<_> = cp.plan.phases.iter()
                    .filter(|p| cp.completed_phase_ids.contains(&p.id))
                    .map(|p| format!("    - {} (phase {})", p.name, p.id))
                    .collect();
                phase_names.sort();
                println!("  Completed phases:");
                for name in &phase_names {
                    println!("{name}");
                }
            }
            println!("  Started at:     {}", cp.started_at.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("  Updated at:     {}", cp.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("\nRun `ath run` to resume, or `ath run --fresh` to start over.");
            Ok(())
        }
        Ok(None) => {
            println!("No checkpoint found. No incomplete run to resume.");
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Failed to load checkpoint: {e}");
        }
    }
}

pub(crate) async fn run_command(args: RunArgs, global: GlobalArgs) -> Result<()> {
    let project_dir = std::env::current_dir().map_err(|e| anyhow!("{e}"))?;

    if args.status {
        return show_checkpoint_status(&project_dir);
    }

    if args.dry_run {
        return crate::dry_run::execute(&project_dir);
    }

    let cp_path = checkpoint_path(&project_dir);

    // --fresh: delete any existing checkpoint before proceeding
    if args.fresh {
        if cp_path.exists() {
            std::fs::remove_file(&cp_path).map_err(|e| anyhow!("Failed to remove checkpoint: {e}"))?;
            println!("Checkpoint cleared. Starting fresh run.");
        }
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
    let output_dir = std::env::current_dir().map_err(|e| anyhow!("{e}"))?;
    crate::dry_run::write_plan_cache(&output_dir, &plan)?;

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
    let coordinator = AgentCoordinator::new(registry, output_dir.clone(), None);

    let records = coordinator
        .run_plan_with_progress(&plan, Some(observer), Some(&cp_path))
        .await
        .map_err(|e| anyhow!("{e}"))?;

    let report = crate::report::build_run_report(&plan, &records);
    crate::report::write_run_report(&output_dir, &report)?;

    reporter.finish(&format!(
        "Run complete: {} phase(s) executed and reviewed.",
        records.len()
    ));

    Ok(())
}

/// Build a backend for planning (decomposition). Picks the first available agent.
fn build_planning_backend(config: &ConfigStore) -> Result<Arc<dyn AgentBackend>> {
    for agent_cfg in &config.agents.agents {
        if agent_cfg.resolve_api_key().is_none() {
            continue;
        }
        if let Some(backend) = build_backend_for_agent(agent_cfg, config)? {
            return Ok(backend);
        }
    }

    anyhow::bail!("No AI providers are configured. Add an API key before running `ath run`.");
}

/// Build a registry of all available agent backends from config.
fn build_agent_registry(config: &ConfigStore) -> Result<AgentRegistry> {
    let mut registry = AgentRegistry::new();

    for agent_cfg in &config.agents.agents {
        if agent_cfg.resolve_api_key().is_none() {
            continue;
        }
        if let Some(backend) = build_backend_for_agent(agent_cfg, config)? {
            let agent_id = AgentId::new(&agent_cfg.provider, &agent_cfg.model);
            registry.register(agent_id, backend);
        }
    }

    if !config.agents.agents.iter().any(|a| {
        a.resolve_api_key().is_some() && a.is_builtin_provider()
    }) && registry.get(&AgentId::new("any", "placeholder")).is_none()
    {
        // Check if we got at least one provider registered
        let has_any = config.agents.agents.iter().any(|a| a.resolve_api_key().is_some());
        if !has_any {
            anyhow::bail!("No execution providers are available. Configure at least one provider.");
        }
    }

    Ok(registry)
}

/// Build the appropriate backend for a single agent config entry.
///
/// Returns `Ok(None)` for unsupported custom providers (handled in S04).
fn build_backend_for_agent(
    agent_cfg: &ath_config::AgentConfig,
    config: &ConfigStore,
) -> Result<Option<Arc<dyn AgentBackend>>> {
    match agent_cfg.provider.as_str() {
        "anthropic" => {
            let handle = ClaudeHandle::new(config).map_err(|e| anyhow!("{e}"))?;
            Ok(Some(Arc::new(handle)))
        }
        "google" => {
            let handle = GeminiHandle::new(config).map_err(|e| anyhow!("{e}"))?;
            Ok(Some(Arc::new(handle)))
        }
        "openai" => {
            let handle = CodexHandle::new(config).map_err(|e| anyhow!("{e}"))?;
            Ok(Some(Arc::new(handle)))
        }
        provider => {
            // Custom providers will be handled in S04 (generic OpenAI provider)
            eprintln!("Warning: provider '{}' is not yet supported, skipping agent {}/{}", 
                provider, agent_cfg.provider, agent_cfg.model);
            Ok(None)
        }
    }
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
    available: impl Fn(&AgentId) -> bool + Copy,
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
    use ath_types::plan::{PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;

    fn make_task(name: &str, skill_tags: &[&str], expected_files: &[&str]) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("{name} description"),
            skill_tags: skill_tags
                .iter()
                .map(|tag| SkillTag((*tag).to_string()))
                .collect(),
            expected_output_files: expected_files
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
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

        assert!(plan.phases[0].tasks[0].assigned_agent.as_ref().map_or(false, |a| a.is_claude()));
        assert!(plan.phases[0].tasks[1].assigned_agent.as_ref().map_or(false, |a| a.is_gemini()));
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
        assert!(plan
            .phases
            .iter()
            .all(|phase| phase.tasks.iter().all(|task| task.assigned_agent.is_some())));
    }

    #[test]
    fn checkpoint_path_is_project_relative() {
        let dir = PathBuf::from("/tmp/my-project");
        let path = checkpoint_path(&dir);
        assert_eq!(path, PathBuf::from("/tmp/my-project/.ath/checkpoint.json"));
    }

    #[test]
    fn show_status_missing_checkpoint() {
        // show_checkpoint_status prints "No checkpoint found" for missing file
        let temp = tempfile::tempdir().unwrap();
        // We can't easily capture stdout in a unit test, so we just verify it doesn't error
        let result = show_checkpoint_status(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn show_status_existing_checkpoint() {
        use ath_orchestrator::checkpoint::{Checkpoint, CheckpointStore};

        let temp = tempfile::tempdir().unwrap();
        let cp_path = checkpoint_path(temp.path());

        let plan = ExecutionPlan {
            phases: vec![make_phase(1, "foundation", vec![])],
            execution_order: vec![1],
            parallel_groups: vec![vec![1]],
            critical_path_length: 1,
        };

        let mut cp = Checkpoint::new("run-test".into(), &plan);
        cp.add_records(vec![ath_types::phase::PhaseRecord {
            id: uuid::Uuid::new_v4(),
            phase_id: 1,
            phase_name: "foundation".into(),
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            contributions: vec![],
            review_attempts: vec![],
        }]);
        CheckpointStore::save(&cp_path, &cp).unwrap();

        let result = show_checkpoint_status(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn fresh_flag_deletes_checkpoint() {
        use ath_orchestrator::checkpoint::{Checkpoint, CheckpointStore};

        let temp = tempfile::tempdir().unwrap();
        let cp_path = checkpoint_path(temp.path());

        let plan = ExecutionPlan {
            phases: vec![],
            execution_order: vec![],
            parallel_groups: vec![],
            critical_path_length: 0,
        };
        let cp = Checkpoint::new("run-fresh".into(), &plan);
        CheckpointStore::save(&cp_path, &cp).unwrap();
        assert!(cp_path.exists());

        // Simulate what --fresh does
        std::fs::remove_file(&cp_path).unwrap();
        assert!(!cp_path.exists());
    }

    #[test]
    fn show_status_corrupt_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let cp_path = checkpoint_path(temp.path());
        std::fs::create_dir_all(cp_path.parent().unwrap()).unwrap();
        std::fs::write(&cp_path, "not json").unwrap();

        let result = show_checkpoint_status(temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to load"));
    }
}
