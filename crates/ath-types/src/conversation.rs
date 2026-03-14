//! Conversation history builder with token budget truncation.
//!
//! Accumulates user/assistant message pairs during multi-turn interactions.
//! When the estimated token count exceeds the budget, older messages are
//! dropped (keeping the first exchange for context continuity).

use crate::agent::ChatMessage;

/// Default token budget for conversation history (32K tokens).
const DEFAULT_TOKEN_BUDGET: usize = 32_000;

/// Builds and manages conversation history with token budget enforcement.
///
/// Messages are accumulated via `push_user` / `push_assistant`. When the
/// total estimated tokens exceed the budget, the oldest messages are
/// dropped — keeping the first user/assistant pair for context continuity.
#[derive(Debug, Clone)]
pub struct ConversationBuilder {
    messages: Vec<ChatMessage>,
    token_budget: usize,
}

impl Default for ConversationBuilder {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            token_budget: DEFAULT_TOKEN_BUDGET,
        }
    }
}

impl ConversationBuilder {
    /// Create a builder with a specific token budget.
    pub fn with_budget(token_budget: usize) -> Self {
        Self {
            messages: Vec::new(),
            token_budget,
        }
    }

    /// Add a user message.
    pub fn push_user(&mut self, content: impl Into<String>) {
        self.messages.push(ChatMessage::user(content));
        self.truncate_if_needed();
    }

    /// Add an assistant message.
    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.messages.push(ChatMessage::assistant(content));
        self.truncate_if_needed();
    }

    /// Add a message of any role.
    pub fn push(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.truncate_if_needed();
    }

    /// Current estimated token count.
    pub fn estimated_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.estimated_tokens()).sum()
    }

    /// Number of messages in the conversation.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the conversation is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the messages as a slice (for passing to AgentRequest).
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Consume the builder and return the messages.
    pub fn into_messages(self) -> Vec<ChatMessage> {
        self.messages
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Truncate conversation to fit within token budget.
    ///
    /// Strategy: keep the first two messages (initial user/assistant pair for
    /// context continuity) and the most recent messages. Drop messages from
    /// the middle until under budget.
    fn truncate_if_needed(&mut self) {
        if self.estimated_tokens() <= self.token_budget {
            return;
        }

        // Need at least 3 messages to truncate (keep first pair + something recent)
        if self.messages.len() <= 2 {
            return;
        }

        // Keep first 2 messages (initial context) and remove from index 2
        // until we're under budget or only 2 messages remain
        while self.messages.len() > 2 && self.estimated_tokens() > self.token_budget {
            self.messages.remove(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ChatRole;

    #[test]
    fn empty_builder() {
        let builder = ConversationBuilder::default();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.estimated_tokens(), 0);
    }

    #[test]
    fn push_messages_and_retrieve() {
        let mut builder = ConversationBuilder::default();
        builder.push_user("Hello");
        builder.push_assistant("Hi there!");

        assert_eq!(builder.len(), 2);
        assert!(!builder.is_empty());
        assert_eq!(builder.messages()[0].role, ChatRole::User);
        assert_eq!(builder.messages()[0].content, "Hello");
        assert_eq!(builder.messages()[1].role, ChatRole::Assistant);
        assert_eq!(builder.messages()[1].content, "Hi there!");
    }

    #[test]
    fn into_messages_consumes_builder() {
        let mut builder = ConversationBuilder::default();
        builder.push_user("msg1");
        builder.push_assistant("msg2");

        let messages = builder.into_messages();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn token_estimation() {
        let mut builder = ConversationBuilder::default();
        // "Hello World" = 11 chars → ~2 tokens (11/4 = 2)
        builder.push_user("Hello World");
        assert_eq!(builder.estimated_tokens(), 2);
    }

    #[test]
    fn truncation_drops_middle_messages() {
        // Budget of 20 tokens → ~80 chars
        let mut builder = ConversationBuilder::with_budget(20);

        // First pair (preserved)
        builder.push_user("A".repeat(20)); // ~5 tokens
        builder.push_assistant("B".repeat(20)); // ~5 tokens

        // Middle messages (will be dropped)
        builder.push_user("C".repeat(40)); // ~10 tokens
        builder.push_assistant("D".repeat(40)); // ~10 tokens

        // At this point: 4 messages, ~30 tokens > 20 budget
        // Truncation keeps first 2 + drops from middle until under budget
        assert!(builder.estimated_tokens() <= 20);
        // First pair preserved
        assert_eq!(builder.messages()[0].content, "A".repeat(20));
        assert_eq!(builder.messages()[1].content, "B".repeat(20));
    }

    #[test]
    fn truncation_preserves_most_recent() {
        let mut builder = ConversationBuilder::with_budget(30);

        // First pair: ~5+5 = 10 tokens
        builder.push_user("A".repeat(20));
        builder.push_assistant("B".repeat(20));

        // Middle pair: ~10+10 = 20 tokens (pushes to 30, at limit)
        builder.push_user("C".repeat(40));
        builder.push_assistant("D".repeat(40));

        // Recent message: ~10 tokens (pushes to 40 > 30)
        builder.push_user("E".repeat(40));

        // Should keep first pair + most recent, drop middle
        assert!(builder.estimated_tokens() <= 30);
        assert_eq!(builder.messages()[0].content, "A".repeat(20));
        assert_eq!(builder.messages()[1].content, "B".repeat(20));
        // Last message should be the most recent
        assert_eq!(builder.messages().last().unwrap().content, "E".repeat(40));
    }

    #[test]
    fn no_truncation_under_budget() {
        let mut builder = ConversationBuilder::with_budget(1000);
        builder.push_user("Hello");
        builder.push_assistant("Hi");
        builder.push_user("How are you?");
        builder.push_assistant("I'm good!");

        assert_eq!(builder.len(), 4);
    }

    #[test]
    fn clear_resets_conversation() {
        let mut builder = ConversationBuilder::default();
        builder.push_user("msg");
        builder.push_assistant("resp");
        assert_eq!(builder.len(), 2);

        builder.clear();
        assert!(builder.is_empty());
        assert_eq!(builder.estimated_tokens(), 0);
    }

    #[test]
    fn with_budget_constructor() {
        let builder = ConversationBuilder::with_budget(100);
        assert!(builder.is_empty());
    }
}
