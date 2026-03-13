#[cfg(test)]
mod tests {
    use ath_types::agent::AgentKind;

    use super::*;

    #[test]
    fn known_models_produce_deterministic_costs() {
        let claude_cost =
            estimate_cost(&AgentKind::Claude("opus-4".into()), 1_000_000, 500_000).expect("claude");
        let gemini_cost =
            estimate_cost(&AgentKind::Gemini("2.5-pro".into()), 1_000_000, 500_000).expect("gemini");
        let openai_cost =
            estimate_cost(&AgentKind::Codex("o3".into()), 1_000_000, 500_000).expect("o3");

        assert_eq!(claude_cost, 52.5);
        assert_eq!(gemini_cost, 6.25);
        assert_eq!(openai_cost, 6.0);
    }

    #[test]
    fn unknown_models_return_explicit_unsupported_pricing() {
        let error =
            estimate_cost(&AgentKind::Codex("unknown-model".into()), 1_000, 1_000).unwrap_err();

        assert_eq!(
            error,
            UnsupportedPricing {
                provider: "OpenAI".into(),
                model: "unknown-model".into(),
            }
        );
    }
}
