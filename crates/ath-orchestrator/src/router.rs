//! Task-to-agent routing with majority vote, fallback, and rationale.
//!
//! Routes each task to exactly one agent based on skill taxonomy match.
//! Uses majority vote across skill tags with priority tiebreaking
//! (Claude > Gemini > Codex) and circuit-breaker-aware fallback.

use std::collections::HashMap;
use std::mem;

use ath_types::agent::AgentKind;
use ath_types::plan::PhaseSpec;
use ath_types::project::SkillTag;

use crate::error::IsolationError;
use crate::taxonomy;

/// The result of routing a single task to an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingDecision {
    /// The agent selected for this task.
    pub agent: AgentKind,
    /// Human-readable explanation of why this agent was chosen.
    pub rationale: String,
}

/// Routes a single task to the best available agent via majority vote.
///
/// Algorithm:
/// 1. If `skill_tags` is empty, return `default_agent()`.
/// 2. Look up each tag in the routing table, counting votes per agent discriminant.
/// 3. Sort candidates by (vote_count desc, priority asc).
/// 4. Pick the first candidate passing the `available` check.
/// 5. If no candidates pass, return `AllAgentsUnavailable`.
pub fn route_task(
    skill_tags: &[SkillTag],
    table: &HashMap<String, AgentKind>,
    available: impl Fn(&AgentKind) -> bool,
) -> Result<RoutingDecision, IsolationError> {
    // Empty tags -> default
    if skill_tags.is_empty() {
        let agent = taxonomy::default_agent();
        return Ok(RoutingDecision {
            agent,
            rationale: "No skill tags; defaulting to Claude".into(),
        });
    }

    // Count votes by discriminant
    // Map: discriminant -> (representative AgentKind, vote count)
    let mut votes: HashMap<mem::Discriminant<AgentKind>, (AgentKind, usize)> = HashMap::new();

    let mut any_match = false;
    for tag in skill_tags {
        if let Some(agent) = taxonomy::lookup(&tag.0, table) {
            any_match = true;
            let disc = mem::discriminant(&agent);
            votes
                .entry(disc)
                .and_modify(|(_, count)| *count += 1)
                .or_insert((agent, 1));
        }
    }

    // No tags matched -> default
    if !any_match {
        let agent = taxonomy::default_agent();
        return Ok(RoutingDecision {
            agent,
            rationale: "No matching skill tags; defaulting to Claude".into(),
        });
    }

    // Sort candidates: highest votes first, then lowest priority (tiebreak)
    let mut candidates: Vec<(AgentKind, usize)> = votes.into_values().collect();
    candidates.sort_by(|(a, count_a), (b, count_b)| {
        count_b
            .cmp(count_a)
            .then_with(|| taxonomy::priority(a).cmp(&taxonomy::priority(b)))
    });

    // Build rationale with vote counts for all three agent kinds
    let tag_strs: Vec<&str> = skill_tags.iter().map(|t| t.0.as_str()).collect();
    let claude_votes = candidates
        .iter()
        .find(|(a, _)| matches!(a, AgentKind::Claude(_)))
        .map_or(0, |(_, c)| *c);
    let gemini_votes = candidates
        .iter()
        .find(|(a, _)| matches!(a, AgentKind::Gemini(_)))
        .map_or(0, |(_, c)| *c);
    let codex_votes = candidates
        .iter()
        .find(|(a, _)| matches!(a, AgentKind::Codex(_)))
        .map_or(0, |(_, c)| *c);

    // Pick first available candidate
    for (agent, _) in &candidates {
        if available(agent) {
            let rationale = format!(
                "Tags [{}] -> Claude ({} votes), Gemini ({} votes), Codex ({} votes)",
                tag_strs.join(", "),
                claude_votes,
                gemini_votes,
                codex_votes,
            );
            return Ok(RoutingDecision {
                agent: agent.clone(),
                rationale,
            });
        }
    }

    // All candidates unavailable — also try default if it wasn't a candidate
    let default = taxonomy::default_agent();
    if available(&default)
        && !candidates
            .iter()
            .any(|(a, _)| mem::discriminant(a) == mem::discriminant(&default))
    {
        return Ok(RoutingDecision {
            agent: default,
            rationale: format!(
                "Tags [{}] -> all matched agents unavailable; falling back to Claude (default)",
                tag_strs.join(", "),
            ),
        });
    }

    // Truly all unavailable
    let provider_names: Vec<&str> = candidates.iter().map(|(a, _)| a.provider_name()).collect();
    Err(IsolationError::AllAgentsUnavailable {
        providers: provider_names.join(", "),
        hint: "Check API keys and provider quotas".into(),
    })
}

/// Routes all tasks in a phase, setting each task's `assigned_agent` field.
///
/// Iterates over all tasks in the phase, calling `route_task` for each.
/// On success, every task will have `assigned_agent = Some(...)`.
/// Fails fast on the first routing error (e.g., all agents unavailable).
///
/// Returns the list of `RoutingDecision`s for verbose/diagnostic output.
pub fn assign_all_tasks(
    phase: &mut PhaseSpec,
    table: &HashMap<String, AgentKind>,
    available: impl Fn(&AgentKind) -> bool,
) -> Result<Vec<RoutingDecision>, IsolationError> {
    let mut decisions = Vec::with_capacity(phase.tasks.len());

    for task in &mut phase.tasks {
        let decision = route_task(&task.skill_tags, table, &available)?;
        task.assigned_agent = Some(decision.agent.clone());
        decisions.push(decision);
    }

    Ok(decisions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::build_routing_table;

    fn tags(names: &[&str]) -> Vec<SkillTag> {
        names.iter().map(|n| SkillTag(n.to_string())).collect()
    }

    // --- route_task tests ---

    #[test]
    fn route_rust_api_returns_claude_tiebreak() {
        // "rust" -> Claude, "api" -> Gemini => 1-1 tie, Claude wins by priority
        let table = build_routing_table();
        let result = route_task(&tags(&["rust", "api"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
    }

    #[test]
    fn route_rust_logic_api_returns_claude_majority() {
        // "rust" -> Claude, "logic" -> Claude, "api" -> Gemini => 2-1, Claude wins
        let table = build_routing_table();
        let result = route_task(&tags(&["rust", "logic", "api"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
    }

    #[test]
    fn route_docs_research_api_returns_gemini() {
        // "docs" -> Gemini, "research" -> Gemini, "api" -> Gemini => 3 Gemini
        let table = build_routing_table();
        let result = route_task(&tags(&["docs", "research", "api"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Gemini(_)));
    }

    #[test]
    fn route_codegen_boilerplate_returns_codex() {
        // "codegen" -> Codex, "boilerplate" -> Codex => 2 Codex
        let table = build_routing_table();
        let result = route_task(&tags(&["codegen", "boilerplate"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Codex(_)));
    }

    #[test]
    fn route_empty_tags_returns_claude_default() {
        let table = build_routing_table();
        let result = route_task(&tags(&[]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
        assert!(result.rationale.contains("defaulting to Claude"));
    }

    #[test]
    fn route_unknown_tags_returns_claude_default() {
        let table = build_routing_table();
        let result = route_task(&tags(&["unknown1", "unknown2"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
        assert!(result.rationale.contains("defaulting to Claude"));
    }

    #[test]
    fn route_fallback_when_best_unavailable() {
        // "rust", "logic" => Claude wins, but Claude unavailable => falls back to next
        let table = build_routing_table();
        // Also add an api tag so Gemini is a candidate
        let result = route_task(&tags(&["rust", "logic", "api"]), &table, |a| {
            !matches!(a, AgentKind::Claude(_))
        })
        .unwrap();
        assert!(matches!(result.agent, AgentKind::Gemini(_)));
    }

    #[test]
    fn route_all_agents_unavailable_returns_error() {
        let table = build_routing_table();
        let result = route_task(&tags(&["rust", "docs", "codegen"]), &table, |_| false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, IsolationError::AllAgentsUnavailable { .. }));
        assert_eq!(err.hint(), "Check API keys and provider quotas");
    }

    #[test]
    fn route_rationale_contains_tags_and_votes() {
        let table = build_routing_table();
        let result = route_task(&tags(&["rust", "logic", "api"]), &table, |_| true).unwrap();
        assert!(result.rationale.contains("Tags ["));
        assert!(result.rationale.contains("votes"));
    }

    #[test]
    fn route_discriminant_based_counting() {
        // Claude("opus-4") and Claude("sonnet-4") should count as same provider
        // We test this indirectly: the routing table uses Claude("opus-4"),
        // but the discriminant match means any Claude variant counts the same
        let table = build_routing_table();
        let result = route_task(&tags(&["rust", "architecture"]), &table, |_| true).unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
    }

    #[test]
    fn route_case_insensitive_tags() {
        let table = build_routing_table();
        let result = route_task(
            &[SkillTag("RUST".into()), SkillTag("Logic".into())],
            &table,
            |_| true,
        )
        .unwrap();
        assert!(matches!(result.agent, AgentKind::Claude(_)));
    }

    // --- assign_all_tasks tests ---

    use ath_types::plan::{PhaseSpec, TaskSpec};

    fn make_task(name: &str, skill_tags: &[&str]) -> TaskSpec {
        TaskSpec {
            name: name.into(),
            description: format!("{name} description"),
            skill_tags: tags(skill_tags),
            expected_output_files: vec![],
            acceptance_criteria: vec![],
            goal_indices: vec![],
            assigned_agent: None,
        }
    }

    fn make_phase(tasks: Vec<TaskSpec>) -> PhaseSpec {
        PhaseSpec {
            id: 1,
            name: "Test Phase".into(),
            description: "A test phase".into(),
            tasks,
            depends_on: vec![],
            produces: vec![],
            consumes: vec![],
        }
    }

    #[test]
    fn assign_all_tasks_sets_all_agents() {
        let table = build_routing_table();
        let mut phase = make_phase(vec![
            make_task("Task A", &["rust", "logic"]),
            make_task("Task B", &["docs", "research"]),
            make_task("Task C", &["codegen", "boilerplate"]),
        ]);

        let decisions = assign_all_tasks(&mut phase, &table, |_| true).unwrap();

        assert_eq!(decisions.len(), 3);
        // Every task should now have an assigned agent
        for task in &phase.tasks {
            assert!(
                task.assigned_agent.is_some(),
                "Task '{}' has no agent",
                task.name
            );
        }
    }

    #[test]
    fn assign_all_tasks_routes_to_correct_agents() {
        let table = build_routing_table();
        let mut phase = make_phase(vec![
            make_task("Rust task", &["rust", "logic"]),    // -> Claude
            make_task("Docs task", &["docs", "research"]), // -> Gemini
            make_task("Gen task", &["codegen", "boilerplate"]), // -> Codex
        ]);

        let decisions = assign_all_tasks(&mut phase, &table, |_| true).unwrap();

        assert!(matches!(
            phase.tasks[0].assigned_agent,
            Some(AgentKind::Claude(_))
        ));
        assert!(matches!(
            phase.tasks[1].assigned_agent,
            Some(AgentKind::Gemini(_))
        ));
        assert!(matches!(
            phase.tasks[2].assigned_agent,
            Some(AgentKind::Codex(_))
        ));

        // Decisions match task agents
        assert!(matches!(decisions[0].agent, AgentKind::Claude(_)));
        assert!(matches!(decisions[1].agent, AgentKind::Gemini(_)));
        assert!(matches!(decisions[2].agent, AgentKind::Codex(_)));
    }

    #[test]
    fn assign_all_tasks_error_stops_processing() {
        let table = build_routing_table();
        let mut phase = make_phase(vec![
            make_task("Good task", &["rust"]),
            make_task("Bad task", &["rust", "docs", "codegen"]), // all agents unavailable
            make_task("Never reached", &["docs"]),
        ]);

        // All agents unavailable
        let result = assign_all_tasks(&mut phase, &table, |_| false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IsolationError::AllAgentsUnavailable { .. }
        ));
    }
}
