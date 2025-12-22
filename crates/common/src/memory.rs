//! Message history and conversation memory for LLM context

use crate::llm::ChatMessage;
use crate::message::{Envelope, EnvelopeType, SayPayload, SenderKind};
use serde_json::Value;
use std::collections::VecDeque;

/// Message history tracker with configurable size
#[derive(Debug)]
pub struct MessageHistory {
    messages: VecDeque<Envelope>,
    max_messages: usize,
}

impl MessageHistory {
    /// Create a new message history with the specified capacity
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_messages),
            max_messages,
        }
    }

    /// Add a message to the history
    pub fn add(&mut self, envelope: Envelope) {
        if self.messages.len() >= self.max_messages {
            self.messages.pop_front();
        }
        self.messages.push_back(envelope);
    }

    /// Get the number of messages in history
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Convert message history into chat messages for LLM
    /// Users and system -> "user" role
    /// Agents -> "assistant" role
    pub fn to_chat_messages(&self) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .filter_map(|envelope| {
                let role = match envelope.from.kind {
                    SenderKind::User | SenderKind::System => "user",
                    SenderKind::Agent => "assistant",
                };

                let content = match envelope.message_type {
                    EnvelopeType::Say => extract_say_text(&envelope.payload)
                        .map(|text| format!("{}: {}", envelope.from.id, text)),
                    EnvelopeType::Result => extract_result_text(&envelope.payload)
                        .map(|text| format!("{}: {}", envelope.from.id, text)),
                    _ => None,
                };

                content.map(|c| ChatMessage {
                    role: role.to_string(),
                    content: Some(c),
                    tool_calls: None,
                    tool_call_id: None,
                })
            })
            .collect()
    }

    /// Convert with a filter - only include specific message types
    pub fn to_chat_messages_filtered(
        &self,
        filter: impl Fn(&Envelope) -> bool,
    ) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .filter(|env| filter(env))
            .filter_map(|envelope| {
                let role = match envelope.from.kind {
                    SenderKind::User | SenderKind::System => "user",
                    SenderKind::Agent => "assistant",
                };

                let content = match envelope.message_type {
                    EnvelopeType::Say => extract_say_text(&envelope.payload)
                        .map(|text| format!("{}: {}", envelope.from.id, text)),
                    _ => None,
                };

                content.map(|c| ChatMessage {
                    role: role.to_string(),
                    content: Some(c),
                    tool_calls: None,
                    tool_call_id: None,
                })
            })
            .collect()
    }
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self::new(50)
    }
}

// Helper functions to extract content from payloads

fn extract_say_text(payload: &Value) -> Option<String> {
    serde_json::from_value::<SayPayload>(payload.clone())
        .ok()
        .map(|p| p.text)
}

fn extract_result_text(payload: &Value) -> Option<String> {
    payload
        .get("content")?
        .get("text")
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Sender;

    #[test]
    fn test_message_history_capacity() {
        let mut history = MessageHistory::new(3);

        for i in 0..5 {
            let envelope = Envelope {
                id: format!("msg_{}", i),
                message_type: EnvelopeType::Say,
                room_id: "test".to_string(),
                from: Sender {
                    kind: SenderKind::User,
                    id: "user1".to_string(),
                },
                ts: i as u64,
                payload: serde_json::json!({"text": format!("Message {}", i)}),
            };
            history.add(envelope);
        }

        assert_eq!(history.len(), 3);
    }
}
