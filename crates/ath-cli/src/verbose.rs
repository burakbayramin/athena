use std::sync::{Arc, Mutex};

use ath_orchestrator::progress::{ProgressEvent, ProgressObserver, Transcript, TranscriptKind};
use ath_types::agent::AgentKind;

use crate::progress::TerminalProgressReporter;

pub(crate) struct CliObserver {
    progress: Arc<TerminalProgressReporter>,
    verbose: Option<Arc<VerboseTranscriptSink>>,
}

impl CliObserver {
    pub(crate) fn new(
        progress: Arc<TerminalProgressReporter>,
        verbose: Option<Arc<VerboseTranscriptSink>>,
    ) -> Self {
        Self { progress, verbose }
    }
}

impl ProgressObserver for CliObserver {
    fn on_event(&self, event: ProgressEvent) {
        self.progress.on_event(event.clone());
        if let Some(verbose) = &self.verbose {
            verbose.on_event(&event);
        }
    }

    fn captures_transcripts(&self) -> bool {
        self.verbose.is_some()
    }
}

pub(crate) struct VerboseTranscriptSink {
    progress: Arc<TerminalProgressReporter>,
    secrets: Vec<String>,
    blocks: Mutex<Vec<String>>,
}

impl VerboseTranscriptSink {
    pub(crate) fn new(progress: Arc<TerminalProgressReporter>, secrets: Vec<String>) -> Self {
        Self {
            progress,
            secrets,
            blocks: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn on_event(&self, event: &ProgressEvent) {
        if let ProgressEvent::Transcript(transcript) = event {
            let block = format_transcript_block(transcript, &self.secrets);
            self.blocks.lock().unwrap().push(block.clone());
            self.progress.print_durable_block(&block);
        }
    }

    #[cfg(test)]
    pub(crate) fn blocks(&self) -> Vec<String> {
        self.blocks.lock().unwrap().clone()
    }
}

pub(crate) fn format_transcript_block(transcript: &Transcript, secrets: &[String]) -> String {
    let title = match transcript.kind {
        TranscriptKind::Executor => "Executor Transcript",
        TranscriptKind::Reviewer => "Reviewer Transcript",
        TranscriptKind::RetryFeedback => "Retry Feedback",
    };

    let mut lines = vec![format!(
        "[{}] {} / {} / attempt {} / {}",
        title,
        transcript.phase_name,
        transcript.label,
        transcript.attempt_number,
        format_agent(&transcript.agent)
    )];

    let prompt = redact(&transcript.prompt, secrets);
    let response = redact(&transcript.response, secrets);
    lines.push("Prompt:".into());
    lines.push(prompt);
    lines.push("Response:".into());
    lines.push(response);

    if let Some(feedback) = &transcript.retry_feedback {
        lines.push("Retry Feedback Context:".into());
        lines.push(redact(feedback, secrets));
    }

    lines.join("\n")
}

pub(crate) fn redact(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_string();

    for secret in secrets {
        if !secret.trim().is_empty() {
            redacted = redacted.replace(secret, "[REDACTED]");
        }
    }

    redacted = redact_assignment_values(&redacted);

    for prefix in ["sk-", "AIza", "ghp_", "xoxb-", "xoxp-"] {
        redacted = redact_prefixed_token(&redacted, prefix);
    }

    redacted
}

fn redact_assignment_values(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.lines() {
        let upper = line.to_ascii_uppercase();
        if (upper.contains("API_KEY")
            || upper.contains("TOKEN")
            || upper.contains("SECRET")
            || upper.contains("PASSWORD"))
            && (line.contains('=') || line.contains(':'))
        {
            if let Some((key, _)) = line.split_once('=') {
                lines.push(format!("{}=[REDACTED]", key.trim_end()));
                continue;
            }
            if let Some((key, _)) = line.split_once(':') {
                lines.push(format!("{}: [REDACTED]", key.trim_end()));
                continue;
            }
        }

        lines.push(line.to_string());
    }

    lines.join("\n")
}

fn redact_prefixed_token(text: &str, prefix: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index..].starts_with(prefix_bytes) {
            let mut end = index + prefix_bytes.len();
            while end < bytes.len() && is_secret_char(bytes[end] as char) {
                end += 1;
            }

            if end > index + prefix_bytes.len() {
                redacted.push_str("[REDACTED]");
                index = end;
                continue;
            }
        }

        redacted.push(bytes[index] as char);
        index += 1;
    }

    redacted
}

fn is_secret_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/')
}

fn format_agent(agent: &AgentKind) -> String {
    format!("{}/{}", agent.provider_name(), agent.model())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_orchestrator::progress::ProgressEvent;

    fn sample_transcript(kind: TranscriptKind) -> Transcript {
        Transcript {
            kind,
            phase_id: 1,
            phase_name: "foundation".into(),
            label: "task-1".into(),
            attempt_number: 1,
            agent: AgentKind::Claude("opus-4".into()),
            prompt: "Use OPENAI_API_KEY=sk-test-secret".into(),
            response: "Created output with AIzaSecretToken".into(),
            retry_feedback: Some("ANTHROPIC_API_KEY=secret-value".into()),
        }
    }

    #[test]
    fn verbose_mode_preserves_normal_progress_output() {
        let reporter = Arc::new(TerminalProgressReporter::new_for_test(true));
        let sink = Arc::new(VerboseTranscriptSink::new(reporter.clone(), vec![]));
        let observer = CliObserver::new(reporter.clone(), Some(sink));

        observer.on_event(ProgressEvent::PhaseStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            phase_index: 1,
            total_phases: 2,
            tasks: vec!["task-1".into()],
        });
        observer.on_event(ProgressEvent::TaskStarted {
            phase_id: 1,
            phase_name: "foundation".into(),
            task_name: "task-1".into(),
            task_index: 1,
            total_tasks: 1,
            agent: AgentKind::Claude("opus-4".into()),
        });

        let snapshot = reporter.snapshot();
        assert!(snapshot.contains("Phase 1/2: foundation"));
        assert!(snapshot.contains("Active: task-1 via Anthropic/opus-4"));
    }

    #[test]
    fn completed_units_flush_grouped_transcript_blocks() {
        let reporter = Arc::new(TerminalProgressReporter::new_for_test(true));
        let sink = VerboseTranscriptSink::new(reporter, vec![]);

        sink.on_event(&ProgressEvent::Transcript(sample_transcript(
            TranscriptKind::Executor,
        )));

        let blocks = sink.blocks();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("[Executor Transcript]"));
        assert!(blocks[0].contains("Prompt:"));
        assert!(blocks[0].contains("Response:"));
    }

    #[test]
    fn reviewer_verdicts_and_retry_feedback_appear_in_grouped_output() {
        let reporter = Arc::new(TerminalProgressReporter::new_for_test(true));
        let sink = VerboseTranscriptSink::new(reporter, vec![]);

        sink.on_event(&ProgressEvent::Transcript(sample_transcript(
            TranscriptKind::Reviewer,
        )));
        sink.on_event(&ProgressEvent::Transcript(sample_transcript(
            TranscriptKind::RetryFeedback,
        )));

        let blocks = sink.blocks();
        assert!(blocks
            .iter()
            .any(|block| block.contains("[Reviewer Transcript]")));
        assert!(blocks
            .iter()
            .any(|block| block.contains("[Retry Feedback]")));
    }

    #[test]
    fn redaction_hides_api_keys_and_common_secret_patterns() {
        let redacted = redact(
            "OPENAI_API_KEY=sk-test-secret\nAIzaSecretValue\ncustom secret-value",
            &["secret-value".into()],
        );

        assert!(!redacted.contains("sk-test-secret"));
        assert!(!redacted.contains("AIzaSecretValue"));
        assert!(!redacted.contains("secret-value"));
        assert!(redacted.contains("[REDACTED]"));
    }
}
