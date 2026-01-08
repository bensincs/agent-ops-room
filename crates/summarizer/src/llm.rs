//! LLM client for generating conversation summaries

use common::{AorError, ChatMessage, LlmClient};

pub struct SummarizerLlm {
    client: LlmClient,
}

impl SummarizerLlm {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: LlmClient::new(api_key, model, base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())),
        }
    }

    /// Generate a new summary from previous summary (if any) + new messages
    pub async fn generate_summary(
        &self,
        previous_summary: Option<&str>,
        new_messages: &[ChatMessage],
    ) -> Result<String, AorError> {
        let system_prompt = if let Some(prev) = previous_summary {
            format!(
                "You are a conversation summarizer for an AI agent collaboration room.\n\n\
                Create a BRIEF summary (2-3 sentences max) that:\n\
                1. Incorporates the previous summary\n\
                2. Adds only the most critical new information\n\
                3. Focuses on: user requests, agent actions, key findings\n\
                4. Omits: greetings, acknowledgments, routine status updates\n\n\
                Previous summary:\n{}\n\n\
                Update this summary with ONLY the essential new information from the messages below.",
                prev
            )
        } else {
            "You are a conversation summarizer for an AI agent collaboration room.\n\n\
            Create a BRIEF summary (2-3 sentences max) that:\n\
            1. Captures ONLY the most important information\n\
            2. Focuses on: user requests, agent actions, key findings\n\
            3. Omits: greetings, acknowledgments, routine messages\n\n\
            Provide a concise summary of the essential points from the messages below."
                .to_string()
        };

        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        }];

        messages.extend(new_messages.iter().cloned());

        let summary = self
            .client
            .complete(messages, Some(0.3))
            .await
            .map_err(|e| AorError::Llm(e))?;

        Ok(summary)
    }
}
