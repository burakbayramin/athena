//! Terminal display formatting for execution plans.
//!
//! Provides human-readable output of the decomposed execution plan,
//! including phase details, dependency information, parallel groups,
//! and non-blocking warnings.

use colored::Colorize;

use ath_types::plan::ExecutionPlan;
use super::validate::PlanWarning;

/// Format an execution plan into a writable buffer (for testability).
///
/// Writes phase details, dependency info, parallel groups, and warnings
/// without any ANSI color codes (plain text).
pub fn format_execution_plan(
    plan: &ExecutionPlan,
    warnings: &[PlanWarning],
    out: &mut impl std::fmt::Write,
) -> std::fmt::Result {
    // Header
    writeln!(out, "Execution Plan")?;
    writeln!(
        out,
        "{} phases, {} parallel groups, critical path: {}",
        plan.phases.len(),
        plan.parallel_groups.len(),
        plan.critical_path_length,
    )?;
    writeln!(out)?;

    // Phase details in execution order
    for &phase_id in &plan.execution_order {
        let Some(phase) = plan.phases.iter().find(|p| p.id == phase_id) else {
            continue;
        };

        writeln!(out, "Phase {}: {}", phase.id, phase.name)?;
        writeln!(out, "  {}", phase.description)?;

        // Tasks
        let task_names: Vec<&str> = phase.tasks.iter().map(|t| t.name.as_str()).collect();
        writeln!(out, "  Tasks: {} ({})", phase.tasks.len(), task_names.join(", "))?;

        // Dependencies
        if phase.depends_on.is_empty() {
            writeln!(out, "  Depends on: none")?;
        } else {
            let deps: Vec<String> = phase.depends_on.iter().map(|d| d.to_string()).collect();
            writeln!(out, "  Depends on: {}", deps.join(", "))?;
        }

        // Produces
        if phase.produces.is_empty() {
            writeln!(out, "  Produces: none")?;
        } else {
            writeln!(out, "  Produces: {}", phase.produces.join(", "))?;
        }

        // Consumes
        if phase.consumes.is_empty() {
            writeln!(out, "  Consumes: none")?;
        } else {
            writeln!(out, "  Consumes: {}", phase.consumes.join(", "))?;
        }

        writeln!(out)?;
    }

    // Parallel groups
    writeln!(out, "Parallel Groups:")?;
    for (i, group) in plan.parallel_groups.iter().enumerate() {
        let ids: Vec<String> = group.iter().map(|id| id.to_string()).collect();
        if group.len() > 1 {
            writeln!(out, "  Wave {}: phases {} [parallel]", i + 1, ids.join(", "))?;
        } else {
            writeln!(out, "  Wave {}: phases {}", i + 1, ids.join(", "))?;
        }
    }

    // Warnings
    if !warnings.is_empty() {
        writeln!(out)?;
        writeln!(out, "Warnings:")?;
        for warning in warnings {
            match warning {
                PlanWarning::LargePhase {
                    phase_id,
                    phase_name,
                    task_count,
                } => {
                    writeln!(
                        out,
                        "  Phase {} '{}' has {} tasks (recommended: 2-5)",
                        phase_id, phase_name, task_count,
                    )?;
                }
                PlanWarning::LongCriticalPath { length } => {
                    writeln!(
                        out,
                        "  Critical path length is {} (consider reducing dependencies)",
                        length,
                    )?;
                }
            }
        }
    }

    Ok(())
}

/// Display the execution plan to stdout with colored output.
///
/// Uses `format_execution_plan` internally, then prints with ANSI colors applied
/// via the `colored` crate.
pub fn display_execution_plan(plan: &ExecutionPlan, warnings: &[PlanWarning]) {
    // Header
    println!();
    println!("{}", "Execution Plan".bold());
    println!(
        "{} phases, {} parallel groups, critical path: {}",
        plan.phases.len(),
        plan.parallel_groups.len(),
        plan.critical_path_length,
    );
    println!();

    // Phase details
    for &phase_id in &plan.execution_order {
        let Some(phase) = plan.phases.iter().find(|p| p.id == phase_id) else {
            continue;
        };

        println!("{}", format!("Phase {}: {}", phase.id, phase.name).bold());
        println!("  {}", phase.description.dimmed());

        let task_names: Vec<&str> = phase.tasks.iter().map(|t| t.name.as_str()).collect();
        println!("  Tasks: {} ({})", phase.tasks.len(), task_names.join(", "));

        if phase.depends_on.is_empty() {
            println!("  Depends on: {}", "none".dimmed());
        } else {
            let deps: Vec<String> = phase.depends_on.iter().map(|d| d.to_string()).collect();
            println!("  Depends on: {}", deps.join(", "));
        }

        if phase.produces.is_empty() {
            println!("  Produces: {}", "none".dimmed());
        } else {
            println!("  Produces: {}", phase.produces.join(", "));
        }

        if phase.consumes.is_empty() {
            println!("  Consumes: {}", "none".dimmed());
        } else {
            println!("  Consumes: {}", phase.consumes.join(", "));
        }

        println!();
    }

    // Parallel groups
    println!("{}", "Parallel Groups:".bold());
    for (i, group) in plan.parallel_groups.iter().enumerate() {
        let ids: Vec<String> = group.iter().map(|id| id.to_string()).collect();
        if group.len() > 1 {
            println!(
                "  Wave {}: phases {} {}",
                i + 1,
                ids.join(", "),
                "[parallel]".green(),
            );
        } else {
            println!("  Wave {}: phases {}", i + 1, ids.join(", "));
        }
    }

    // Warnings
    if !warnings.is_empty() {
        println!();
        println!("{}", "Warnings:".yellow().bold());
        for warning in warnings {
            match warning {
                PlanWarning::LargePhase {
                    phase_id,
                    phase_name,
                    task_count,
                } => {
                    println!(
                        "  {}",
                        format!(
                            "Phase {} '{}' has {} tasks (recommended: 2-5)",
                            phase_id, phase_name, task_count,
                        )
                        .yellow(),
                    );
                }
                PlanWarning::LongCriticalPath { length } => {
                    println!(
                        "  {}",
                        format!(
                            "Critical path length is {} (consider reducing dependencies)",
                            length,
                        )
                        .yellow(),
                    );
                }
            }
        }
    }
}

/// Print warnings to stderr with yellow coloring.
///
/// Called separately from main display when warnings need to go to stderr
/// while the plan display goes to stdout.
pub fn display_warnings_stderr(warnings: &[PlanWarning]) {
    if warnings.is_empty() {
        return;
    }

    eprintln!("{}", "Warnings:".yellow().bold());
    for warning in warnings {
        match warning {
            PlanWarning::LargePhase {
                phase_id,
                phase_name,
                task_count,
            } => {
                eprintln!(
                    "  {}",
                    format!(
                        "Phase {} '{}' has {} tasks (recommended: 2-5)",
                        phase_id, phase_name, task_count,
                    )
                    .yellow(),
                );
            }
            PlanWarning::LongCriticalPath { length } => {
                eprintln!(
                    "  {}",
                    format!(
                        "Critical path length is {} (consider reducing dependencies)",
                        length,
                    )
                    .yellow(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;

    fn make_task(name: &str) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("{} description", name),
            skill_tags: vec![SkillTag("rust".into())],
            expected_output_files: vec![],
            acceptance_criteria: vec![],
            goal_indices: vec![0],
        }
    }

    fn make_test_plan() -> ExecutionPlan {
        ExecutionPlan {
            phases: vec![
                PhaseSpec {
                    id: 1,
                    name: "Foundation".into(),
                    description: "Set up project structure".into(),
                    tasks: vec![make_task("Init"), make_task("Config")],
                    depends_on: vec![],
                    produces: vec!["project-structure".into()],
                    consumes: vec![],
                },
                PhaseSpec {
                    id: 2,
                    name: "API Layer".into(),
                    description: "Build REST endpoints".into(),
                    tasks: vec![make_task("Endpoints")],
                    depends_on: vec![1],
                    produces: vec!["api-types".into()],
                    consumes: vec!["project-structure".into()],
                },
                PhaseSpec {
                    id: 3,
                    name: "Auth".into(),
                    description: "Add authentication".into(),
                    tasks: vec![make_task("JWT")],
                    depends_on: vec![1],
                    produces: vec![],
                    consumes: vec!["project-structure".into()],
                },
            ],
            execution_order: vec![1, 2, 3],
            parallel_groups: vec![vec![1], vec![2, 3]],
            critical_path_length: 2,
        }
    }

    #[test]
    fn output_contains_phase_ids() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Phase 1:"), "should contain Phase 1:");
        assert!(out.contains("Phase 2:"), "should contain Phase 2:");
        assert!(out.contains("Phase 3:"), "should contain Phase 3:");
    }

    #[test]
    fn output_contains_phase_names() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Foundation"), "should contain Foundation");
        assert!(out.contains("API Layer"), "should contain API Layer");
        assert!(out.contains("Auth"), "should contain Auth");
    }

    #[test]
    fn output_contains_depends_on_labels() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Depends on: none"), "Phase 1 should show 'Depends on: none'");
        assert!(out.contains("Depends on: 1"), "Phase 2 should show 'Depends on: 1'");
    }

    #[test]
    fn output_contains_wave_indicators() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Wave 1:"), "should contain Wave 1:");
        assert!(out.contains("Wave 2:"), "should contain Wave 2:");
        assert!(out.contains("[parallel]"), "multi-phase group should show [parallel]");
    }

    #[test]
    fn output_contains_task_info() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Tasks: 2"), "Phase 1 should show 2 tasks");
        assert!(out.contains("Init, Config"), "Phase 1 should list task names");
    }

    #[test]
    fn output_contains_contract_info() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("Produces: project-structure"), "Phase 1 produces project-structure");
        assert!(out.contains("Consumes: project-structure"), "Phase 2 consumes project-structure");
    }

    #[test]
    fn output_contains_warnings_when_present() {
        let plan = make_test_plan();
        let warnings = vec![
            PlanWarning::LargePhase {
                phase_id: 1,
                phase_name: "Big Phase".into(),
                task_count: 8,
            },
            PlanWarning::LongCriticalPath { length: 10 },
        ];

        let mut out = String::new();
        format_execution_plan(&plan, &warnings, &mut out).unwrap();

        assert!(out.contains("Warnings:"), "should contain Warnings section");
        assert!(
            out.contains("Phase 1 'Big Phase' has 8 tasks (recommended: 2-5)"),
            "should contain LargePhase warning text",
        );
        assert!(
            out.contains("Critical path length is 10 (consider reducing dependencies)"),
            "should contain LongCriticalPath warning text",
        );
    }

    #[test]
    fn output_omits_warnings_section_when_empty() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(!out.contains("Warnings:"), "should not contain Warnings section when empty");
    }

    #[test]
    fn output_contains_plan_stats() {
        let plan = make_test_plan();
        let mut out = String::new();
        format_execution_plan(&plan, &[], &mut out).unwrap();

        assert!(out.contains("3 phases"), "header should show phase count");
        assert!(out.contains("2 parallel groups"), "header should show group count");
        assert!(out.contains("critical path: 2"), "header should show critical path length");
    }
}
