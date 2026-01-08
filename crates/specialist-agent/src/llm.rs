//! LLM-based task execution for the specialist agent

use common::{ChatMessage, FunctionDefinition, LlmClient, ResponseMessage, Tool};
use serde_json::json;
use tracing::{debug, error};

pub struct SpecialistLlm {
    client: LlmClient,
    system_prompt: String,
}

impl SpecialistLlm {
    pub fn new(api_key: String, model: String, base_url: String, system_prompt: String) -> Self {
        let client = LlmClient::new(api_key, model, base_url);
        Self {
            client,
            system_prompt,
        }
    }

    /// Execute specialist logic: solve the given goal using conversation context from memory
    pub async fn execute(
        &self,
        goal: &str,
        context: &[ChatMessage],
    ) -> Result<ResponseMessage, Box<dyn std::error::Error>> {
        // Build messages with system prompt
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: Some(self.system_prompt.clone()),
            tool_calls: None,
            tool_call_id: None,
        }];

        // Add conversation context (last 10 messages for context)
        messages.extend(context.iter().rev().take(10).rev().cloned());

        // Add the current goal/task
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(goal.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        // Define available tools
        let tools = vec![Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "run_command".to_string(),
                description: "Execute a shell command (bash/zsh) and return its output. Use this to run any command-line operations, check system status, run scripts, etc.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute (e.g., 'ls -la', 'ps aux | grep python', 'date')"
                        }
                    },
                    "required": ["command"]
                }),
            },
        }];

        // Use LLM with tool support
        match self
            .client
            .complete_with_tools(messages, tools, Some(0.3), None)
            .await
        {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(calls) = &choice.message.tool_calls {
                        debug!("LLM made {} tool call(s)", calls.len());
                        for call in calls {
                            debug!(
                                "Tool call: {} with args: {}",
                                call.function.name, call.function.arguments
                            );
                        }
                    } else {
                        debug!("No tool calls in LLM response");
                    }
                    Ok(choice.message.clone())
                } else {
                    Err("No response from LLM".into())
                }
            }
            Err(e) => {
                error!("LLM error: {}", e);
                Err(format!(
                    "Sorry, I encountered an error while solving this problem: {}",
                    e
                )
                .into())
            }
        }
    }
}
