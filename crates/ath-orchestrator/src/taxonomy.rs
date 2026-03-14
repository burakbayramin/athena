//! Skill taxonomy routing table.
//!
//! Maps skill tags to preferred agent kinds for task routing.
//! The routing table is the static foundation for agent assignment:
//! Claude handles architecture/logic, Gemini handles docs/research,
//! Codex handles code generation/scaffolding.

use std::collections::HashMap;

use ath_config::skills::SkillsConfig;
use ath_types::agent::AgentId;

/// Builds the static routing table mapping lowercase skill tag strings to agent kinds.
///
/// Mapping rationale:
/// - Claude: deep reasoning tasks (architecture, logic, systems design)
/// - Gemini: information processing tasks (docs, research, API analysis)
/// - Codex: mechanical code production tasks (scaffolding, boilerplate, templates)
pub fn build_routing_table() -> HashMap<String, AgentId> {
    let mut table = HashMap::new();

    // Claude: architecture and logic tasks
    let claude_tags = ["rust", "architecture", "logic", "systems", "design"];
    for tag in claude_tags {
        table.insert(tag.to_string(), AgentId::claude("opus-4"));
    }

    // Gemini: documentation and research tasks
    let gemini_tags = ["docs", "research", "api", "documentation", "analysis"];
    for tag in gemini_tags {
        table.insert(tag.to_string(), AgentId::gemini("2.5-pro"));
    }

    // Codex: code generation and scaffolding tasks
    let codex_tags = [
        "codegen",
        "boilerplate",
        "scaffolding",
        "template",
        "generation",
    ];
    for tag in codex_tags {
        table.insert(tag.to_string(), AgentId::codex("o3"));
    }

    table
}

/// Returns the priority rank for tiebreaking when multiple agents match.
///
/// Lower is higher priority: Claude(0) > Gemini(1) > Codex(2).
pub fn priority(agent: &AgentId) -> u8 {
    if agent.is_claude() {
        0
    } else if agent.is_gemini() {
        1
    } else if agent.is_codex() {
        2
    } else {
        3 // Custom providers get lowest priority
    }
}

/// Builds a routing table from the hardcoded defaults merged with an optional
/// skills config. Config routes override defaults for the same tag.
pub fn build_routing_table_with_config(skills: Option<&SkillsConfig>) -> HashMap<String, AgentId> {
    let mut table = build_routing_table();

    if let Some(config) = skills {
        for (tag, agent_ref) in &config.routes {
            table.insert(
                tag.to_lowercase(),
                AgentId::new(&agent_ref.provider, &agent_ref.model),
            );
        }
    }

    table
}

/// Looks up a skill tag in the routing table (case-insensitive).
///
/// Returns a cloned `AgentId` if the tag matches, or `None` if unrecognized.
/// The caller is responsible for handling the `None` case (e.g., defaulting to Claude).
pub fn lookup(tag: &str, table: &HashMap<String, AgentId>) -> Option<AgentId> {
    table.get(&tag.to_lowercase()).cloned()
}

/// Returns the default agent, optionally overridden by skills config.
pub fn default_agent_with_config(skills: Option<&SkillsConfig>) -> AgentId {
    if let Some(config) = skills {
        if let Some(ref default) = config.default {
            return AgentId::new(&default.provider, &default.model);
        }
    }
    default_agent()
}

/// Returns the default agent for unrecognized skill tags.
///
/// Per project decision: unrecognized tags default to Claude as the
/// strongest general-purpose model.
pub fn default_agent() -> AgentId {
    AgentId::claude("opus-4")
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
        assert!(table.get("rust").is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn routing_table_architecture_maps_to_claude() {
        let table = build_routing_table();
        assert!(table.get("architecture").is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn routing_table_logic_maps_to_claude() {
        let table = build_routing_table();
        assert!(table.get("logic").is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn routing_table_systems_maps_to_claude() {
        let table = build_routing_table();
        assert!(table.get("systems").is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn routing_table_design_maps_to_claude() {
        let table = build_routing_table();
        assert!(table.get("design").is_some_and(|a| a.is_claude()));
    }

    // Gemini tags
    #[test]
    fn routing_table_docs_maps_to_gemini() {
        let table = build_routing_table();
        assert!(table.get("docs").is_some_and(|a| a.is_gemini()));
    }

    #[test]
    fn routing_table_research_maps_to_gemini() {
        let table = build_routing_table();
        assert!(table.get("research").is_some_and(|a| a.is_gemini()));
    }

    #[test]
    fn routing_table_api_maps_to_gemini() {
        let table = build_routing_table();
        assert!(table.get("api").is_some_and(|a| a.is_gemini()));
    }

    #[test]
    fn routing_table_documentation_maps_to_gemini() {
        let table = build_routing_table();
        assert!(table.get("documentation").is_some_and(|a| a.is_gemini()));
    }

    #[test]
    fn routing_table_analysis_maps_to_gemini() {
        let table = build_routing_table();
        assert!(table.get("analysis").is_some_and(|a| a.is_gemini()));
    }

    // Codex tags
    #[test]
    fn routing_table_codegen_maps_to_codex() {
        let table = build_routing_table();
        assert!(table.get("codegen").is_some_and(|a| a.is_codex()));
    }

    #[test]
    fn routing_table_boilerplate_maps_to_codex() {
        let table = build_routing_table();
        assert!(table.get("boilerplate").is_some_and(|a| a.is_codex()));
    }

    #[test]
    fn routing_table_scaffolding_maps_to_codex() {
        let table = build_routing_table();
        assert!(table.get("scaffolding").is_some_and(|a| a.is_codex()));
    }

    #[test]
    fn routing_table_template_maps_to_codex() {
        let table = build_routing_table();
        assert!(table.get("template").is_some_and(|a| a.is_codex()));
    }

    #[test]
    fn routing_table_generation_maps_to_codex() {
        let table = build_routing_table();
        assert!(table.get("generation").is_some_and(|a| a.is_codex()));
    }

    // Lookup tests
    #[test]
    fn lookup_returns_agent_for_known_tag() {
        let table = build_routing_table();
        let result = lookup("rust", &table);
        assert!(result.is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn lookup_case_insensitive() {
        let table = build_routing_table();
        let result = lookup("RUST", &table);
        assert!(result.is_some_and(|a| a.is_claude()));
    }

    #[test]
    fn lookup_mixed_case() {
        let table = build_routing_table();
        let result = lookup("Research", &table);
        assert!(result.is_some_and(|a| a.is_gemini()));
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
        assert_eq!(priority(&AgentId::claude("opus-4")), 0);
    }

    #[test]
    fn priority_gemini_is_one() {
        assert_eq!(priority(&AgentId::gemini("2.5-pro")), 1);
    }

    #[test]
    fn priority_codex_is_two() {
        assert_eq!(priority(&AgentId::codex("o3")), 2);
    }

    #[test]
    fn priority_ordering_claude_beats_gemini() {
        let claude_p = priority(&AgentId::claude("opus-4"));
        let gemini_p = priority(&AgentId::gemini("2.5-pro"));
        assert!(claude_p < gemini_p);
    }

    #[test]
    fn priority_ordering_gemini_beats_codex() {
        let gemini_p = priority(&AgentId::gemini("2.5-pro"));
        let codex_p = priority(&AgentId::codex("o3"));
        assert!(gemini_p < codex_p);
    }

    // Default agent
    #[test]
    fn default_agent_is_claude() {
        assert!(default_agent().is_claude());
    }

    #[test]
    fn default_agent_model_is_opus_4() {
        assert_eq!(default_agent().model(), "opus-4");
    }

    // Config-driven routing

    #[test]
    fn config_overrides_existing_route() {
        use ath_config::skills::{AgentRef, SkillsConfig};
        let mut routes = std::collections::HashMap::new();
        routes.insert(
            "rust".to_string(),
            AgentRef {
                provider: "ollama".to_string(),
                model: "codestral".to_string(),
            },
        );
        let config = SkillsConfig {
            default: None,
            routes,
        };
        let table = build_routing_table_with_config(Some(&config));
        let rust_agent = table.get("rust").unwrap();
        assert_eq!(rust_agent.provider(), "ollama");
        assert_eq!(rust_agent.model(), "codestral");
    }

    #[test]
    fn config_adds_new_route() {
        use ath_config::skills::{AgentRef, SkillsConfig};
        let mut routes = std::collections::HashMap::new();
        routes.insert(
            "python".to_string(),
            AgentRef {
                provider: "ollama".to_string(),
                model: "llama3.3".to_string(),
            },
        );
        let config = SkillsConfig {
            default: None,
            routes,
        };
        let table = build_routing_table_with_config(Some(&config));
        let python_agent = table.get("python").unwrap();
        assert_eq!(python_agent.provider(), "ollama");
        // Original routes still present
        assert!(table.get("rust").unwrap().is_claude());
    }

    #[test]
    fn config_none_returns_defaults() {
        let table = build_routing_table_with_config(None);
        assert_eq!(table.len(), 15);
        assert!(table.get("rust").unwrap().is_claude());
    }

    #[test]
    fn default_agent_with_config_override() {
        use ath_config::skills::{AgentRef, SkillsConfig};
        let config = SkillsConfig {
            default: Some(AgentRef {
                provider: "ollama".to_string(),
                model: "llama3.3".to_string(),
            }),
            routes: std::collections::HashMap::new(),
        };
        let agent = default_agent_with_config(Some(&config));
        assert_eq!(agent.provider(), "ollama");
        assert_eq!(agent.model(), "llama3.3");
    }

    #[test]
    fn default_agent_with_config_none_returns_claude() {
        let agent = default_agent_with_config(None);
        assert!(agent.is_claude());
    }

    #[test]
    fn priority_custom_provider_is_three() {
        let custom = AgentId::new("ollama", "llama3.3");
        assert_eq!(priority(&custom), 3);
    }
}
