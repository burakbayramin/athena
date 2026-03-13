//! Skill taxonomy routing table.
//!
//! Maps skill tags to preferred agent kinds for task routing.
//! The routing table is the static foundation for agent assignment:
//! Claude handles architecture/logic, Gemini handles docs/research,
//! Codex handles code generation/scaffolding.

use std::collections::HashMap;

use ath_types::agent::AgentKind;

/// Builds the static routing table mapping lowercase skill tag strings to agent kinds.
///
/// Mapping rationale:
/// - Claude: deep reasoning tasks (architecture, logic, systems design)
/// - Gemini: information processing tasks (docs, research, API analysis)
/// - Codex: mechanical code production tasks (scaffolding, boilerplate, templates)
pub fn build_routing_table() -> HashMap<String, AgentKind> {
    let mut table = HashMap::new();

    // Claude: architecture and logic tasks
    let claude_tags = ["rust", "architecture", "logic", "systems", "design"];
    for tag in claude_tags {
        table.insert(tag.to_string(), AgentKind::Claude("opus-4".into()));
    }

    // Gemini: documentation and research tasks
    let gemini_tags = ["docs", "research", "api", "documentation", "analysis"];
    for tag in gemini_tags {
        table.insert(tag.to_string(), AgentKind::Gemini("2.5-pro".into()));
    }

    // Codex: code generation and scaffolding tasks
    let codex_tags = ["codegen", "boilerplate", "scaffolding", "template", "generation"];
    for tag in codex_tags {
        table.insert(tag.to_string(), AgentKind::Codex("o3".into()));
    }

    table
}

/// Returns the priority rank for tiebreaking when multiple agents match.
///
/// Lower is higher priority: Claude(0) > Gemini(1) > Codex(2).
pub fn priority(agent: &AgentKind) -> u8 {
    match agent {
        AgentKind::Claude(_) => 0,
        AgentKind::Gemini(_) => 1,
        AgentKind::Codex(_) => 2,
    }
}

/// Looks up a skill tag in the routing table (case-insensitive).
///
/// Returns a cloned `AgentKind` if the tag matches, or `None` if unrecognized.
/// The caller is responsible for handling the `None` case (e.g., defaulting to Claude).
pub fn lookup(tag: &str, table: &HashMap<String, AgentKind>) -> Option<AgentKind> {
    table.get(&tag.to_lowercase()).cloned()
}

/// Returns the default agent for unrecognized skill tags.
///
/// Per project decision: unrecognized tags default to Claude as the
/// strongest general-purpose model.
pub fn default_agent() -> AgentKind {
    AgentKind::Claude("opus-4".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_table_has_15_entries() {
        let table = build_routing_table();
        assert_eq!(table.len(), 15);
    }

    // Claude tags
    #[test]
    fn routing_table_rust_maps_to_claude() {
        let table = build_routing_table();
        assert!(matches!(table.get("rust"), Some(AgentKind::Claude(_))));
    }

    #[test]
    fn routing_table_architecture_maps_to_claude() {
        let table = build_routing_table();
        assert!(matches!(table.get("architecture"), Some(AgentKind::Claude(_))));
    }

    #[test]
    fn routing_table_logic_maps_to_claude() {
        let table = build_routing_table();
        assert!(matches!(table.get("logic"), Some(AgentKind::Claude(_))));
    }

    #[test]
    fn routing_table_systems_maps_to_claude() {
        let table = build_routing_table();
        assert!(matches!(table.get("systems"), Some(AgentKind::Claude(_))));
    }

    #[test]
    fn routing_table_design_maps_to_claude() {
        let table = build_routing_table();
        assert!(matches!(table.get("design"), Some(AgentKind::Claude(_))));
    }

    // Gemini tags
    #[test]
    fn routing_table_docs_maps_to_gemini() {
        let table = build_routing_table();
        assert!(matches!(table.get("docs"), Some(AgentKind::Gemini(_))));
    }

    #[test]
    fn routing_table_research_maps_to_gemini() {
        let table = build_routing_table();
        assert!(matches!(table.get("research"), Some(AgentKind::Gemini(_))));
    }

    #[test]
    fn routing_table_api_maps_to_gemini() {
        let table = build_routing_table();
        assert!(matches!(table.get("api"), Some(AgentKind::Gemini(_))));
    }

    #[test]
    fn routing_table_documentation_maps_to_gemini() {
        let table = build_routing_table();
        assert!(matches!(table.get("documentation"), Some(AgentKind::Gemini(_))));
    }

    #[test]
    fn routing_table_analysis_maps_to_gemini() {
        let table = build_routing_table();
        assert!(matches!(table.get("analysis"), Some(AgentKind::Gemini(_))));
    }

    // Codex tags
    #[test]
    fn routing_table_codegen_maps_to_codex() {
        let table = build_routing_table();
        assert!(matches!(table.get("codegen"), Some(AgentKind::Codex(_))));
    }

    #[test]
    fn routing_table_boilerplate_maps_to_codex() {
        let table = build_routing_table();
        assert!(matches!(table.get("boilerplate"), Some(AgentKind::Codex(_))));
    }

    #[test]
    fn routing_table_scaffolding_maps_to_codex() {
        let table = build_routing_table();
        assert!(matches!(table.get("scaffolding"), Some(AgentKind::Codex(_))));
    }

    #[test]
    fn routing_table_template_maps_to_codex() {
        let table = build_routing_table();
        assert!(matches!(table.get("template"), Some(AgentKind::Codex(_))));
    }

    #[test]
    fn routing_table_generation_maps_to_codex() {
        let table = build_routing_table();
        assert!(matches!(table.get("generation"), Some(AgentKind::Codex(_))));
    }

    // Lookup tests
    #[test]
    fn lookup_returns_agent_for_known_tag() {
        let table = build_routing_table();
        let result = lookup("rust", &table);
        assert!(matches!(result, Some(AgentKind::Claude(_))));
    }

    #[test]
    fn lookup_case_insensitive() {
        let table = build_routing_table();
        let result = lookup("RUST", &table);
        assert!(matches!(result, Some(AgentKind::Claude(_))));
    }

    #[test]
    fn lookup_mixed_case() {
        let table = build_routing_table();
        let result = lookup("Research", &table);
        assert!(matches!(result, Some(AgentKind::Gemini(_))));
    }

    #[test]
    fn lookup_unknown_tag_returns_none() {
        let table = build_routing_table();
        let result = lookup("unknown_tag", &table);
        assert!(result.is_none());
    }

    // Priority tests
    #[test]
    fn priority_claude_is_zero() {
        assert_eq!(priority(&AgentKind::Claude("opus-4".into())), 0);
    }

    #[test]
    fn priority_gemini_is_one() {
        assert_eq!(priority(&AgentKind::Gemini("2.5-pro".into())), 1);
    }

    #[test]
    fn priority_codex_is_two() {
        assert_eq!(priority(&AgentKind::Codex("o3".into())), 2);
    }

    #[test]
    fn priority_ordering_claude_beats_gemini() {
        let claude_p = priority(&AgentKind::Claude("opus-4".into()));
        let gemini_p = priority(&AgentKind::Gemini("2.5-pro".into()));
        assert!(claude_p < gemini_p);
    }

    #[test]
    fn priority_ordering_gemini_beats_codex() {
        let gemini_p = priority(&AgentKind::Gemini("2.5-pro".into()));
        let codex_p = priority(&AgentKind::Codex("o3".into()));
        assert!(gemini_p < codex_p);
    }

    // Default agent
    #[test]
    fn default_agent_is_claude() {
        assert!(matches!(default_agent(), AgentKind::Claude(_)));
    }

    #[test]
    fn default_agent_model_is_opus_4() {
        assert_eq!(default_agent().model(), "opus-4");
    }
}
