//! File ownership validation and post-execution audit.
//!
//! Pre-dispatch: validates that parallel-eligible phases have disjoint
//! file ownership before any agent is dispatched.
//! Post-execution: compares actual output files against declared expectations.

use std::collections::{HashMap, HashSet};

use ath_types::plan::{ExecutionPlan, TaskSpec};

use crate::error::IsolationError;

/// A warning produced by post-execution audit when actual output files
/// differ from declared expectations.
///
/// Warnings are informational -- they do not block execution.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditWarning {
    /// The task that produced the discrepancy.
    pub task_name: String,
    /// Files produced by the task but not declared in expected_output_files.
    pub unexpected_files: Vec<String>,
    /// Files declared in expected_output_files but not actually produced.
    pub missing_files: Vec<String>,
}

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
                            hint: "Split into separate phases or consolidate into one task".into(),
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

/// Compares actual output files against a task's declared expected_output_files.
///
/// Returns a list of warnings (empty if everything matches). Warnings are
/// informational only -- they do not block execution.
pub fn audit_outputs(task: &TaskSpec, actual_files: &[String]) -> Vec<AuditWarning> {
    let expected: HashSet<&str> = task
        .expected_output_files
        .iter()
        .map(|s| s.as_str())
        .collect();
    let actual: HashSet<&str> = actual_files.iter().map(|s| s.as_str()).collect();

    let mut unexpected: Vec<String> = actual
        .difference(&expected)
        .map(|s| s.to_string())
        .collect();
    unexpected.sort();

    let mut missing: Vec<String> = expected
        .difference(&actual)
        .map(|s| s.to_string())
        .collect();
    missing.sort();

    if unexpected.is_empty() && missing.is_empty() {
        return Vec::new();
    }

    vec![AuditWarning {
        task_name: task.name.clone(),
        unexpected_files: unexpected,
        missing_files: missing,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::plan::PhaseSpec;
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
        let p1 = make_phase(
            1,
            "Phase A",
            vec![make_task("T1", &["src/main.rs"])],
            vec![],
        );
        let p2 = make_phase(
            2,
            "Phase B",
            vec![make_task("T2", &["src/main.rs"])],
            vec![],
        );
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

    // --- audit_outputs tests ---

    #[test]
    fn audit_matching_files_returns_empty() {
        let task = make_task("T1", &["src/a.rs", "src/b.rs"]);
        let actual = vec!["src/a.rs".into(), "src/b.rs".into()];
        let warnings = audit_outputs(&task, &actual);
        assert!(warnings.is_empty());
    }

    #[test]
    fn audit_unexpected_files_returns_warning() {
        let task = make_task("T1", &["src/a.rs"]);
        let actual = vec!["src/a.rs".into(), "src/extra.rs".into()];
        let warnings = audit_outputs(&task, &actual);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].task_name, "T1");
        assert_eq!(warnings[0].unexpected_files, vec!["src/extra.rs"]);
        assert!(warnings[0].missing_files.is_empty());
    }

    #[test]
    fn audit_missing_files_returns_warning() {
        let task = make_task("T1", &["src/a.rs", "src/b.rs"]);
        let actual = vec!["src/a.rs".into()];
        let warnings = audit_outputs(&task, &actual);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].task_name, "T1");
        assert!(warnings[0].unexpected_files.is_empty());
        assert_eq!(warnings[0].missing_files, vec!["src/b.rs"]);
    }

    #[test]
    fn audit_both_unexpected_and_missing_returns_warning() {
        let task = make_task("T1", &["src/expected.rs"]);
        let actual = vec!["src/surprise.rs".into()];
        let warnings = audit_outputs(&task, &actual);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].task_name, "T1");
        assert_eq!(warnings[0].unexpected_files, vec!["src/surprise.rs"]);
        assert_eq!(warnings[0].missing_files, vec!["src/expected.rs"]);
    }

    #[test]
    fn audit_warning_includes_task_name() {
        let task = make_task("My Important Task", &["src/a.rs"]);
        let actual: Vec<String> = vec![];
        let warnings = audit_outputs(&task, &actual);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].task_name, "My Important Task");
    }
}
