//! Shared LLM client utilities

use serde::{Deserialize, Serialize};
use tracing::{debug, error};

/// LLM client for OpenAI-compatible APIs (OpenAI, Azure AI Foundry, etc.)
#[derive(Debug, Clone)]
pub struct LlmClient {
    api_key: String,
    model: String,
    base_url: String,
}

/// Chat message for LLM conversations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: FunctionDefinition,
}

/// Function definition for tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion request
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

/// Chat completion response
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

/// Response choice
#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

/// Response message with optional content and tool calls
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Tool call in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
        }
    }

    /// Send a chat completion request
    pub async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse, String> {
        let client = reqwest::Client::new();

        debug!(
            "Sending LLM request with {} messages",
            request.messages.len()
        );

        let url = format!("{}/chat/completions", self.base_url);
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("LLM API error {}: {}", status, body);
            return Err(format!("LLM API error: {} - {}", status, body));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        debug!("LLM raw response: {}", response_text);

        let chat_response: ChatResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        Ok(chat_response)
    }

    /// Simple text completion without tools
    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
    ) -> Result<String, String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            tools: None,
            tool_choice: None,
        };

        let response = self.chat_completion(request).await?;

        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                return Ok(content.clone());
            }
        }

        Err("No response from LLM".to_string())
    }

    /// Completion with tool calling support
    pub async fn complete_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
        temperature: Option<f32>,
        tool_choice: Option<String>,
    ) -> Result<ChatResponse, String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            tools: Some(tools),
            tool_choice,
        };

        self.chat_completion(request).await
    }
}
