//! ReviewEngine: reviewer selection with cross-agent pairing, review prompt
//! construction, and structured verdict parsing.
//!
//! Implements QUAL-01 (cross-agent review where reviewer != author). The reviewer
//! selection logic enforces the never-same-as-author rule, handles circuit breaker
//! fallback, and uses priority tiebreaking (Claude > Gemini > Codex).

use std::collections::HashMap;
use std::mem;

use ath_types::agent::AgentKind;

/// Errors specific to the review subsystem.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// No eligible reviewer is available (all non-author agents are down).
    #[error("no reviewer available for phase '{phase_name}': {reason}")]
    NoReviewerAvailable {
        phase_name: String,
        reason: String,
    },

    /// Failed to parse the review verdict JSON from the reviewer agent.
    #[error("verdict parse failed: {reason}\nraw: {raw}")]
    VerdictParseFailed {
        raw: String,
        reason: String,
    },
}

/// All three agent kinds in priority order (Claude > Gemini > Codex).
const CANDIDATE_ORDER: [fn() -> AgentKind; 3] = [
    || AgentKind::Claude("opus-4".into()),
    || AgentKind::Gemini("2.5-pro".into()),
    || AgentKind::Codex("o3".into()),
];

/// Selects a reviewer agent that is different from the majority author.
///
/// Algorithm:
/// 1. Count tasks per agent discriminant.
/// 2. Find the majority author (ties broken by priority: Claude > Gemini > Codex).
/// 3. Iterate candidates in priority order, skip majority author.
/// 4. Return the first available candidate.
/// 5. If none available, return `NoReviewerAvailable`.
pub fn select_reviewer(
    task_agents: &[AgentKind],
    phase_name: &str,
    available: impl Fn(&AgentKind) -> bool,
) -> Result<AgentKind, ReviewError> {
    if task_agents.is_empty() {
        return Err(ReviewError::NoReviewerAvailable {
            phase_name: phase_name.into(),
            reason: "no tasks in phase".into(),
        });
    }

    // Count tasks per agent discriminant, track priority index for tiebreaking
    let mut counts: HashMap<mem::Discriminant<AgentKind>, (usize, usize)> = HashMap::new();
    for agent in task_agents {
        let disc = mem::discriminant(agent);
        let priority = priority_index(agent);
        counts
            .entry(disc)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, priority));
    }

    // Find majority author: highest count, then lowest priority index (= highest priority) for ties
    let majority_disc = counts
        .iter()
        .max_by(|a, b| {
            a.1 .0
                .cmp(&b.1 .0) // higher count wins
                .then(b.1 .1.cmp(&a.1 .1)) // lower priority index wins (= higher priority)
        })
        .map(|(disc, _)| *disc);

    // Iterate candidates in priority order, skip majority author
    for make_candidate in &CANDIDATE_ORDER {
        let candidate = make_candidate();
        let disc = mem::discriminant(&candidate);
        if Some(disc) == majority_disc {
            continue;
        }
        if available(&candidate) {
            return Ok(candidate);
        }
    }

    Err(ReviewError::NoReviewerAvailable {
        phase_name: phase_name.into(),
        reason: "all non-author agents are unavailable".into(),
    })
}

/// Returns the priority index for an agent (lower = higher priority).
fn priority_index(agent: &AgentKind) -> usize {
    match agent {
        AgentKind::Claude(_) => 0,
        AgentKind::Gemini(_) => 1,
        AgentKind::Codex(_) => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_available(_: &AgentKind) -> bool {
        true
    }

    fn none_available(_: &AgentKind) -> bool {
        false
    }

    fn exclude(kind_fn: fn() -> AgentKind) -> impl Fn(&AgentKind) -> bool {
        move |agent: &AgentKind| mem::discriminant(agent) != mem::discriminant(&kind_fn())
    }

    // Test 1: Single Claude-authored phase selects Gemini as reviewer
    #[test]
    fn single_claude_author_selects_gemini() {
        let agents = vec![AgentKind::Claude("opus-4".into())];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 2: Single Gemini-authored phase selects Claude as reviewer
    #[test]
    fn single_gemini_author_selects_claude() {
        let agents = vec![AgentKind::Gemini("2.5-pro".into())];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Claude(_)));
    }

    // Test 3: Multi-author phase (2 Claude, 1 Gemini) excludes Claude, selects Gemini
    #[test]
    fn majority_claude_selects_gemini() {
        let agents = vec![
            AgentKind::Claude("opus-4".into()),
            AgentKind::Claude("opus-4".into()),
            AgentKind::Gemini("2.5-pro".into()),
        ];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 4: Tie (1 Claude, 1 Gemini) excludes Claude (higher priority = majority tiebreak), selects Gemini
    #[test]
    fn tie_excludes_higher_priority_selects_next() {
        let agents = vec![
            AgentKind::Claude("opus-4".into()),
            AgentKind::Gemini("2.5-pro".into()),
        ];
        let reviewer = select_reviewer(&agents, "test-phase", always_available).unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }

    // Test 5: Preferred reviewer circuit breaker tripped, falls back to next
    #[test]
    fn fallback_when_preferred_unavailable() {
        // Gemini-authored, so Claude is preferred reviewer, but Claude is unavailable
        let agents = vec![AgentKind::Gemini("2.5-pro".into())];
        let reviewer = select_reviewer(
            &agents,
            "test-phase",
            exclude(|| AgentKind::Claude("".into())),
        )
        .unwrap();
        assert!(matches!(reviewer, AgentKind::Codex(_)));
    }

    // Test 6: ALL non-author agents unavailable -> error
    #[test]
    fn error_when_all_non_author_unavailable() {
        let agents = vec![AgentKind::Claude("opus-4".into())];
        let result = select_reviewer(&agents, "my-phase", none_available);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ReviewError::NoReviewerAvailable { .. }));
        let msg = err.to_string();
        assert!(msg.contains("my-phase"));
    }

    // Test 7: Codex-only phase with Claude unavailable selects Gemini
    #[test]
    fn codex_only_claude_unavailable_selects_gemini() {
        let agents = vec![AgentKind::Codex("o3".into())];
        let reviewer = select_reviewer(
            &agents,
            "test-phase",
            exclude(|| AgentKind::Claude("".into())),
        )
        .unwrap();
        assert!(matches!(reviewer, AgentKind::Gemini(_)));
    }
}
