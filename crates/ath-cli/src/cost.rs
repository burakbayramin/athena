use ath_types::agent::AgentKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedPricing {
    pub(crate) provider: String,
    pub(crate) model: String,
}

#[derive(Debug, Clone, Copy)]
struct ModelPricing {
    input_per_million: f64,
    output_per_million: f64,
}

pub(crate) fn estimate_cost(
    agent: &AgentKind,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<f64, UnsupportedPricing> {
    let pricing = pricing_for(agent)?;
    Ok(
        (input_tokens as f64 / 1_000_000.0) * pricing.input_per_million
            + (output_tokens as f64 / 1_000_000.0) * pricing.output_per_million,
    )
}

fn pricing_for(agent: &AgentKind) -> Result<ModelPricing, UnsupportedPricing> {
    match agent {
        AgentKind::Claude(model) if matches!(model.as_str(), "opus-4" | "claude-opus-4") => {
            Ok(ModelPricing {
                input_per_million: 15.0,
                output_per_million: 75.0,
            })
        }
        AgentKind::Claude(model) if matches!(model.as_str(), "sonnet-4" | "claude-sonnet-4") => {
            Ok(ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
            })
        }
        // Gemini 2.5 Pro has tiered long-context pricing, but Athena's saved
        // report data does not retain request-level context size. Use the
        // standard <=200K-input-token rate explicitly as the base estimate.
        AgentKind::Gemini(model) if matches!(model.as_str(), "2.5-pro" | "gemini-2.5-pro") => {
            Ok(ModelPricing {
                input_per_million: 1.25,
                output_per_million: 10.0,
            })
        }
        AgentKind::Codex(model) if model == "o3" => Ok(ModelPricing {
            input_per_million: 2.0,
            output_per_million: 8.0,
        }),
        other => Err(UnsupportedPricing {
            provider: other.provider_name().to_string(),
            model: other.model().to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_produce_deterministic_costs() {
        let claude_cost =
            estimate_cost(&AgentKind::Claude("opus-4".into()), 1_000_000, 500_000).expect("claude");
        let gemini_cost = estimate_cost(&AgentKind::Gemini("2.5-pro".into()), 1_000_000, 500_000)
            .expect("gemini");
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
