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
use common::{topics, MessageHistory};
use config::AgentConfig;
use llm::SpecialistLlm;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = Arc::new(AgentConfig::parse());

    info!("Command Execution Agent starting...");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  LLM: {}", config.openai_model);
    info!("  Agent ID: {}", config.agent_id);

    // Initialize LLM client with command execution capability
    let system_prompt = "You are a command-line execution assistant. You can run shell commands using the run_command tool and return their output to the user.

When a user asks you to run a command, check something on the system, or perform any task that requires shell access:
1. Call the run_command tool with the appropriate bash/shell command
2. Wait for the output
3. Present the results clearly to the user

Be helpful and explain what commands you're running and why.".to_string();
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
    let memory = Arc::new(tokio::sync::Mutex::new(MessageHistory::new(
        config.max_memory_messages,
    )));

    // Specific Initializers
    let heartbeat_client = client.clone();
    let heartbeat_room_id = config.room_id.clone();
    let heartbeat_agent_id = config.agent_id.clone();
    tokio::spawn(async move {
        send_heartbeats(heartbeat_client, &heartbeat_room_id, &heartbeat_agent_id).await;
    });

    info!("Command Execution Agent running");

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

async fn handle_public_message(payload: &[u8], memory: &Arc<tokio::sync::Mutex<MessageHistory>>) {
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
    memory: &Arc<tokio::sync::Mutex<MessageHistory>>,
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

            if tool_call.function.name == "run_command" {
                // Parse arguments
                let args: serde_json::Value =
                    serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

                if let Some(command_str) = args.get("command").and_then(|v| v.as_str()) {
                    info!("🔧 Executing command: {}", command_str);

                    // Send finding with command being executed
                    send_result(
                        client,
                        config,
                        &task_payload.task_id,
                        ResultMessageType::Finding,
                        ResultContent::Finding(FindingContent {
                            text: Some(format!("🔧 Executing: {}", command_str)),
                            bullets: None,
                        }),
                    )
                    .await;

                    // Execute the command using zsh
                    let output = Command::new("zsh")
                        .arg("-c")
                        .arg(command_str)
                        .output()
                        .await;

                    let tool_result = match output {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let exit_code = output.status.code().unwrap_or(-1);

                            info!("Command exit code: {}", exit_code);

                            // Send finding with execution details
                            send_result(
                                client,
                                config,
                                &task_payload.task_id,
                                ResultMessageType::Finding,
                                ResultContent::Finding(FindingContent {
                                    text: Some(format!("Exit code: {}", exit_code)),
                                    bullets: None,
                                }),
                            )
                            .await;

                            // Format output as markdown for the LLM
                            let mut result_parts = Vec::new();
                            result_parts.push(format!("**Exit Code:** {}", exit_code));

                            if !stdout.is_empty() {
                                result_parts
                                    .push(format!("**Output:**\n```\n{}\n```", stdout.trim()));
                            }

                            if !stderr.is_empty() {
                                result_parts
                                    .push(format!("**Errors:**\n```\n{}\n```", stderr.trim()));
                            }

                            result_parts.join("\n\n")
                        }
                        Err(e) => {
                            error!("Failed to execute command: {}", e);
                            format!("**Error:** {}", e)
                        }
                    };

                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": tool_result
                    }));
                } else {
                    tool_result_msgs.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": "Error: missing 'command' argument"
                    }));
                }
            } else {
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
    let description = "Command execution agent. Can run shell commands (bash/zsh) and return their output. Ask me to check system status, run scripts, or execute any command-line operations.";

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
