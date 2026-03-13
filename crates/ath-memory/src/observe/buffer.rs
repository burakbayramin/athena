//! Thread-safe in-memory observation buffer.
//!
//! `ObservationBuffer` collects observations during a run and flushes
//! them to an `ObservationWriter` on demand. It enforces a configurable
//! cap (default 500) and evicts the oldest observation when full.

use std::sync::Mutex;

use chrono::Utc;
use uuid::Uuid;

use super::storage::ObservationWriter;
use super::types::{Observation, ObservationType};
use crate::error::MemoryError;

/// Internal state guarded by the mutex.
struct BufferInner {
    observations: Vec<Observation>,
    run_id: Uuid,
    max_cap: usize,
}

/// Thread-safe buffer that collects observations during a run.
///
/// Uses `Mutex` with poison recovery — if a thread panics while holding
/// the lock, subsequent operations recover the inner state rather than
/// propagating the panic. Observation loss is acceptable; run failure is not.
pub struct ObservationBuffer {
    inner: Mutex<BufferInner>,
}

// Compile-time assertions that ObservationBuffer is Send + Sync.
const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assertions() {
        assert_send::<ObservationBuffer>();
        assert_sync::<ObservationBuffer>();
    }
};

impl ObservationBuffer {
    /// Create a new buffer for the given run with the default cap (500).
    pub fn new(run_id: Uuid) -> Self {
        Self::with_capacity(run_id, 500)
    }

    /// Create a new buffer with a custom cap.
    pub fn with_capacity(run_id: Uuid, max_cap: usize) -> Self {
        Self {
            inner: Mutex::new(BufferInner {
                observations: Vec::with_capacity(max_cap.min(1024)),
                run_id,
                max_cap,
            }),
        }
    }

    /// Record an observation event.
    ///
    /// Constructs an `Observation` with an auto-generated ID and the current
    /// timestamp. If the buffer is at capacity, the oldest observation is
    /// evicted before the new one is pushed.
    pub fn record(&self, event: ObservationType) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Extract phase_id from the event for the wrapper
        let phase_id = match &event {
            ObservationType::AgentRequest { phase_id, .. }
            | ObservationType::AgentResponse { phase_id, .. }
            | ObservationType::RetryStarted { phase_id, .. }
            | ObservationType::FileOperation { phase_id, .. }
            | ObservationType::Error { phase_id, .. }
            | ObservationType::RoutingDecision { phase_id, .. } => *phase_id,
            ObservationType::ReviewVerdict { phase_id, .. } => *phase_id,
        };

        let obs = Observation {
            id: Uuid::new_v4(),
            run_id: inner.run_id,
            timestamp: Utc::now(),
            phase_id,
            event,
        };

        if inner.observations.len() >= inner.max_cap {
            inner.observations.remove(0);
        }
        inner.observations.push(obs);
    }

    /// Flush all buffered observations to the writer.
    ///
    /// Drains the buffer and writes each observation. Returns the number
    /// of observations written. The writer is flushed after all observations
    /// are written.
    pub fn flush(&self, writer: &mut ObservationWriter) -> Result<usize, MemoryError> {
        let observations = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::take(&mut inner.observations)
        };

        let count = observations.len();
        for obs in &observations {
            writer.write(obs)?;
        }
        writer.flush()?;
        Ok(count)
    }

    /// Returns the number of observations currently buffered.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .observations
            .len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the run ID for this buffer.
    pub fn run_id(&self) -> Uuid {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .run_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::storage::ObservationReader;
    use crate::observe::types::FileOpKind;
    use ath_types::{AgentKind, Severity, TokenUsage};
    use tempfile::TempDir;

    fn make_event(index: u32) -> ObservationType {
        ObservationType::FileOperation {
            op: FileOpKind::Create,
            path: format!("file_{index}.rs"),
            phase_id: Some(index),
        }
    }

    #[test]
    fn record_and_len() {
        let buf = ObservationBuffer::new(Uuid::new_v4());
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());

        buf.record(make_event(1));
        assert_eq!(buf.len(), 1);
        assert!(!buf.is_empty());

        buf.record(make_event(2));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn cap_enforcement_evicts_oldest() {
        let run_id = Uuid::new_v4();
        let buf = ObservationBuffer::with_capacity(run_id, 5);

        // Fill to capacity
        for i in 0..5 {
            buf.record(make_event(i));
        }
        assert_eq!(buf.len(), 5);

        // One more should evict the oldest
        buf.record(make_event(99));
        assert_eq!(buf.len(), 5);

        // Verify the oldest was evicted — flush and check phase_ids
        let dir = TempDir::new().unwrap();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();
        let count = buf.flush(&mut writer).unwrap();
        assert_eq!(count, 5);

        let observations = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        // First observation should be phase_id 1 (phase_id 0 was evicted)
        assert_eq!(observations[0].phase_id, Some(1));
        // Last observation should be phase_id 99
        assert_eq!(observations[4].phase_id, Some(99));
    }

    #[test]
    fn cap_501_yields_500() {
        let buf = ObservationBuffer::with_capacity(Uuid::new_v4(), 500);

        for i in 0..501 {
            buf.record(make_event(i));
        }
        assert_eq!(buf.len(), 500);
    }

    #[test]
    fn flush_drains_buffer() {
        let run_id = Uuid::new_v4();
        let buf = ObservationBuffer::new(run_id);

        buf.record(make_event(1));
        buf.record(make_event(2));
        buf.record(make_event(3));
        assert_eq!(buf.len(), 3);

        let dir = TempDir::new().unwrap();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();
        let count = buf.flush(&mut writer).unwrap();

        assert_eq!(count, 3);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());

        // Verify written to disk
        let observations = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(observations.len(), 3);
    }

    #[test]
    fn flush_empty_buffer() {
        let run_id = Uuid::new_v4();
        let buf = ObservationBuffer::new(run_id);

        let dir = TempDir::new().unwrap();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();
        let count = buf.flush(&mut writer).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn flush_all_event_types() {
        let run_id = Uuid::new_v4();
        let buf = ObservationBuffer::new(run_id);

        buf.record(ObservationType::AgentRequest {
            agent: AgentKind::Claude("opus-4".into()),
            prompt_summary: "test".into(),
            phase_id: Some(1),
            task_name: None,
        });
        buf.record(ObservationType::AgentResponse {
            agent: AgentKind::Gemini("2.5-pro".into()),
            token_usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                estimated_cost_usd: 0.01,
            },
            phase_id: Some(1),
            task_name: Some("gen".into()),
        });
        buf.record(ObservationType::ReviewVerdict {
            reviewer: AgentKind::Codex("o3".into()),
            passed: true,
            severity: Severity::Info,
            reason_summary: "ok".into(),
            attempt_number: 1,
            phase_id: Some(2),
        });
        buf.record(ObservationType::RetryStarted {
            phase_id: Some(2),
            attempt_number: 2,
            feedback_summary: "fix".into(),
        });
        buf.record(ObservationType::FileOperation {
            op: FileOpKind::Delete,
            path: "old.rs".into(),
            phase_id: None,
        });
        buf.record(ObservationType::Error {
            message: "boom".into(),
            phase_id: None,
            severity: Severity::Critical,
        });
        buf.record(ObservationType::RoutingDecision {
            agent: AgentKind::Claude("opus-4".into()),
            task_name: "task".into(),
            reason: "because".into(),
            phase_id: Some(3),
        });

        assert_eq!(buf.len(), 7);

        let dir = TempDir::new().unwrap();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();
        let count = buf.flush(&mut writer).unwrap();
        assert_eq!(count, 7);

        let read_back = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(read_back.len(), 7);
    }

    #[test]
    fn send_sync_compile_assertion() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ObservationBuffer>();
    }

    #[test]
    fn buffer_preserves_run_id() {
        let run_id = Uuid::new_v4();
        let buf = ObservationBuffer::new(run_id);
        assert_eq!(buf.run_id(), run_id);

        buf.record(make_event(1));

        let dir = TempDir::new().unwrap();
        let mut writer = ObservationWriter::new(dir.path(), &run_id).unwrap();
        buf.flush(&mut writer).unwrap();

        let observations = ObservationReader::read_run(dir.path(), &run_id).unwrap();
        assert_eq!(observations[0].run_id, run_id);
    }
}
