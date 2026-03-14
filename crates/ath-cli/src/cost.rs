use ath_types::agent::AgentId;

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
    agent: &AgentId,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<f64, UnsupportedPricing> {
    let pricing = pricing_for(agent)?;
    Ok(
        (input_tokens as f64 / 1_000_000.0) * pricing.input_per_million
            + (output_tokens as f64 / 1_000_000.0) * pricing.output_per_million,
    )
}

fn pricing_for(agent: &AgentId) -> Result<ModelPricing, UnsupportedPricing> {
    let model = agent.model();

    if agent.is_claude() {
        match model {
            "opus-4" | "claude-opus-4" => return Ok(ModelPricing {
                input_per_million: 15.0,
                output_per_million: 75.0,
            }),
            "sonnet-4" | "claude-sonnet-4" => return Ok(ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
            }),
            _ => {}
        }
    }

    if agent.is_gemini() {
        match model {
            "2.5-pro" | "gemini-2.5-pro" => return Ok(ModelPricing {
                // Gemini 2.5 Pro has tiered long-context pricing, but Athena's saved
                // report data does not retain request-level context size. Use the
                // standard <=200K-input-token rate explicitly as the base estimate.
                input_per_million: 1.25,
                output_per_million: 10.0,
            }),
            _ => {}
        }
    }

    if agent.is_codex() && model == "o3" {
        return Ok(ModelPricing {
            input_per_million: 2.0,
            output_per_million: 8.0,
        });
    }

    Err(UnsupportedPricing {
        provider: agent.provider_name().to_string(),
        model: agent.model().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_produce_deterministic_costs() {
        let claude_cost =
            estimate_cost(&AgentId::claude("opus-4"), 1_000_000, 500_000).expect("claude");
        let gemini_cost = estimate_cost(&AgentId::gemini("2.5-pro"), 1_000_000, 500_000)
            .expect("gemini");
        let openai_cost =
            estimate_cost(&AgentId::codex("o3"), 1_000_000, 500_000).expect("o3");

        assert_eq!(claude_cost, 52.5);
        assert_eq!(gemini_cost, 6.25);
        assert_eq!(openai_cost, 6.0);
    }

    #[test]
    fn unknown_models_return_explicit_unsupported_pricing() {
        let error =
            estimate_cost(&AgentId::codex("unknown-model"), 1_000, 1_000).unwrap_err();

        assert_eq!(
            error,
            UnsupportedPricing {
                provider: "OpenAI".into(),
                model: "unknown-model".into(),
            }
        );
    }
}
