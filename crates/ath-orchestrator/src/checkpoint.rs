//! Checkpoint persistence for resumable execution.
//!
//! Saves completed phase records after each parallel group so that a failed
//! run can be resumed from the last successfully completed group. Uses atomic
//! temp-file + rename writes to prevent corruption on crash.

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ath_types::phase::PhaseRecord;
use ath_types::plan::ExecutionPlan;

use crate::error::PhaseRunnerError;

/// Persisted checkpoint for a partially completed run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier for this run.
    pub run_id: String,
    /// SHA-256 fingerprint of the execution plan.
    pub plan_fingerprint: String,
    /// The full execution plan (needed to resume without re-planning).
    pub plan: ExecutionPlan,
    /// Phase records accumulated so far.
    pub completed_records: Vec<PhaseRecord>,
    /// Set of phase IDs that have completed records (derived, but persisted for fast lookup).
    pub completed_phase_ids: HashSet<u32>,
    /// When this run started.
    pub started_at: DateTime<Utc>,
    /// When the checkpoint was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Checkpoint {
    /// Create a new checkpoint for a fresh run.
    pub fn new(run_id: String, plan: &ExecutionPlan) -> Self {
        let now = Utc::now();
        Self {
            run_id,
            plan_fingerprint: plan_fingerprint(plan),
            plan: plan.clone(),
            completed_records: Vec::new(),
            completed_phase_ids: HashSet::new(),
            started_at: now,
            updated_at: now,
        }
    }

    /// Add completed phase records from a group.
    /// Updates `completed_phase_ids` and `updated_at`.
    pub fn add_records(&mut self, records: Vec<PhaseRecord>) {
        for record in &records {
            self.completed_phase_ids.insert(record.phase_id);
        }
        self.completed_records.extend(records);
        self.updated_at = Utc::now();
    }

    /// Check if an entire parallel group should be skipped.
    /// Returns true only if ALL phase IDs in the group have completed records.
    pub fn should_skip_group(&self, group: &[u32]) -> bool {
        group.iter().all(|id| self.completed_phase_ids.contains(id))
    }

    /// Check if this checkpoint is stale (plan has changed since checkpoint was created).
    pub fn is_stale(&self, plan: &ExecutionPlan) -> bool {
        self.plan_fingerprint != plan_fingerprint(plan)
    }

    /// Number of completed phases.
    pub fn completed_count(&self) -> usize {
        self.completed_records.len()
    }
}

/// Compute a deterministic fingerprint for an execution plan.
///
/// Uses canonical JSON serialization → SHA-256 → hex encoding.
/// Same plan always produces the same fingerprint.
pub fn plan_fingerprint(plan: &ExecutionPlan) -> String {
    // serde_json::to_string produces deterministic output for the same struct
    // (field order follows declaration order, not insertion order).
    let json = serde_json::to_string(plan).expect("ExecutionPlan is always serializable");
    let hash = Sha256::digest(json.as_bytes());
    format!("{:x}", hash)
}

/// Atomic checkpoint persistence.
pub struct CheckpointStore;

impl CheckpointStore {
    /// Save a checkpoint atomically (temp file + rename).
    pub fn save(path: &Path, checkpoint: &Checkpoint) -> Result<(), PhaseRunnerError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: format!("failed to create checkpoint directory: {e}"),
            })?;
        }

        let json = serde_json::to_string_pretty(checkpoint).map_err(|e| {
            PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: format!("failed to serialize checkpoint: {e}"),
            }
        })?;

        let temp_path = path.with_extension("tmp");

        std::fs::write(&temp_path, &json).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
            path: path.display().to_string(),
            reason: format!("failed to write temp checkpoint: {e}"),
        })?;

        // On Windows, rename fails if target exists — remove first.
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }

        std::fs::rename(&temp_path, path).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
            path: path.display().to_string(),
            reason: format!("failed to rename checkpoint: {e}"),
        })?;

        Ok(())
    }

    /// Load a checkpoint from disk. Returns None if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Option<Checkpoint>, PhaseRunnerError> {
        if !path.exists() {
            return Ok(None);
        }

        let raw =
            std::fs::read_to_string(path).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: format!("failed to read checkpoint: {e}"),
            })?;

        let checkpoint: Checkpoint =
            serde_json::from_str(&raw).map_err(|e| PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: format!("corrupt checkpoint file: {e}"),
            })?;

        Ok(Some(checkpoint))
    }

    /// Delete a checkpoint file. No error if the file doesn't exist.
    pub fn delete(path: &Path) -> Result<(), PhaseRunnerError> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(PhaseRunnerError::AtomicWriteFailed {
                path: path.display().to_string(),
                reason: format!("failed to delete checkpoint: {e}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::phase::{AgentContribution, PhaseRecord, TokenUsage};
    use ath_types::plan::{ExecutionPlan, PhaseSpec, TaskSpec};
    use ath_types::project::SkillTag;

    fn make_plan(name: &str) -> ExecutionPlan {
        ExecutionPlan {
            phases: vec![PhaseSpec {
                id: 1,
                name: name.into(),
                description: format!("{name} description"),
                tasks: vec![TaskSpec {
                    name: "task-1".into(),
                    description: "task-1 desc".into(),
                    skill_tags: vec![SkillTag("rust".into())],
                    expected_output_files: vec!["src/lib.rs".into()],
                    acceptance_criteria: vec![],
                    goal_indices: vec![],
                    assigned_agent: None,
                }],
                depends_on: vec![],
                produces: vec![],
                consumes: vec![],
            }],
            execution_order: vec![1],
            parallel_groups: vec![vec![1]],
            critical_path_length: 1,
        }
    }

    fn make_multi_phase_plan() -> ExecutionPlan {
        ExecutionPlan {
            phases: vec![
                PhaseSpec {
                    id: 1,
                    name: "phase-1".into(),
                    description: "phase-1 desc".into(),
                    tasks: vec![],
                    depends_on: vec![],
                    produces: vec![],
                    consumes: vec![],
                },
                PhaseSpec {
                    id: 2,
                    name: "phase-2".into(),
                    description: "phase-2 desc".into(),
                    tasks: vec![],
                    depends_on: vec![1],
                    produces: vec![],
                    consumes: vec![],
                },
                PhaseSpec {
                    id: 3,
                    name: "phase-3".into(),
                    description: "phase-3 desc".into(),
                    tasks: vec![],
                    depends_on: vec![1],
                    produces: vec![],
                    consumes: vec![],
                },
                PhaseSpec {
                    id: 4,
                    name: "phase-4".into(),
                    description: "phase-4 desc".into(),
                    tasks: vec![],
                    depends_on: vec![2, 3],
                    produces: vec![],
                    consumes: vec![],
                },
            ],
            execution_order: vec![1, 2, 3, 4],
            parallel_groups: vec![vec![1], vec![2, 3], vec![4]],
            critical_path_length: 3,
        }
    }

    fn make_record(phase_id: u32) -> PhaseRecord {
        use ath_types::agent::AgentId;
        PhaseRecord {
            id: uuid::Uuid::new_v4(),
            phase_id,
            phase_name: format!("phase-{phase_id}"),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            contributions: vec![AgentContribution {
                agent: AgentId::claude("test"),
                tokens: TokenUsage::default(),
                files_produced: vec![],
            }],
            review_attempts: vec![],
        }
    }

    // -----------------------------------------------------------------------
    // Fingerprint tests
    // -----------------------------------------------------------------------

    #[test]
    fn fingerprint_is_deterministic() {
        let plan = make_plan("test-phase");
        let fp1 = plan_fingerprint(&plan);
        let fp2 = plan_fingerprint(&plan);
        assert_eq!(fp1, fp2);
        // SHA-256 hex is 64 chars
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn fingerprint_changes_with_different_plan() {
        let plan_a = make_plan("phase-alpha");
        let plan_b = make_plan("phase-beta");
        assert_ne!(plan_fingerprint(&plan_a), plan_fingerprint(&plan_b));
    }

    #[test]
    fn fingerprint_sensitive_to_execution_order() {
        let mut plan_a = make_multi_phase_plan();
        let mut plan_b = make_multi_phase_plan();
        plan_b.execution_order = vec![1, 3, 2, 4]; // swap 2 and 3
        assert_ne!(plan_fingerprint(&plan_a), plan_fingerprint(&plan_b));

        // But identical plans match
        plan_a.execution_order = vec![1, 2, 3, 4];
        plan_b.execution_order = vec![1, 2, 3, 4];
        assert_eq!(plan_fingerprint(&plan_a), plan_fingerprint(&plan_b));
    }

    // -----------------------------------------------------------------------
    // Checkpoint logic tests
    // -----------------------------------------------------------------------

    #[test]
    fn new_checkpoint_has_empty_records() {
        let plan = make_plan("test");
        let cp = Checkpoint::new("run-1".into(), &plan);
        assert!(cp.completed_records.is_empty());
        assert!(cp.completed_phase_ids.is_empty());
        assert_eq!(cp.completed_count(), 0);
        assert_eq!(cp.plan_fingerprint, plan_fingerprint(&plan));
    }

    #[test]
    fn add_records_updates_state() {
        let plan = make_multi_phase_plan();
        let mut cp = Checkpoint::new("run-1".into(), &plan);
        let before = cp.updated_at;

        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        cp.add_records(vec![make_record(1)]);
        assert_eq!(cp.completed_count(), 1);
        assert!(cp.completed_phase_ids.contains(&1));
        assert!(!cp.completed_phase_ids.contains(&2));
        assert!(cp.updated_at >= before);

        cp.add_records(vec![make_record(2), make_record(3)]);
        assert_eq!(cp.completed_count(), 3);
        assert!(cp.completed_phase_ids.contains(&2));
        assert!(cp.completed_phase_ids.contains(&3));
    }

    #[test]
    fn should_skip_fully_completed_group() {
        let plan = make_multi_phase_plan();
        let mut cp = Checkpoint::new("run-1".into(), &plan);
        cp.add_records(vec![make_record(2), make_record(3)]);

        // Group [2, 3] — all completed
        assert!(cp.should_skip_group(&[2, 3]));
    }

    #[test]
    fn should_not_skip_partially_completed_group() {
        let plan = make_multi_phase_plan();
        let mut cp = Checkpoint::new("run-1".into(), &plan);
        cp.add_records(vec![make_record(2)]);

        // Group [2, 3] — only 2 completed
        assert!(!cp.should_skip_group(&[2, 3]));
    }

    #[test]
    fn should_skip_empty_group() {
        let plan = make_plan("test");
        let cp = Checkpoint::new("run-1".into(), &plan);

        // Empty group — vacuously true
        assert!(cp.should_skip_group(&[]));
    }

    #[test]
    fn is_stale_detects_plan_change() {
        let plan_a = make_plan("original");
        let cp = Checkpoint::new("run-1".into(), &plan_a);

        // Same plan → not stale
        assert!(!cp.is_stale(&plan_a));

        // Different plan → stale
        let plan_b = make_plan("modified");
        assert!(cp.is_stale(&plan_b));
    }

    // -----------------------------------------------------------------------
    // Persistence tests
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("checkpoint.json");
        let plan = make_multi_phase_plan();

        let mut cp = Checkpoint::new("run-42".into(), &plan);
        cp.add_records(vec![make_record(1)]);

        CheckpointStore::save(&path, &cp).unwrap();

        let loaded = CheckpointStore::load(&path).unwrap().unwrap();
        assert_eq!(loaded.run_id, "run-42");
        assert_eq!(loaded.completed_count(), 1);
        assert!(loaded.completed_phase_ids.contains(&1));
        assert_eq!(loaded.plan_fingerprint, cp.plan_fingerprint);
        assert_eq!(loaded.plan, plan);
    }

    #[test]
    fn load_missing_file_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nonexistent.json");
        let result = CheckpointStore::load(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_corrupt_file_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("checkpoint.json");
        std::fs::write(&path, "not valid json {{{").unwrap();

        let result = CheckpointStore::load(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("corrupt checkpoint"),
            "expected corrupt message, got: {err}"
        );
    }

    #[test]
    fn delete_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("checkpoint.json");
        std::fs::write(&path, "{}").unwrap();

        CheckpointStore::delete(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn delete_missing_file_is_ok() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nonexistent.json");
        CheckpointStore::delete(&path).unwrap();
    }

    #[test]
    fn save_creates_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("deep")
            .join("nested")
            .join("checkpoint.json");
        let plan = make_plan("test");
        let cp = Checkpoint::new("run-1".into(), &plan);

        CheckpointStore::save(&path, &cp).unwrap();
        assert!(path.exists());

        let loaded = CheckpointStore::load(&path).unwrap().unwrap();
        assert_eq!(loaded.run_id, "run-1");
    }

    #[test]
    fn save_overwrites_existing_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("checkpoint.json");
        let plan = make_multi_phase_plan();

        let mut cp1 = Checkpoint::new("run-1".into(), &plan);
        cp1.add_records(vec![make_record(1)]);
        CheckpointStore::save(&path, &cp1).unwrap();

        let mut cp2 = Checkpoint::new("run-1".into(), &plan);
        cp2.add_records(vec![make_record(1), make_record(2), make_record(3)]);
        CheckpointStore::save(&path, &cp2).unwrap();

        let loaded = CheckpointStore::load(&path).unwrap().unwrap();
        assert_eq!(loaded.completed_count(), 3);
    }
}
