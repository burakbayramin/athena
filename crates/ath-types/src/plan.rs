//! Execution plan types for phase decomposition.
//!
//! These types represent the output of the LLM-based project decomposition:
//! an ordered set of phases with dependency edges, parallelism groups, and
//! critical path information.

use serde::{Deserialize, Serialize};

use crate::agent::AgentKind;
use crate::project::SkillTag;

/// A named contract label for inter-phase produces/consumes tracking.
pub type ContractLabel = String;

/// A single task within a phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSpec {
    /// Task name.
    pub name: String,
    /// Task description.
    pub description: String,
    /// Skills needed to accomplish this task.
    pub skill_tags: Vec<SkillTag>,
    /// Files expected to be created or modified.
    pub expected_output_files: Vec<String>,
    /// Criteria that must be met for the task to be considered complete.
    pub acceptance_criteria: Vec<String>,
    /// Indices into the ProjectSpec.goals array that this task fulfills.
    pub goal_indices: Vec<usize>,
    /// Agent assigned to execute this task (None before routing, Some after).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<AgentKind>,
}

/// A phase in the execution plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseSpec {
    /// Unique phase identifier.
    pub id: u32,
    /// Phase name.
    pub name: String,
    /// Phase description.
    pub description: String,
    /// Tasks within this phase (executed sequentially).
    pub tasks: Vec<TaskSpec>,
    /// IDs of phases this phase depends on.
    pub depends_on: Vec<u32>,
    /// Contract labels this phase produces (available to dependents).
    pub produces: Vec<ContractLabel>,
    /// Contract labels this phase consumes (must be produced by a dependency).
    pub consumes: Vec<ContractLabel>,
}

/// The full execution plan produced by decomposition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionPlan {
    /// All phases in the plan.
    pub phases: Vec<PhaseSpec>,
    /// Topologically sorted phase IDs (valid execution order).
    pub execution_order: Vec<u32>,
    /// Groups of phase IDs that can execute in parallel.
    pub parallel_groups: Vec<Vec<u32>>,
    /// Length of the critical (longest) dependency path.
    pub critical_path_length: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task() -> TaskSpec {
        TaskSpec {
            name: "Implement API".into(),
            description: "Build the REST endpoints".into(),
            skill_tags: vec![SkillTag("rust".into()), SkillTag("api".into())],
            expected_output_files: vec!["src/api.rs".into()],
            acceptance_criteria: vec!["All endpoints return JSON".into()],
            goal_indices: vec![0, 1],
            assigned_agent: None,
        }
    }

    fn sample_phase() -> PhaseSpec {
        PhaseSpec {
            id: 1,
            name: "API Layer".into(),
            description: "Build the API".into(),
            tasks: vec![sample_task()],
            depends_on: vec![0],
            produces: vec!["api-types".into()],
            consumes: vec!["db-schema".into()],
        }
    }

    fn sample_plan() -> ExecutionPlan {
        ExecutionPlan {
            phases: vec![sample_phase()],
            execution_order: vec![0, 1],
            parallel_groups: vec![vec![0], vec![1]],
            critical_path_length: 2,
        }
    }

    #[test]
    fn execution_plan_round_trip() {
        let plan = sample_plan();
        let json = serde_json::to_string(&plan).expect("serialize");
        let deserialized: ExecutionPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(plan, deserialized);
    }

    #[test]
    fn phase_spec_round_trip() {
        let phase = sample_phase();
        let json = serde_json::to_string(&phase).expect("serialize");
        let deserialized: PhaseSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(phase, deserialized);
    }

    #[test]
    fn task_spec_round_trip() {
        let task = sample_task();
        let json = serde_json::to_string(&task).expect("serialize");
        let deserialized: TaskSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(task, deserialized);
    }

    #[test]
    fn task_spec_assigned_agent_none_omitted_in_json() {
        let task = sample_task();
        let json = serde_json::to_string(&task).expect("serialize");
        assert!(!json.contains("assigned_agent"), "None should be skipped in serialization");
    }

    #[test]
    fn task_spec_without_assigned_agent_deserializes() {
        // Simulate old JSON without assigned_agent field
        let json = r#"{
            "name": "Test",
            "description": "A test task",
            "skill_tags": [],
            "expected_output_files": [],
            "acceptance_criteria": [],
            "goal_indices": []
        }"#;
        let task: TaskSpec = serde_json::from_str(json).expect("backward compat deserialize");
        assert_eq!(task.assigned_agent, None);
    }

    #[test]
    fn task_spec_assigned_agent_round_trip() {
        let mut task = sample_task();
        task.assigned_agent = Some(AgentKind::Claude("opus-4".into()));
        let json = serde_json::to_string(&task).expect("serialize");
        assert!(json.contains("assigned_agent"));
        let deserialized: TaskSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(task, deserialized);
    }
}
