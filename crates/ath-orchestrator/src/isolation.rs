//! File ownership validation and post-execution audit.
//!
//! Pre-dispatch: validates that parallel-eligible phases have disjoint
//! file ownership before any agent is dispatched.
//! Post-execution: compares actual output files against declared expectations.

use std::collections::HashMap;

use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};

use crate::error::IsolationError;

/// Validates that no two phases within the same parallel group claim the same output file.
///
/// Tasks within a single phase execute sequentially and are allowed to share files.
/// Only cross-phase overlaps within a parallel group are flagged as conflicts.
pub fn check_isolation(plan: &ExecutionPlan) -> Result<(), IsolationError> {
    for group in &plan.parallel_groups {
        // Build ownership map: file -> (phase_id, task_name)
        let mut ownership: HashMap<&str, (u32, &str)> = HashMap::new();

        for &phase_id in group {
            // Find the PhaseSpec by id
            let phase = match plan.phases.iter().find(|p| p.id == phase_id) {
                Some(p) => p,
                None => continue,
            };

            for task in &phase.tasks {
                for file in &task.expected_output_files {
                    if let Some(&(existing_phase, ref existing_task)) = ownership.get(file.as_str())
                    {
                        // Same phase -> sequential tasks, allow sharing
                        if existing_phase == phase_id {
                            continue;
                        }
                        // Different phase in same parallel group -> conflict
                        return Err(IsolationError::FileConflict {
                            file: file.clone(),
                            task_a: existing_task.to_string(),
                            phase_a: existing_phase,
                            task_b: task.name.clone(),
                            phase_b: phase_id,
                            hint: "Split into separate phases or consolidate into one task"
                                .into(),
                        });
                    } else {
                        ownership.insert(file.as_str(), (phase_id, &task.name));
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::project::SkillTag;

    fn make_task(name: &str, files: &[&str]) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: String::new(),
            skill_tags: vec![SkillTag("rust".into())],
            expected_output_files: files.iter().map(|f| f.to_string()).collect(),
            acceptance_criteria: vec![],
            goal_indices: vec![],
            assigned_agent: None,
        }
    }

    fn make_phase(id: u32, name: &str, tasks: Vec<TaskSpec>, depends_on: Vec<u32>) -> PhaseSpec {
        PhaseSpec {
            id,
            name: name.into(),
            description: String::new(),
            tasks,
            depends_on,
            produces: vec![],
            consumes: vec![],
        }
    }

    fn make_plan(phases: Vec<PhaseSpec>, parallel_groups: Vec<Vec<u32>>) -> ExecutionPlan {
        let execution_order: Vec<u32> = phases.iter().map(|p| p.id).collect();
        let critical_path_length = parallel_groups.len();
        ExecutionPlan {
            phases,
            execution_order,
            parallel_groups,
            critical_path_length,
        }
    }

    // --- check_isolation tests ---

    #[test]
    fn check_isolation_disjoint_parallel_phases_ok() {
        let p1 = make_phase(1, "Phase A", vec![make_task("T1", &["src/a.rs"])], vec![]);
        let p2 = make_phase(2, "Phase B", vec![make_task("T2", &["src/b.rs"])], vec![]);
        let plan = make_plan(vec![p1, p2], vec![vec![1, 2]]);
        assert!(check_isolation(&plan).is_ok());
    }

    #[test]
    fn check_isolation_overlapping_parallel_phases_conflict() {
        let p1 = make_phase(1, "Phase A", vec![make_task("T1", &["src/main.rs"])], vec![]);
        let p2 = make_phase(2, "Phase B", vec![make_task("T2", &["src/main.rs"])], vec![]);
        let plan = make_plan(vec![p1, p2], vec![vec![1, 2]]);
        let err = check_isolation(&plan).unwrap_err();
        match &err {
            IsolationError::FileConflict {
                file,
                task_a,
                phase_a,
                task_b,
                phase_b,
                ..
            } => {
                assert_eq!(file, "src/main.rs");
                assert_eq!(task_a, "T1");
                assert_eq!(*phase_a, 1);
                assert_eq!(task_b, "T2");
                assert_eq!(*phase_b, 2);
            }
            _ => panic!("Expected FileConflict"),
        }
    }

    #[test]
    fn check_isolation_same_file_sequential_phases_ok() {
        // Phases 1 and 2 share a file but are in different parallel groups (sequential)
        let p1 = make_phase(1, "Phase A", vec![make_task("T1", &["src/lib.rs"])], vec![]);
        let p2 = make_phase(
            2,
            "Phase B",
            vec![make_task("T2", &["src/lib.rs"])],
            vec![1],
        );
        let plan = make_plan(vec![p1, p2], vec![vec![1], vec![2]]);
        assert!(check_isolation(&plan).is_ok());
    }

    #[test]
    fn check_isolation_same_file_within_phase_ok() {
        // Two tasks in the same phase sharing a file is allowed (sequential execution)
        let t1 = make_task("T1", &["src/shared.rs"]);
        let t2 = make_task("T2", &["src/shared.rs"]);
        let p1 = make_phase(1, "Phase A", vec![t1, t2], vec![]);
        let plan = make_plan(vec![p1], vec![vec![1]]);
        assert!(check_isolation(&plan).is_ok());
    }

    #[test]
    fn check_isolation_conflict_identifies_file_tasks_phases() {
        let p1 = make_phase(
            3,
            "Phase X",
            vec![make_task("Build API", &["src/api.rs"])],
            vec![],
        );
        let p2 = make_phase(
            5,
            "Phase Y",
            vec![make_task("Add CLI", &["src/api.rs"])],
            vec![],
        );
        let plan = make_plan(vec![p1, p2], vec![vec![3, 5]]);
        let err = check_isolation(&plan).unwrap_err();
        match &err {
            IsolationError::FileConflict {
                file,
                task_a,
                phase_a,
                task_b,
                phase_b,
                ..
            } => {
                assert_eq!(file, "src/api.rs");
                assert_eq!(task_a, "Build API");
                assert_eq!(*phase_a, 3);
                assert_eq!(task_b, "Add CLI");
                assert_eq!(*phase_b, 5);
            }
            _ => panic!("Expected FileConflict"),
        }
    }

    #[test]
    fn check_isolation_conflict_hint_message() {
        let p1 = make_phase(1, "A", vec![make_task("T1", &["f.rs"])], vec![]);
        let p2 = make_phase(2, "B", vec![make_task("T2", &["f.rs"])], vec![]);
        let plan = make_plan(vec![p1, p2], vec![vec![1, 2]]);
        let err = check_isolation(&plan).unwrap_err();
        assert_eq!(
            err.hint(),
            "Split into separate phases or consolidate into one task"
        );
    }

    #[test]
    fn check_isolation_single_phase_groups_always_pass() {
        let p1 = make_phase(1, "A", vec![make_task("T1", &["src/a.rs"])], vec![]);
        let p2 = make_phase(2, "B", vec![make_task("T2", &["src/a.rs"])], vec![1]);
        // Each group has only one phase -- no parallel conflict possible
        let plan = make_plan(vec![p1, p2], vec![vec![1], vec![2]]);
        assert!(check_isolation(&plan).is_ok());
    }

    #[test]
    fn check_isolation_empty_parallel_groups_ok() {
        let plan = make_plan(vec![], vec![]);
        assert!(check_isolation(&plan).is_ok());
    }
}
