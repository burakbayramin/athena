use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use ath_planner::decompose::display_execution_plan;
use ath_types::plan::ExecutionPlan;

const CACHE_DIR: &str = ".ath";
const CACHE_FILE: &str = "last-plan.json";

pub(crate) fn plan_cache_path(project_dir: &Path) -> PathBuf {
    project_dir.join(CACHE_DIR).join(CACHE_FILE)
}

pub(crate) fn write_plan_cache(project_dir: &Path, plan: &ExecutionPlan) -> Result<PathBuf> {
    let path = plan_cache_path(project_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| anyhow!("{e}"))?;
    }

    let json = serde_json::to_string_pretty(plan).map_err(|e| anyhow!("{e}"))?;
    std::fs::write(&path, json).map_err(|e| anyhow!("{e}"))?;
    Ok(path)
}

pub(crate) fn load_plan_cache(project_dir: &Path) -> Result<ExecutionPlan> {
    let path = plan_cache_path(project_dir);
    let raw = std::fs::read_to_string(&path).map_err(|_| {
        anyhow!(
            "`ath run --dry-run` requires a previously generated local plan at {}. Run `ath run` once first.",
            path.display()
        )
    })?;

    serde_json::from_str(&raw).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
pub(crate) fn render_cached_plan(project_dir: &Path) -> Result<String> {
    use ath_planner::decompose::display::format_execution_plan;
    let plan = load_plan_cache(project_dir)?;
    let mut out = String::new();
    format_execution_plan(&plan, &[], &mut out).map_err(|e| anyhow!("{e}"))?;
    Ok(out)
}

pub(crate) fn execute(project_dir: &Path) -> Result<()> {
    let plan = load_plan_cache(project_dir)?;
    display_execution_plan(&plan, &[]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{run, GlobalArgs};
    use ath_types::agent::AgentKind;
    use ath_types::plan::{PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;
    use std::sync::Mutex;

    static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

    struct CurrentDirReset(std::path::PathBuf);

    impl Drop for CurrentDirReset {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).expect("restore current dir");
        }
    }

    fn make_task(name: &str, agent: AgentKind) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("{name} description"),
            skill_tags: vec![SkillTag("rust".into())],
            expected_output_files: vec!["src/main.rs".into()],
            acceptance_criteria: vec![],
            goal_indices: vec![0],
            assigned_agent: Some(agent),
        }
    }

    fn make_plan() -> ExecutionPlan {
        ExecutionPlan {
            phases: vec![PhaseSpec {
                id: 1,
                name: "Foundation".into(),
                description: "Set up the project".into(),
                tasks: vec![make_task("Bootstrap", AgentKind::Claude("opus-4".into()))],
                depends_on: vec![],
                produces: vec!["project-structure".into()],
                consumes: vec![],
            }],
            execution_order: vec![1],
            parallel_groups: vec![vec![1]],
            critical_path_length: 1,
        }
    }

    #[test]
    fn plan_cache_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let plan = make_plan();

        let path = write_plan_cache(temp.path(), &plan).expect("cache written");
        assert_eq!(path, plan_cache_path(temp.path()));

        let loaded = load_plan_cache(temp.path()).expect("cache loaded");
        assert_eq!(loaded, plan);
    }

    #[test]
    fn dry_run_requires_local_plan() {
        let temp = tempfile::tempdir().unwrap();
        let err = load_plan_cache(temp.path()).unwrap_err();
        assert!(err.to_string().contains("previously generated local plan"));
        assert!(err.to_string().contains(".ath"));
    }

    #[test]
    fn cached_plan_output_includes_assigned_agents() {
        let temp = tempfile::tempdir().unwrap();
        write_plan_cache(temp.path(), &make_plan()).expect("cache written");

        let output = render_cached_plan(temp.path()).expect("rendered");
        assert!(output.contains("Assigned agents:"));
        assert!(output.contains("Bootstrap -> Anthropic/opus-4"));
    }

    #[tokio::test]
    async fn dry_run_uses_cached_plan_without_constructing_backends() {
        let _guard = CURRENT_DIR_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let previous_dir = std::env::current_dir().unwrap();
        let _reset = CurrentDirReset(previous_dir);
        std::env::set_current_dir(temp.path()).unwrap();

        write_plan_cache(temp.path(), &make_plan()).expect("cache written");

        let args = run::RunArgs {
            description: None,
            spec: None,
            codebase: None,
            dry_run: true,
        };

        let result = run::run_command(
            args,
            GlobalArgs {
                verbose: false,
                no_color: true,
            },
        )
        .await;

        assert!(
            result.is_ok(),
            "dry-run should not need configured providers"
        );
    }

    #[tokio::test]
    async fn dry_run_missing_cache_fails_before_constructing_backends() {
        let _guard = CURRENT_DIR_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let previous_dir = std::env::current_dir().unwrap();
        let _reset = CurrentDirReset(previous_dir);
        std::env::set_current_dir(temp.path()).unwrap();

        let args = run::RunArgs {
            description: Some("build something".into()),
            spec: None,
            codebase: None,
            dry_run: true,
        };

        let err = run::run_command(
            args,
            GlobalArgs {
                verbose: false,
                no_color: true,
            },
        )
        .await
        .expect_err("dry-run should fail when no cached plan exists");

        let message = err.to_string();
        assert!(
            message.contains("previously generated local plan"),
            "expected missing-cache guidance, got: {message}"
        );
        assert!(
            message.contains(".ath"),
            "expected cache path hint, got: {message}"
        );
    }
}
