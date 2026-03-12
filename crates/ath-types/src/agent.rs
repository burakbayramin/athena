//! Agent identification and inter-agent request/response types.
//!
//! `AgentKind` provides type-safe agent identification with model variant strings.
//! `AgentRequest` and `AgentResponse` define the typed messages between orchestrator and agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type-safe agent identification carrying the model variant string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentKind {
    /// Anthropic Claude model (e.g., "opus-4")
    Claude(String),
    /// Google Gemini model (e.g., "2.5-pro")
    Gemini(String),
    /// OpenAI Codex model (e.g., "o3")
    Codex(String),
}

impl AgentKind {
    /// Returns the provider name for this agent kind.
    pub fn provider_name(&self) -> &str {
        match self {
            AgentKind::Claude(_) => "Anthropic",
            AgentKind::Gemini(_) => "Google",
            AgentKind::Codex(_) => "OpenAI",
        }
    }

    /// Returns the model variant string.
    pub fn model(&self) -> &str {
        match self {
            AgentKind::Claude(m) | AgentKind::Gemini(m) | AgentKind::Codex(m) => m,
        }
    }
}

/// A typed request from the orchestrator to an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRequest {
    /// Unique request identifier for correlation.
    pub id: Uuid,
    /// The agent this request is destined for.
    pub agent: AgentKind,
    /// The prompt to send to the agent.
    pub prompt: String,
    /// Optional additional context for the agent.
    pub context: Option<String>,
    /// Optional JSON schema for structured output (provider-native JSON mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
    /// When this request was created.
    pub created_at: DateTime<Utc>,
}

/// A typed response from an agent back to the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentResponse {
    /// The ID of the request this responds to.
    pub request_id: Uuid,
    /// The agent that produced this response.
    pub agent: AgentKind,
    /// The content of the response.
    pub content: String,
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens produced.
    pub output_tokens: u64,
    /// When this response was created.
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_kind_claude_serializes_and_deserializes() {
        let agent = AgentKind::Claude("opus-4".into());
        let json = serde_json::to_string(&agent).expect("serialize");
        let deserialized: AgentKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(agent, deserialized);
    }

    #[test]
    fn agent_kind_provider_name_claude() {
        assert_eq!(AgentKind::Claude("opus-4".into()).provider_name(), "Anthropic");
    }

    #[test]
    fn agent_kind_provider_name_gemini() {
        assert_eq!(AgentKind::Gemini("2.5-pro".into()).provider_name(), "Google");
    }

    #[test]
    fn agent_kind_provider_name_codex() {
        assert_eq!(AgentKind::Codex("o3".into()).provider_name(), "OpenAI");
    }

    #[test]
    fn agent_request_round_trip() {
        let req = AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentKind::Claude("opus-4".into()),
            prompt: "Analyze this code".into(),
            context: Some("src/main.rs contents".into()),
            json_schema: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&req).expect("serialize");
        let deserialized: AgentRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[test]
    fn agent_request_json_schema_none_backward_compatible() {
        let req = AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentKind::Claude("opus-4".into()),
            prompt: "test".into(),
            context: None,
            json_schema: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        // json_schema: None should not appear in serialized output
        assert!(!json.contains("json_schema"));
        let deserialized: AgentRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[test]
    fn agent_request_json_schema_round_trip() {
        let req = AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentKind::Claude("opus-4".into()),
            prompt: "test".into(),
            context: None,
            json_schema: Some(serde_json::json!({"type": "object"})),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains("json_schema"));
        let deserialized: AgentRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[test]
    fn agent_response_round_trip() {
        let resp = AgentResponse {
            request_id: Uuid::new_v4(),
            agent: AgentKind::Gemini("2.5-pro".into()),
            content: "Analysis complete. No issues found.".into(),
            input_tokens: 1500,
            output_tokens: 200,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&resp).expect("serialize");
        let deserialized: AgentResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resp, deserialized);
    }
}
