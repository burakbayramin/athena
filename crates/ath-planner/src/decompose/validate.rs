//! DAG structural validation for decomposed phase plans.
//!
//! Validates the phase graph for structural correctness (hard errors)
//! and generates non-blocking warnings for suspicious patterns.

use std::collections::{HashMap, HashSet, VecDeque};

use ath_types::plan::PhaseSpec;
use ath_types::project::ProjectSpec;
use ath_types::ValidationError;

use super::dag::{critical_path_length, topological_sort};

/// Non-blocking warnings about the plan structure.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanWarning {
    /// A phase has an unusually large number of tasks (>5).
    LargePhase {
        phase_id: u32,
        phase_name: String,
        task_count: usize,
    },
    /// The critical path through the DAG is unusually long (>7).
    LongCriticalPath { length: usize },
}

/// Validate a decomposed phase plan for structural correctness.
///
/// Returns `Ok(warnings)` if the plan is structurally valid (warnings are non-blocking),
/// or `Err(errors)` if hard errors are found (circular deps, orphaned goals, etc.).
///
/// Hard errors checked:
/// 1. Empty phases (zero tasks)
/// 2. Missing dependency targets (depends_on references non-existent phase)
/// 3. Circular dependencies (via topological sort)
/// 4. Orphaned goals (project goal with no task mapping)
/// 5. Unsatisfied contracts (consumed contract not produced by transitive dependency)
pub fn validate_plan(
    phases: &[PhaseSpec],
    project: &ProjectSpec,
) -> Result<Vec<PlanWarning>, Vec<ValidationError>> {
    let mut errors: Vec<ValidationError> = Vec::new();
    let phase_ids: HashSet<u32> = phases.iter().map(|p| p.id).collect();

    // 1. Empty phases
    for phase in phases {
        if phase.tasks.is_empty() {
            errors.push(ValidationError::EmptyPhase {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                hint: "Add at least one task to the phase".into(),
            });
        }
    }

    // 2. Missing dependency targets
    for phase in phases {
        for &dep_id in &phase.depends_on {
            if !phase_ids.contains(&dep_id) {
                errors.push(ValidationError::MissingDependencyTarget {
                    phase_id: phase.id,
                    missing_id: dep_id,
                    hint: "Check that depends_on references valid phase IDs".into(),
                });
            }
        }
    }

    // 3. Circular dependencies
    if let Err(cycle) = topological_sort(phases) {
        let cycle_str = cycle
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        let cycle_path = if !cycle.is_empty() {
            format!("{} -> {}", cycle_str, cycle[0])
        } else {
            cycle_str
        };
        errors.push(ValidationError::CircularDependency {
            cycle_path,
            hint: "Remove one dependency edge to break the cycle".into(),
        });
    }

    // 4. Orphaned goals
    let mut covered_goals: HashSet<usize> = HashSet::new();
    for phase in phases {
        for task in &phase.tasks {
            for &gi in &task.goal_indices {
                covered_goals.insert(gi);
            }
        }
    }
    for (i, goal) in project.goals.iter().enumerate() {
        if !covered_goals.contains(&i) {
            errors.push(ValidationError::OrphanedGoal {
                goal_index: i,
                goal_description: goal.description.clone(),
                hint: "Add a task that maps to this goal via goal_indices".into(),
            });
        }
    }

    // 5. Unsatisfied contracts (transitive closure check)
    // Build a map of what each phase produces
    let produces_map: HashMap<u32, HashSet<&str>> = phases
        .iter()
        .map(|p| {
            (
                p.id,
                p.produces.iter().map(|c| c.as_str()).collect::<HashSet<_>>(),
            )
        })
        .collect();

    // Build adjacency for BFS (depends_on edges)
    let deps_map: HashMap<u32, &Vec<u32>> = phases.iter().map(|p| (p.id, &p.depends_on)).collect();

    for phase in phases {
        if phase.consumes.is_empty() {
            continue;
        }

        // Compute transitive dependency closure via BFS
        let mut transitive_produces: HashSet<&str> = HashSet::new();
        let mut visited: HashSet<u32> = HashSet::new();
        let mut queue: VecDeque<u32> = VecDeque::new();

        for &dep_id in &phase.depends_on {
            if phase_ids.contains(&dep_id) && visited.insert(dep_id) {
                queue.push_back(dep_id);
            }
        }

        while let Some(node) = queue.pop_front() {
            if let Some(produced) = produces_map.get(&node) {
                transitive_produces.extend(produced);
            }
            if let Some(node_deps) = deps_map.get(&node) {
                for &d in *node_deps {
                    if phase_ids.contains(&d) && visited.insert(d) {
                        queue.push_back(d);
                    }
                }
            }
        }

        for contract in &phase.consumes {
            if !transitive_produces.contains(contract.as_str()) {
                errors.push(ValidationError::UnsatisfiedContract {
                    phase_id: phase.id,
                    contract: contract.clone(),
                    hint: format!(
                        "Add a dependency on the phase that produces '{}'",
                        contract
                    ),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // Collect warnings (non-blocking)
    let mut warnings: Vec<PlanWarning> = Vec::new();

    // Large phases (>5 tasks)
    for phase in phases {
        if phase.tasks.len() > 5 {
            warnings.push(PlanWarning::LargePhase {
                phase_id: phase.id,
                phase_name: phase.name.clone(),
                task_count: phase.tasks.len(),
            });
        }
    }

    // Long critical path (>7)
    if let Ok(sorted) = topological_sort(phases) {
        let cpl = critical_path_length(phases, &sorted);
        if cpl > 7 {
            warnings.push(PlanWarning::LongCriticalPath { length: cpl });
        }
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::plan::TaskSpec;
    use ath_types::project::{GoalSpec, SkillTag};

    fn make_project(goal_count: usize) -> ProjectSpec {
        ProjectSpec {
            name: "test-project".into(),
            description: "A test project".into(),
            goals: (0..goal_count)
                .map(|i| GoalSpec {
                    description: format!("Goal {}", i),
                    skill_tags: vec![SkillTag("test".into())],
                })
                .collect(),
            constraints: vec![],
            target_language: Some("Rust".into()),
            target_framework: None,
            expected_files: vec!["src/main.rs".into()],
        }
    }

    fn make_task(goal_indices: Vec<usize>) -> TaskSpec {
        TaskSpec {
            name: "task".into(),
            description: "a task".into(),
            skill_tags: vec![SkillTag("test".into())],
            expected_output_files: vec![],
            acceptance_criteria: vec![],
            goal_indices,
            assigned_agent: None,
        }
    }

    fn make_phase_with_tasks(
        id: u32,
        name: &str,
        depends_on: Vec<u32>,
        tasks: Vec<TaskSpec>,
        produces: Vec<String>,
        consumes: Vec<String>,
    ) -> PhaseSpec {
        PhaseSpec {
            id,
            name: name.into(),
            description: format!("Phase {name}"),
            tasks,
            depends_on,
            produces,
            consumes,
        }
    }

    #[test]
    fn valid_plan_returns_ok_with_no_warnings() {
        let project = make_project(2);
        let phases = vec![
            make_phase_with_tasks(1, "Setup", vec![], vec![make_task(vec![0])], vec![], vec![]),
            make_phase_with_tasks(2, "Build", vec![1], vec![make_task(vec![1])], vec![], vec![]),
        ];
        let result = validate_plan(&phases, &project);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn circular_dependency_returns_err() {
        let project = make_project(1);
        let phases = vec![
            make_phase_with_tasks(1, "A", vec![3], vec![make_task(vec![0])], vec![], vec![]),
            make_phase_with_tasks(2, "B", vec![1], vec![make_task(vec![0])], vec![], vec![]),
            make_phase_with_tasks(3, "C", vec![2], vec![make_task(vec![0])], vec![], vec![]),
        ];
        let result = validate_plan(&phases, &project);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::CircularDependency { .. })));
    }

    #[test]
    fn missing_dependency_target_returns_err() {
        let project = make_project(1);
        let phases = vec![make_phase_with_tasks(
            1,
            "A",
            vec![99],
            vec![make_task(vec![0])],
            vec![],
            vec![],
        )];
        let result = validate_plan(&phases, &project);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::MissingDependencyTarget { phase_id: 1, missing_id: 99, .. }
        )));
    }

    #[test]
    fn orphaned_goal_returns_err() {
        let project = make_project(3); // 3 goals: 0, 1, 2
        let phases = vec![
            make_phase_with_tasks(1, "A", vec![], vec![make_task(vec![0])], vec![], vec![]),
            make_phase_with_tasks(2, "B", vec![1], vec![make_task(vec![2])], vec![], vec![]),
            // Goal 1 is orphaned -- no task maps to it
        ];
        let result = validate_plan(&phases, &project);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::OrphanedGoal { goal_index: 1, .. }
        )));
    }

    #[test]
    fn empty_phase_returns_err() {
        let project = make_project(1);
        let phases = vec![make_phase_with_tasks(
            1,
            "Empty",
            vec![],
            vec![],
            vec![],
            vec![],
        )];
        let result = validate_plan(&phases, &project);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::EmptyPhase { phase_id: 1, .. }
        )));
    }

    #[test]
    fn unsatisfied_contract_returns_err() {
        let project = make_project(1);
        let phases = vec![
            make_phase_with_tasks(
                1,
                "A",
                vec![],
                vec![make_task(vec![0])],
                vec!["types".into()],
                vec![],
            ),
            make_phase_with_tasks(
                2,
                "B",
                vec![1],
                vec![make_task(vec![0])],
                vec![],
                vec!["db-schema".into()], // Not produced by any dependency
            ),
        ];
        let result = validate_plan(&phases, &project);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::UnsatisfiedContract { phase_id: 2, ref contract, .. }
            if contract == "db-schema"
        )));
    }

    #[test]
    fn transitive_contract_satisfaction_is_ok() {
        // Phase C consumes "auth-types" which Phase A produces.
        // Phase C depends on B, B depends on A. Transitive path satisfies contract.
        let project = make_project(1);
        let phases = vec![
            make_phase_with_tasks(
                1,
                "A",
                vec![],
                vec![make_task(vec![0])],
                vec!["auth-types".into()],
                vec![],
            ),
            make_phase_with_tasks(
                2,
                "B",
                vec![1],
                vec![make_task(vec![0])],
                vec!["api-layer".into()],
                vec![],
            ),
            make_phase_with_tasks(
                3,
                "C",
                vec![2],
                vec![make_task(vec![0])],
                vec![],
                vec!["auth-types".into()], // Produced by A, transitive via B
            ),
        ];
        let result = validate_plan(&phases, &project);
        assert!(result.is_ok());
    }

    #[test]
    fn large_phase_triggers_warning() {
        let project = make_project(1);
        let tasks: Vec<TaskSpec> = (0..6).map(|_| make_task(vec![0])).collect();
        let phases = vec![make_phase_with_tasks(
            1,
            "Big",
            vec![],
            tasks,
            vec![],
            vec![],
        )];
        let result = validate_plan(&phases, &project);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert!(warnings.iter().any(|w| matches!(
            w,
            PlanWarning::LargePhase { phase_id: 1, task_count: 6, .. }
        )));
    }

    #[test]
    fn long_critical_path_triggers_warning() {
        let project = make_project(1);
        // Chain of 8 phases
        let phases: Vec<PhaseSpec> = (1..=8u32)
            .map(|id| {
                let deps = if id == 1 { vec![] } else { vec![id - 1] };
                make_phase_with_tasks(id, &format!("P{}", id), deps, vec![make_task(vec![0])], vec![], vec![])
            })
            .collect();
        let result = validate_plan(&phases, &project);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert!(warnings.iter().any(|w| matches!(
            w,
            PlanWarning::LongCriticalPath { length: 8 }
        )));
    }
}
