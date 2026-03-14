//! Append-only JSONL storage for observations.
//!
//! `ObservationWriter` appends serialized observations to a per-run JSONL file.
//! `ObservationReader` reads them back for downstream consumers.
//!
//! Storage path convention: `<root>/observations/<run-uuid>.jsonl`

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::types::Observation;
use crate::error::MemoryError;

/// Append-only JSONL writer for a single run's observations.
///
/// Creates `<root>/<run_id>.jsonl` on first write, using `BufWriter`
/// for batched I/O. Call [`flush`](Self::flush) to guarantee all
/// buffered data hits disk.
pub struct ObservationWriter {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl ObservationWriter {
    /// Create a new writer for the given run under `root`.
    ///
    /// Creates parent directories if they don't exist.
    /// Opens the file in append mode so multiple writer instances
    /// (across process restarts) are safe.
    pub fn new(root: &Path, run_id: &Uuid) -> Result<Self, MemoryError> {
        let dir = root.to_path_buf();
        fs::create_dir_all(&dir).map_err(|e| MemoryError::ObservationWriteError {
            path: dir.display().to_string(),
            message: format!("Failed to create observations directory: {e}"),
        })?;

        let path = dir.join(format!("{run_id}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| MemoryError::ObservationWriteError {
                path: path.display().to_string(),
                message: format!("Failed to open observation file: {e}"),
            })?;

        Ok(Self {
            writer: BufWriter::new(file),
            path,
        })
    }

    /// Serialize and append a single observation as a JSONL line.
    pub fn write(&mut self, obs: &Observation) -> Result<(), MemoryError> {
        let json = serde_json::to_string(obs).map_err(|e| MemoryError::ObservationWriteError {
            path: self.path.display().to_string(),
            message: format!("Failed to serialize observation: {e}"),
        })?;

        self.writer
            .write_all(json.as_bytes())
            .map_err(|e| MemoryError::ObservationWriteError {
                path: self.path.display().to_string(),
                message: format!("Failed to write observation: {e}"),
            })?;

        self.writer
            .write_all(b"\n")
            .map_err(|e| MemoryError::ObservationWriteError {
                path: self.path.display().to_string(),
                message: format!("Failed to write newline: {e}"),
            })?;

        Ok(())
    }

    /// Flush all buffered data to disk.
    pub fn flush(&mut self) -> Result<(), MemoryError> {
        self.writer
            .flush()
            .map_err(|e| MemoryError::ObservationWriteError {
                path: self.path.display().to_string(),
                message: format!("Failed to flush observation writer: {e}"),
            })
    }

    /// Returns the path to the JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Reads observations from JSONL files.
pub struct ObservationReader;

impl ObservationReader {
    /// Read all observations for a specific run from the standard location.
    ///
    /// Looks for `<root>/<run_id>.jsonl`. Returns an empty vec if the file
    /// doesn't exist. Skips empty lines.
    pub fn read_run(root: &Path, run_id: &Uuid) -> Result<Vec<Observation>, MemoryError> {
        let path = root.join(format!("{run_id}.jsonl"));
        if !path.exists() {
            return Ok(Vec::new());
        }
        Self::read_file(&path)
    }

    /// Read all observations from an arbitrary JSONL file path.
    ///
    /// Skips empty and whitespace-only lines. Returns an error if the file
    /// cannot be opened or if any non-empty line fails to parse.
    pub fn read_file(path: &Path) -> Result<Vec<Observation>, MemoryError> {
        let file = File::open(path).map_err(|e| MemoryError::ObservationReadError {
            path: path.display().to_string(),
            message: format!("Failed to open observation file: {e}"),
        })?;

        let reader = BufReader::new(file);
        let mut observations = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result.map_err(|e| MemoryError::ObservationReadError {
                path: path.display().to_string(),
                message: format!("Failed to read line {}: {e}", line_num + 1),
            })?;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let obs: Observation =
                serde_json::from_str(trimmed).map_err(|e| MemoryError::ObservationReadError {
                    path: path.display().to_string(),
                    message: format!("Failed to parse line {}: {e}", line_num + 1),
                })?;

            observations.push(obs);
        }

        Ok(observations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::types::{FileOpKind, ObservationType};
    use ath_types::{AgentId, Severity, TokenUsage};
    use chrono::Utc;
    use tempfile::TempDir;

    fn sample_observation(run_id: Uuid) -> Observation {
        Observation {
            id: Uuid::new_v4(),
            run_id,
            timestamp: Utc::now(),
            phase_id: Some(1),
            event: ObservationType::AgentRequest {
                agent: AgentId::claude("opus-4"),
                prompt_summary: "Analyze code".into(),
                phase_id: Some(1),
                task_name: Some("analysis".into()),
            },
        }
    }

    #[test]
    fn writer_creates_file_and_appends() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();

        let obs = sample_observation(run_id);
        writer.write(&obs).unwrap();
        writer.flush().unwrap();

        let path = dir.path().join(format!("{run_id}.jsonl"));
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn writer_appends_multiple() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();

        for _ in 0..5 {
            writer.write(&sample_observation(run_id)).unwrap();
        }
        writer.flush().unwrap();

        let path = dir.path().join(format!("{run_id}.jsonl"));
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 5);
    }

    #[test]
    fn writer_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("deep").join("nested").join("observations");
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(&nested, &run_id).unwrap();

        writer.write(&sample_observation(run_id)).unwrap();
        writer.flush().unwrap();

        assert!(nested.join(format!("{run_id}.jsonl")).exists());
    }

    #[test]
    fn reader_reads_back_identically() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();

        let mut originals = Vec::new();
        for i in 0..3 {
            let obs = Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: Utc::now(),
                phase_id: Some(i),
                event: ObservationType::FileOperation {
                    op: FileOpKind::Create,
                    path: format!("src/file_{i}.rs"),
                    phase_id: Some(i),
                },
            };
            writer.write(&obs).unwrap();
            originals.push(obs);
        }
        writer.flush().unwrap();

        let read_back = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(originals, read_back);
    }

    #[test]
    fn reader_handles_empty_file() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let path = dir.path().join(format!("{run_id}.jsonl"));
        File::create(&path).unwrap();

        let result = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn reader_skips_blank_lines() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let path = dir.path().join(format!("{run_id}.jsonl"));

        // Write one valid line surrounded by blank lines
        let obs = sample_observation(run_id);
        let json = serde_json::to_string(&obs).unwrap();
        let content = format!("\n\n{json}\n\n  \n");
        fs::write(&path, content).unwrap();

        let result = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], obs);
    }

    #[test]
    fn reader_returns_empty_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let result = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn reader_read_file_directly() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();

        let obs = sample_observation(run_id);
        writer.write(&obs).unwrap();
        writer.flush().unwrap();

        let result = ObservationReader::read_file(writer.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], obs);
    }

    #[test]
    fn write_and_read_all_event_types() {
        let dir = TempDir::new().unwrap();
        let run_id = Uuid::new_v4();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();

        let events = vec![
            ObservationType::AgentRequest {
                agent: AgentId::claude("opus-4"),
                prompt_summary: "test".into(),
                phase_id: Some(1),
                task_name: None,
            },
            ObservationType::AgentResponse {
                agent: AgentId::gemini("2.5-pro"),
                token_usage: TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    estimated_cost_usd: 0.01,
                },
                phase_id: Some(1),
                task_name: Some("gen".into()),
            },
            ObservationType::ReviewVerdict {
                reviewer: AgentId::codex("o3"),
                passed: true,
                severity: Severity::Info,
                reason_summary: "Looks good".into(),
                attempt_number: 1,
                phase_id: Some(2),
            },
            ObservationType::RetryStarted {
                phase_id: Some(2),
                attempt_number: 2,
                feedback_summary: "Fix error handling".into(),
            },
            ObservationType::FileOperation {
                op: FileOpKind::Modify,
                path: "src/lib.rs".into(),
                phase_id: Some(1),
            },
            ObservationType::Error {
                message: "timeout".into(),
                phase_id: None,
                severity: Severity::Warning,
            },
            ObservationType::RoutingDecision {
                agent: AgentId::claude("opus-4"),
                task_name: "audit".into(),
                reason: "Best for security".into(),
                phase_id: Some(3),
            },
        ];

        let mut originals = Vec::new();
        for event in events {
            let obs = Observation {
                id: Uuid::new_v4(),
                run_id,
                timestamp: Utc::now(),
                phase_id: Some(1),
                event,
            };
            writer.write(&obs).unwrap();
            originals.push(obs);
        }
        writer.flush().unwrap();

        let read_back = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(originals, read_back);
    }
}
