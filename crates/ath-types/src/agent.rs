//! Agent identification and inter-agent request/response types.
//!
//! `AgentId` provides extensible agent identification with provider + model strings.
//! `AgentRequest` and `AgentResponse` define the typed messages between orchestrator and agents.

use std::fmt;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Well-known provider names (lowercase canonical form)
const PROVIDER_ANTHROPIC: &str = "anthropic";
const PROVIDER_GOOGLE: &str = "google";
const PROVIDER_OPENAI: &str = "openai";

// Display names for well-known providers
const DISPLAY_ANTHROPIC: &str = "Anthropic";
const DISPLAY_GOOGLE: &str = "Google";
const DISPLAY_OPENAI: &str = "OpenAI";

/// Extensible agent identification with provider and model strings.
///
/// Replaces the former `AgentKind` enum. Any provider can be represented —
/// built-in providers (Anthropic, Google, OpenAI) have convenience constructors
/// and `is_*()` methods; custom providers use `AgentId::new(provider, model)`.
///
/// Provider names are stored in lowercase canonical form.
#[derive(Debug, Clone)]
pub struct AgentId {
    /// Lowercase canonical provider name (e.g., "anthropic", "google", "openai", "ollama")
    provider: String,
    /// Model identifier (e.g., "opus-4", "2.5-pro", "o3", "llama3.3")
    model: String,
}

impl AgentId {
    /// Create an AgentId with any provider and model.
    /// Provider is normalized to lowercase.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into().to_lowercase(),
            model: model.into(),
        }
    }

    /// Create an Anthropic Claude agent.
    pub fn claude(model: impl Into<String>) -> Self {
        Self::new(PROVIDER_ANTHROPIC, model)
    }

    /// Create a Google Gemini agent.
    pub fn gemini(model: impl Into<String>) -> Self {
        Self::new(PROVIDER_GOOGLE, model)
    }

    /// Create an OpenAI Codex agent.
    pub fn codex(model: impl Into<String>) -> Self {
        Self::new(PROVIDER_OPENAI, model)
    }

    /// The canonical lowercase provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Human-readable provider name for display.
    /// Returns capitalized name for well-known providers, raw name for custom.
    pub fn provider_name(&self) -> &str {
        match self.provider.as_str() {
            PROVIDER_ANTHROPIC => DISPLAY_ANTHROPIC,
            PROVIDER_GOOGLE => DISPLAY_GOOGLE,
            PROVIDER_OPENAI => DISPLAY_OPENAI,
            _ => &self.provider,
        }
    }

    /// The model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Is this an Anthropic Claude agent?
    pub fn is_claude(&self) -> bool {
        self.provider == PROVIDER_ANTHROPIC
    }

    /// Is this a Google Gemini agent?
    pub fn is_gemini(&self) -> bool {
        self.provider == PROVIDER_GOOGLE
    }

    /// Is this an OpenAI Codex agent?
    pub fn is_codex(&self) -> bool {
        self.provider == PROVIDER_OPENAI
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider_name(), self.model)
    }
}

impl PartialEq for AgentId {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider && self.model == other.model
    }
}

impl Eq for AgentId {}

impl Hash for AgentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.provider.hash(state);
        self.model.hash(state);
    }
}

// ---------------------------------------------------------------------------
// Serde: serialize as {"provider":"...","model":"..."}
// Deserialize supports both new format AND old AgentKind enum format.
// ---------------------------------------------------------------------------

impl Serialize for AgentId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AgentId", 2)?;
        s.serialize_field("provider", &self.provider)?;
        s.serialize_field("model", &self.model)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{self, MapAccess, Visitor};

        struct AgentIdVisitor;

        impl<'de> Visitor<'de> for AgentIdVisitor {
            type Value = AgentId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    r#"an AgentId like {"provider":"anthropic","model":"opus-4"} or legacy {"Claude":"opus-4"}"#,
                )
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<AgentId, A::Error> {
                let mut provider: Option<String> = None;
                let mut model: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        // New format fields
                        "provider" => provider = Some(map.next_value()?),
                        "model" => model = Some(map.next_value()?),
                        // Legacy AgentKind enum format: {"Claude":"opus-4"}
                        "Claude" => {
                            let m: String = map.next_value()?;
                            return Ok(AgentId::claude(m));
                        }
                        "Gemini" => {
                            let m: String = map.next_value()?;
                            return Ok(AgentId::gemini(m));
                        }
                        "Codex" => {
                            let m: String = map.next_value()?;
                            return Ok(AgentId::codex(m));
                        }
                        _ => {
                            // Unknown field — skip
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                let provider = provider.ok_or_else(|| de::Error::missing_field("provider"))?;
                let model = model.ok_or_else(|| de::Error::missing_field("model"))?;

                Ok(AgentId::new(provider, model))
            }
        }

        deserializer.deserialize_map(AgentIdVisitor)
    }
}

/// Deprecated type alias for backward compatibility during migration.
/// Use `AgentId` directly in new code.
#[deprecated(note = "Use AgentId instead")]
pub type AgentKind = AgentId;

/// Role in a conversation message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

/// A single message in a conversation history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    /// Rough token estimate: ~4 chars per token (conservative).
    pub fn estimated_tokens(&self) -> usize {
        self.content.len() / 4
    }
}

/// A typed request from the orchestrator to an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRequest {
    /// Unique request identifier for correlation.
    pub id: Uuid,
    /// The agent this request is destined for.
    pub agent: AgentId,
    /// The prompt to send to the agent.
    pub prompt: String,
    /// Optional additional context for the agent.
    pub context: Option<String>,
    /// Optional JSON schema for structured output (provider-native JSON mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
    /// Conversation history for multi-turn exchanges.
    ///
    /// When non-empty, these messages are prepended to the ChatRequest
    /// before the current prompt. The current prompt becomes the final
    /// user message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatMessage>,
    /// When this request was created.
    pub created_at: DateTime<Utc>,
}

/// A typed response from an agent back to the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentResponse {
    /// The ID of the request this responds to.
    pub request_id: Uuid,
    /// The agent that produced this response.
    pub agent: AgentId,
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

    // ---------------------------------------------------------------------------
    // AgentId basic tests
    // ---------------------------------------------------------------------------

    #[test]
    fn agent_id_claude_constructor() {
        let id = AgentId::claude("opus-4");
        assert_eq!(id.provider(), "anthropic");
        assert_eq!(id.model(), "opus-4");
        assert_eq!(id.provider_name(), "Anthropic");
        assert!(id.is_claude());
        assert!(!id.is_gemini());
        assert!(!id.is_codex());
    }

    #[test]
    fn agent_id_gemini_constructor() {
        let id = AgentId::gemini("2.5-pro");
        assert_eq!(id.provider(), "google");
        assert_eq!(id.model(), "2.5-pro");
        assert_eq!(id.provider_name(), "Google");
        assert!(id.is_gemini());
    }

    #[test]
    fn agent_id_codex_constructor() {
        let id = AgentId::codex("o3");
        assert_eq!(id.provider(), "openai");
        assert_eq!(id.model(), "o3");
        assert_eq!(id.provider_name(), "OpenAI");
        assert!(id.is_codex());
    }

    #[test]
    fn agent_id_custom_provider() {
        let id = AgentId::new("ollama", "llama3.3");
        assert_eq!(id.provider(), "ollama");
        assert_eq!(id.model(), "llama3.3");
        assert_eq!(id.provider_name(), "ollama"); // custom returns raw
        assert!(!id.is_claude());
        assert!(!id.is_gemini());
        assert!(!id.is_codex());
    }

    #[test]
    fn agent_id_provider_normalized_to_lowercase() {
        let id = AgentId::new("Anthropic", "opus-4");
        assert_eq!(id.provider(), "anthropic");
        assert!(id.is_claude());
    }

    #[test]
    fn agent_id_display() {
        let id = AgentId::claude("opus-4");
        assert_eq!(format!("{id}"), "Anthropic/opus-4");

        let custom = AgentId::new("ollama", "llama3.3");
        assert_eq!(format!("{custom}"), "ollama/llama3.3");
    }

    #[test]
    fn agent_id_equality() {
        let a = AgentId::claude("opus-4");
        let b = AgentId::claude("opus-4");
        assert_eq!(a, b);

        let c = AgentId::claude("sonnet-4");
        assert_ne!(a, c);

        let d = AgentId::gemini("opus-4"); // same model, different provider
        assert_ne!(a, d);
    }

    #[test]
    fn agent_id_hash_consistent() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(AgentId::claude("opus-4"));
        set.insert(AgentId::claude("opus-4")); // duplicate
        assert_eq!(set.len(), 1);

        set.insert(AgentId::gemini("2.5-pro"));
        assert_eq!(set.len(), 2);
    }

    // ---------------------------------------------------------------------------
    // Serde tests
    // ---------------------------------------------------------------------------

    #[test]
    fn agent_id_serialize_new_format() {
        let id = AgentId::claude("opus-4");
        let json = serde_json::to_string(&id).unwrap();
        assert!(json.contains(r#""provider":"anthropic""#));
        assert!(json.contains(r#""model":"opus-4""#));
    }

    #[test]
    fn agent_id_deserialize_new_format() {
        let json = r#"{"provider":"anthropic","model":"opus-4"}"#;
        let id: AgentId = serde_json::from_str(json).unwrap();
        assert_eq!(id.provider(), "anthropic");
        assert_eq!(id.model(), "opus-4");
        assert!(id.is_claude());
    }

    #[test]
    fn agent_id_deserialize_legacy_claude() {
        let json = r#"{"Claude":"opus-4"}"#;
        let id: AgentId = serde_json::from_str(json).unwrap();
        assert!(id.is_claude());
        assert_eq!(id.model(), "opus-4");
    }

    #[test]
    fn agent_id_deserialize_legacy_gemini() {
        let json = r#"{"Gemini":"2.5-pro"}"#;
        let id: AgentId = serde_json::from_str(json).unwrap();
        assert!(id.is_gemini());
        assert_eq!(id.model(), "2.5-pro");
    }

    #[test]
    fn agent_id_deserialize_legacy_codex() {
        let json = r#"{"Codex":"o3"}"#;
        let id: AgentId = serde_json::from_str(json).unwrap();
        assert!(id.is_codex());
        assert_eq!(id.model(), "o3");
    }

    #[test]
    fn agent_id_round_trip() {
        let original = AgentId::claude("opus-4");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn agent_id_custom_round_trip() {
        let original = AgentId::new("ollama", "llama3.3");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    // ---------------------------------------------------------------------------
    // Request/Response tests
    // ---------------------------------------------------------------------------

    #[test]
    fn agent_request_round_trip() {
        let req = AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentId::claude("opus-4"),
            prompt: "Analyze this code".into(),
            context: Some("src/main.rs contents".into()),
            json_schema: None,
            messages: vec![],
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
            agent: AgentId::claude("opus-4"),
            prompt: "test".into(),
            context: None,
            json_schema: None,
            messages: vec![],
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(!json.contains("json_schema"));
        let deserialized: AgentRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req, deserialized);
    }

    #[test]
    fn agent_request_json_schema_round_trip() {
        let req = AgentRequest {
            id: Uuid::new_v4(),
            agent: AgentId::claude("opus-4"),
            prompt: "test".into(),
            context: None,
            json_schema: Some(serde_json::json!({"type": "object"})),
            messages: vec![],
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
            agent: AgentId::gemini("2.5-pro"),
            content: "Analysis complete. No issues found.".into(),
            input_tokens: 1500,
            output_tokens: 200,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&resp).expect("serialize");
        let deserialized: AgentResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resp, deserialized);
    }

    // ---------------------------------------------------------------------------
    // Backward compat: deserialize old request/response with legacy AgentKind
    // ---------------------------------------------------------------------------

    #[test]
    fn agent_request_deserialize_with_legacy_agent() {
        // Simulates a request serialized with the old AgentKind format
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "agent": {"Claude": "opus-4"},
            "prompt": "test",
            "context": null,
            "created_at": "2026-03-14T00:00:00Z"
        }"#;
        let req: AgentRequest = serde_json::from_str(json).unwrap();
        assert!(req.agent.is_claude());
        assert_eq!(req.agent.model(), "opus-4");
    }
}
