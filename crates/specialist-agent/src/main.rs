//! Specialist Agent - Perform domain-specific work
//!
//! Responsibilities:
//! - Subscribe to room topics
//! - Maintain local memory
//! - Execute tasks when assigned
//! - Publish structured results
//!
//! Non-responsibilities:
//! - No direct publishing to public chat
//! - No coordination authority

mod config;
mod llm;

use clap::Parser;
use common::message::{
    AckContent, Envelope, EnvelopeType, FindingContent, HeartbeatPayload, ResultContent,
    ResultMessageType, ResultOutcome, ResultPayload, Sender, SenderKind, TaskPayload,
};
use common::{topics, ChatMessage, MessageHistory, ResponseMessage};
use config::AgentConfig;
use llm::SpecialistLlm;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = Arc::new(AgentConfig::parse());

    info!("Math Tutor Agent starting...");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  LLM: {}", config.openai_model);
    info!("  Agent ID: {}", config.agent_id);

    // Initialize LLM client with domain-specific system prompt
    let system_prompt = "You are a helpful math tutor. Solve mathematical problems clearly and explain your reasoning step by step. Be concise but thorough.

IMPORTANT: When the user asks you to pick a random number, think of a number, or choose a number, you MUST call the secretly_pick_number tool. Do not just say you picked a number - actually call the function.

Example:
- User: \"Pick a number between 1 and 100\"
- You: Call secretly_pick_number with min=1, max=100
- Then respond: \"I've secretly picked a number between 1 and 100!\"".to_string();
    let llm_client = Arc::new(SpecialistLlm::new(
        config.openai_api_key.clone(),
        config.openai_model.clone(),
        config.openai_base_url.clone(),
        system_prompt,
    ));

    // Initialize MQTT client
    let mut mqttoptions = MqttOptions::new(
        format!("{}-{}", config.mqtt_client_id_prefix, config.agent_id),
        &config.mqtt_host,
        config.mqtt_port,
    );
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(config.mqtt_keep_alive_secs));
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Subscribe to topics
    let public_topic = topics::public(&config.room_id);
    let control_topic = topics::control(&config.room_id);
    let inbox_topic = topics::agent_inbox(&config.room_id, &config.agent_id);

    client.subscribe(&public_topic, QoS::AtLeastOnce).await?;
    client.subscribe(&control_topic, QoS::AtLeastOnce).await?;
    client.subscribe(&inbox_topic, QoS::AtLeastOnce).await?;

    info!("Subscribed to:");
    info!("  {}", public_topic);
    info!("  {}", control_topic);
    info!("  {}", inbox_topic);

    // Initialize conversation memory
    let memory = Arc::new(Mutex::new(MessageHistory::new(config.max_memory_messages)));

    // Specific Initializers
    let heartbeat_client = client.clone();
    let heartbeat_room_id = config.room_id.clone();
    let heartbeat_agent_id = config.agent_id.clone();
    tokio::spawn(async move {
        send_heartbeats(heartbeat_client, &heartbeat_room_id, &heartbeat_agent_id).await;
    });

    info!("Math Tutor Agent running");

    // Main event loop
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if p.topic == inbox_topic {
                    handle_inbox_message(&p.payload, &client, &config, &llm_client, &memory).await;
                } else if p.topic == public_topic {
                    handle_public_message(&p.payload, &memory).await;
                } else if p.topic == control_topic {
                    debug!("Received control message");
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

async fn handle_public_message(payload: &[u8], memory: &Arc<Mutex<MessageHistory>>) {
    if let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) {
        let mut mem = memory.lock().await;
        mem.add(envelope);
    }
}

async fn handle_inbox_message(
    payload: &[u8],
    client: &AsyncClient,
    config: &AgentConfig,
    llm_client: &SpecialistLlm,
    memory: &Arc<Mutex<MessageHistory>>,
) {
    let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) else {
        return;
    };

    if envelope.message_type != EnvelopeType::Task {
        return;
    }

    let task_payload = match serde_json::from_value::<TaskPayload>(envelope.payload.clone()) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to parse task payload: {}", e);
            return;
        }
    };

    info!(
        "Received task {}: {}",
        task_payload.task_id, task_payload.goal
    );

    // Send acknowledgment
    send_result(
        client,
        config,
        &task_payload.task_id,
        ResultMessageType::Ack,
        ResultContent::Ack(AckContent {
            text: "Task received, processing...".to_string(),
        }),
    )
    .await;

    // Get conversation context from memory
    let mut context = {
        let mem = memory.lock().await;
        mem.to_chat_messages()
    };

    // Agentic loop: keep executing until no tool calls are made
    let final_result = loop {
        // Execute specialist logic with context
        let response_msg = match llm_client.execute(&task_payload.goal, &context).await {
            Ok(msg) => {
                let tool_count = msg.tool_calls.as_ref().map(|c| c.len()).unwrap_or(0);
                info!("LLM returned {} tool call(s)", tool_count);
                msg
            }
            Err(e) => {
                error!("LLM error: {}", e);
                break e.to_string();
            }
        };

        // If no tool calls, we're done - return the final result
        let Some(tool_calls) = response_msg.tool_calls.as_ref() else {
            info!("No tool calls to process, returning final result");
            break response_msg
                .content
                .unwrap_or_else(|| "Task completed.".to_string());
        };

        // Process tool calls and collect results
        info!("Processing {} tool call(s)", tool_calls.len());
        let mut tool_result_msgs = Vec::new();

        for tool_call in tool_calls {
            info!("Processing tool call: {}", tool_call.function.name);

            if tool_call.function.name == "secretly_pick_number" {
                // Parse arguments
                let args: serde_json::Value =
                    serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

                if let (Some(min), Some(max)) = (
                    args.get("min").and_then(|v| v.as_f64()),
                    args.get("max").and_then(|v| v.as_f64()),
                ) {
                    // Pick a random number
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    let secret_number = rng.gen_range(min as i32..=max as i32);

                    info!("🎲 Secretly picked number: {}", secret_number);

                    // Send the secret number as a Finding (internal thinking)
                    send_result(
                        client,
                        config,
                        &task_payload.task_id,
                        ResultMessageType::Finding,
                        ResultContent::Finding(FindingContent {
                            text: Some(format!("🎲 Secretly picked number: {}", secret_number)),
                            bullets: None,
                        }),
                    )
                    .await;

                    // Create tool result message with proper tool_call_id
                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": format!("Successfully picked number: {}", secret_number)
                    }));
                } else {
                    warn!("Invalid arguments for secretly_pick_number");
                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": "Error: invalid min/max arguments"
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

        // Loop continues to call LLM again with updated context
    };

    // Send the final result
    send_result(
        client,
        config,
        &task_payload.task_id,
        ResultMessageType::Result,
        ResultContent::Result(ResultOutcome { text: final_result }),
    )
    .await;

    info!("Completed task {}", task_payload.task_id);
}

async fn send_result(
    client: &AsyncClient,
    config: &AgentConfig,
    task_id: &str,
    message_type: ResultMessageType,
    content: ResultContent,
) {
    let ts = now_secs();

    let result_payload = ResultPayload {
        task_id: task_id.to_string(),
        message_type: message_type.clone(),
        content,
    };

    let envelope = Envelope {
        id: format!("result_{}_{}", task_id, ts),
        message_type: EnvelopeType::Result,
        room_id: config.room_id.clone(),
        from: Sender {
            kind: SenderKind::Agent,
            id: config.agent_id.clone(),
        },
        ts,
        payload: serde_json::to_value(result_payload).unwrap(),
    };

    let topic = topics::public_candidates(&config.room_id);
    let payload_bytes = serde_json::to_vec(&envelope).unwrap();

    if let Err(e) = client
        .publish(topic, QoS::AtLeastOnce, false, payload_bytes)
        .await
    {
        error!("Failed to send result: {}", e);
    } else {
        info!("Sent {} for task {}", message_type, task_id);
    }
}

async fn send_heartbeats(client: AsyncClient, room_id: &str, agent_id: &str) {
    let mut counter = 0u64;
    let description = "Specialized in mathematical calculations, solving equations, and numerical analysis. Can help with arithmetic, algebra, calculus, and explaining mathematical concepts.";

    loop {
        counter += 1;
        let ts = now_secs();

        // Send description every 3rd heartbeat
        let payload = if counter % 3 == 0 {
            HeartbeatPayload {
                ts,
                description: Some(description.to_string()),
            }
        } else {
            HeartbeatPayload {
                ts,
                description: None,
            }
        };

        let envelope = Envelope {
            id: format!("heartbeat_{}_{}", agent_id, counter),
            message_type: EnvelopeType::Heartbeat,
            room_id: room_id.to_string(),
            from: Sender {
                kind: SenderKind::Agent,
                id: agent_id.to_string(),
            },
            ts,
            payload: serde_json::to_value(payload).unwrap(),
        };

        let topic = topics::agent_heartbeat(room_id, agent_id);
        let payload_bytes = serde_json::to_vec(&envelope).unwrap();

        if let Err(e) = client
            .publish(topic, QoS::AtLeastOnce, false, payload_bytes)
            .await
        {
            error!("Failed to send heartbeat: {}", e);
        } else {
            debug!("Sent heartbeat #{}", counter);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
