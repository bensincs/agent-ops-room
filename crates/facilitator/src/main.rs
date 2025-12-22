//! Facilitator - Coordination and leadership
//!
//! Watches public chat for user messages, uses LLM to interpret intent,
//! assigns tasks to available agents, and issues mic grants.

mod agent_registry;
mod config;
mod llm;

use agent_registry::AgentRegistry;
use clap::Parser;
use common::message::{
    FromKind, MicGrantPayload, ResultContent, ResultMessageType, ResultOutcome, ResultPayload,
    SayPayload, TaskPayload,
};
use common::{topics, Envelope, EnvelopeType, MessageHistory, Sender, SenderKind};
use config::FacilitatorConfig;
use llm::FacilitatorLlm;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = FacilitatorConfig::parse();

    info!("Facilitator starting");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  LLM: {}", config.openai_model);

    // Connect to MQTT
    let mut mqtt_options = MqttOptions::new("facilitator", &config.mqtt_host, config.mqtt_port);
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(30));
    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    // Subscribe to topics
    let public_topic = topics::public(&config.room_id);
    let heartbeat_topic = topics::all_agent_heartbeats(&config.room_id);
    client.subscribe(&public_topic, QoS::AtLeastOnce).await?;
    client.subscribe(&heartbeat_topic, QoS::AtLeastOnce).await?;

    info!("Subscribed to:");
    info!("  {}", public_topic);
    info!("  {}", heartbeat_topic);

    // Initialize conversation memory
    let memory = Arc::new(Mutex::new(MessageHistory::new(50)));

    // Specific Initializers
    let mut agent_registry = AgentRegistry::new(config.agent_heartbeat_timeout_secs);
    let mut next_task_id = 0u64;
    let llm_client = FacilitatorLlm::new(
        config.openai_api_key.clone(),
        config.openai_model.clone(),
        config.openai_base_url.clone(),
    );

    info!("Facilitator running");

    // Main event loop
    loop {
        match event_loop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if p.topic == public_topic {
                    handle_user_message(
                        &p.payload,
                        &config,
                        &client,
                        &mut next_task_id,
                        &llm_client,
                        &agent_registry,
                        &memory,
                    )
                    .await;
                } else if p.topic.ends_with("/heartbeat") {
                    handle_heartbeat(&p.topic, &p.payload, &mut agent_registry);
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("MQTT error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

fn handle_heartbeat(topic: &str, payload: &[u8], agent_registry: &mut AgentRegistry) {
    // Extract agent_id from: rooms/{roomId}/agents/{agentId}/heartbeat
    if let Some(agent_id) = topic.split('/').nth(3) {
        if let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) {
            if envelope.message_type == EnvelopeType::Heartbeat {
                if let Ok(heartbeat) =
                    serde_json::from_value::<common::message::HeartbeatPayload>(envelope.payload)
                {
                    agent_registry.update_agent(agent_id.to_string(), heartbeat.description);
                }
            }
        }
    }
}

async fn handle_user_message(
    payload: &[u8],
    config: &FacilitatorConfig,
    client: &AsyncClient,
    next_task_id: &mut u64,
    llm_client: &FacilitatorLlm,
    agent_registry: &AgentRegistry,
    memory: &Arc<Mutex<MessageHistory>>,
) {
    // Parse envelope
    let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) else {
        return;
    };

    // Store all public messages in memory
    {
        let mut mem = memory.lock().await;
        mem.add(envelope.clone());
    }

    // Only process 'say' messages from users for task assignment
    if envelope.message_type != EnvelopeType::Say || envelope.from.kind != FromKind::User {
        return;
    }

    let Ok(say) = serde_json::from_value::<SayPayload>(envelope.payload) else {
        return;
    };

    info!("User: {}", say.text);

    // Get active agents
    let active_agents = agent_registry.get_active_agents();
    if active_agents.is_empty() {
        warn!("No active agents available");
        return;
    }

    info!("Active agents: {}", active_agents.join(", "));

    // Get conversation context from memory
    let mut context = {
        let mem = memory.lock().await;
        mem.to_chat_messages()
    };

    // Get active agents with descriptions
    let agents_with_desc = agent_registry.get_active_agents_with_descriptions();

    // Agentic loop: keep executing until no tool calls (task assignments) are made
    loop {
        // Execute facilitator logic
        let response_msg = match llm_client.execute(&context, &agents_with_desc).await {
            Ok(msg) => {
                let tool_count = msg.tool_calls.as_ref().map(|c| c.len()).unwrap_or(0);
                info!("LLM returned {} tool call(s)", tool_count);
                msg
            }
            Err(e) => {
                error!("LLM analysis failed: {}", e);
                return;
            }
        };

        // If no tool calls, check if there's a direct response to send
        let Some(tool_calls) = response_msg.tool_calls.as_ref() else {
            // No tool calls - send direct response if there is one
            if let Some(content) = &response_msg.content {
                if !content.trim().is_empty() {
                    info!("→ Direct reply: {}", content);
                    let now = now_secs();
                    let envelope = Envelope {
                        id: format!("facilitator_{}", now),
                        message_type: EnvelopeType::Result,
                        room_id: config.room_id.clone(),
                        from: Sender {
                            kind: SenderKind::Agent,
                            id: "facilitator".to_string(),
                        },
                        ts: now,
                        payload: serde_json::to_value(ResultPayload {
                            task_id: "direct_reply".to_string(),
                            message_type: ResultMessageType::Result,
                            content: ResultContent::Result(ResultOutcome {
                                text: content.clone(),
                            }),
                        })
                        .unwrap(),
                    };
                    let _ = client
                        .publish(
                            topics::public(&config.room_id),
                            QoS::AtLeastOnce,
                            false,
                            serde_json::to_vec(&envelope).unwrap(),
                        )
                        .await;
                }
            }
            return; // Done - no more actions needed
        };

        // Process tool calls (task assignments)
        info!("Processing {} task assignment(s)", tool_calls.len());
        let mut tool_result_msgs = Vec::new();

        for tool_call in tool_calls {
            // Extract agent_id from function name: assign_to_{agent_id}
            if let Some(agent_id) = tool_call.function.name.strip_prefix("assign_to_") {
                let agent_id = agent_id.replace("_", "-");

                // Parse the arguments (goal and reasoning)
                let args: serde_json::Value =
                    serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

                let goal = args
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !goal.is_empty() {
                    info!("→ @{}: {}", agent_id, goal);

                    let task_id = format!("task_{}", *next_task_id);
                    *next_task_id += 1;
                    let now = now_secs();

                    // 1. Send task to agent inbox
                    let task_envelope = Envelope {
                        id: format!("task_{}", task_id),
                        message_type: EnvelopeType::Task,
                        room_id: config.room_id.clone(),
                        from: Sender {
                            kind: SenderKind::Agent,
                            id: "facilitator".to_string(),
                        },
                        ts: now,
                        payload: serde_json::to_value(TaskPayload {
                            task_id: task_id.clone(),
                            goal: goal.clone(),
                            format: None,
                            deadline: Some(now + 300),
                        })
                        .unwrap(),
                    };
                    let _ = client
                        .publish(
                            topics::agent_inbox(&config.room_id, &agent_id),
                            QoS::AtLeastOnce,
                            false,
                            serde_json::to_vec(&task_envelope).unwrap(),
                        )
                        .await;

                    // 2. Issue mic grant
                    let grant_envelope = Envelope {
                        id: format!("grant_{}", task_id),
                        message_type: EnvelopeType::MicGrant,
                        room_id: config.room_id.clone(),
                        from: Sender {
                            kind: SenderKind::Agent,
                            id: "facilitator".to_string(),
                        },
                        ts: now,
                        payload: serde_json::to_value(MicGrantPayload {
                            task_id: task_id.clone(),
                            agent_id: agent_id.clone(),
                            max_messages: config.default_max_messages,
                            allowed_message_types: vec![
                                ResultMessageType::Ack,
                                ResultMessageType::ClarifyingQuestion,
                                ResultMessageType::Progress,
                                ResultMessageType::Finding,
                                ResultMessageType::Risk,
                                ResultMessageType::Result,
                                ResultMessageType::ArtifactLink,
                            ],
                            expires_at: now + config.default_mic_duration_secs,
                        })
                        .unwrap(),
                    };
                    let _ = client
                        .publish(
                            topics::control(&config.room_id),
                            QoS::AtLeastOnce,
                            false,
                            serde_json::to_vec(&grant_envelope).unwrap(),
                        )
                        .await;

                    // Add tool result
                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": format!("Task {} assigned to {} successfully", task_id, agent_id)
                    }));
                } else {
                    warn!("Empty goal in tool call");
                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": "Error: goal cannot be empty"
                    }));
                }
            } else {
                warn!("Unknown tool: {}", tool_call.function.name);
                tool_result_msgs.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tool_call.id,
                    "content": format!("Error: unknown tool '{}'", tool_call.function.name)
                }));
            }
        }

        // Add the assistant message with tool calls to context
        context.push(
            serde_json::from_value(serde_json::json!({
                "role": "assistant",
                "content": response_msg.content,
                "tool_calls": response_msg.tool_calls
            }))
            .unwrap(),
        );

        // Add all tool result messages to context
        for tool_msg in tool_result_msgs {
            context.push(serde_json::from_value(tool_msg).unwrap());
        }

        // Loop continues - facilitator can make additional assignments or provide final response
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
