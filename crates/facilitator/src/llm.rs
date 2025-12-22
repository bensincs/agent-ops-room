//! LLM-based intent interpretation

use common::{ChatMessage, ChatRequest, FunctionDefinition, LlmClient, ResponseMessage, Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub agent_id: String,
    pub goal: String,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub enum FacilitatorAction {
    AssignTask(TaskAssignment),
    ReplyDirectly(String),
}

pub struct AnalysisResult {
    pub actions: Vec<FacilitatorAction>,
}

pub struct FacilitatorLlm {
    client: LlmClient,
    model: String,
}

impl FacilitatorLlm {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let client = LlmClient::new(api_key, model.clone(), base_url);
        Self { client, model }
    }

    /// Execute facilitator logic: analyze conversation context and determine task assignments
    pub async fn execute(
        &self,
        context: &[ChatMessage],
        available_agents: &[(String, Option<String>)], // (agent_id, description)
    ) -> Result<ResponseMessage, Box<dyn std::error::Error>> {
        let system_prompt = self.build_system_prompt(available_agents);

        // Build messages: system + context
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        }];

        messages.extend(context.iter().cloned());

        // Create dynamic tools - one per agent (no reply_to_user tool)
        let tools: Vec<Tool> = available_agents
            .iter()
            .map(|(agent_id, description)| {
                let desc = description
                    .as_ref()
                    .map(|d| format!("Assign a task to {}. Agent capabilities: {}", agent_id, d))
                    .unwrap_or_else(|| format!("Assign a task to {}", agent_id));

                Tool {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: format!("assign_to_{}", agent_id.replace("-", "_")),
                        description: desc,
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "goal": {
                                    "type": "string",
                                    "description": "Clear description of what the agent should accomplish"
                                },
                                "reasoning": {
                                    "type": "string",
                                    "description": "Why this agent is appropriate for this task"
                                }
                            },
                            "required": ["goal", "reasoning"]
                        }),
                    },
                }
            })
            .collect();

        debug!(
            "Sending LLM request with {} messages and {} agent tools",
            context.len(),
            available_agents.len()
        );

        let chat_response = self
            .client
            .complete_with_tools(messages, tools, Some(0.3), Some("auto".to_string()))
            .await
            .map_err(|e| e.to_string())?;

        if let Some(choice) = chat_response.choices.first() {
            if let Some(tool_calls) = &choice.message.tool_calls {
                debug!("LLM made {} tool call(s)", tool_calls.len());
                for tool_call in tool_calls {
                    debug!(
                        "Tool call: {} with args: {}",
                        tool_call.function.name, tool_call.function.arguments
                    );
                }
            }
            Ok(choice.message.clone())
        } else {
            Err("No response from LLM".into())
        }
    }

    fn build_system_prompt(&self, available_agents: &[(String, Option<String>)]) -> String {
        let agents_list = if available_agents.is_empty() {
            "No agents currently available.".to_string()
        } else {
            available_agents
                .iter()
                .map(|(id, desc)| {
                    if let Some(description) = desc {
                        format!("- {} - {}", id, description)
                    } else {
                        format!("- {}", id)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"You are the Facilitator in an Agent Ops Room. Your role is to coordinate work between users and specialized agents.

Available agents:
{}

IMPORTANT: You do NOT answer questions yourself. You delegate ALL work to agents.

When a user asks a question or requests something:
1. Identify which agent should handle it based on their capabilities
2. Call the appropriate assign_to_{{agent_id}} function with a clear goal

Only respond directly (without calling an agent) for:
- Greetings like "hi", "hello", "hey" (just say hi back)
- "Thank you" or "thanks" (acknowledge it)
- Pure conversation with no question or request

If the user asks ANY question or wants ANY task done, assign it to an agent - even simple ones.

Examples:
- "What's 1+1?" → assign_to_math_agent (don't answer it yourself)
- "Hello" → respond "Hi! I can help coordinate tasks. What do you need?"
- "What time is it?" → assign to an appropriate agent
- "Thanks!" → respond "You're welcome!"
"#,
            agents_list
        )
    }
}
