//! Summarizer - Generates conversation summaries for context management
//!
//! Subscribes to public messages, tracks conversation flow, and generates
//! periodic summaries using incremental summarization (previous summary + new messages).

mod config;
mod llm;

use clap::Parser;
use common::{topics, Envelope, EnvelopeType, MessageHistory, ResultPayload, Sender, SenderKind, SummaryPayload};
use config::SummarizerConfig;
use llm::SummarizerLlm;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = SummarizerConfig::parse();

    info!("Summarizer starting");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  Summary interval: {} messages", config.summary_interval);
    info!("  LLM: {}", config.openai_model);

    // Connect to MQTT
    let mut mqtt_options = MqttOptions::new("summarizer", &config.mqtt_host, config.mqtt_port);
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(30));
    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    // Subscribe to public topic
    let public_topic = topics::public(&config.room_id);
    client.subscribe(&public_topic, QoS::AtLeastOnce).await?;

    info!("Subscribed to: {}", public_topic);

    // Initialize state
    let message_history = Arc::new(Mutex::new(MessageHistory::new(1000))); // Keep more history for summarization
    let last_summary_ts = Arc::new(Mutex::new(0u64));
    let summary_text = Arc::new(Mutex::new(String::new()));
    let message_count_since_summary = Arc::new(Mutex::new(0u64));

    let llm_client = SummarizerLlm::new(
        config.openai_api_key.clone(),
        config.openai_model.clone(),
        Some(config.openai_base_url.clone()),
    );

    info!("Summarizer running");

    // Main event loop
    loop {
        match event_loop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if p.topic == public_topic {
                    handle_public_message(
                        &p.payload,
                        &config,
                        &client,
                        &message_history,
                        &last_summary_ts,
                        &summary_text,
                        &message_count_since_summary,
                        &llm_client,
                    )
                    .await;
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

async fn handle_public_message(
    payload: &[u8],
    config: &SummarizerConfig,
    client: &AsyncClient,
    message_history: &Arc<Mutex<MessageHistory>>,
    last_summary_ts: &Arc<Mutex<u64>>,
    summary_text: &Arc<Mutex<String>>,
    message_count_since_summary: &Arc<Mutex<u64>>,
    llm_client: &SummarizerLlm,
) {
    // Parse envelope
    let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) else {
        return;
    };

    // Skip summary messages (don't summarize summaries)
    if envelope.message_type == EnvelopeType::Summary {
        return;
    }

    // Add to history
    {
        let mut history = message_history.lock().await;
        history.add(envelope.clone());
    }

    // Only trigger summarization on Result messages (task completion)
    if envelope.message_type != EnvelopeType::Result {
        return;
    }

    // Check if this is actually a result (not ack)
    if let Ok(result_payload) = serde_json::from_value::<ResultPayload>(envelope.payload.clone()) {
        use common::ResultMessageType;
        if !matches!(result_payload.message_type, ResultMessageType::Result) {
            // Skip acks and other non-result messages
            return;
        }
    } else {
        return;
    }

    // Increment counter
    let mut count = message_count_since_summary.lock().await;
    *count += 1;

    // Check if we should generate a summary
    if *count >= config.summary_interval {
        info!("Reached {} completed tasks, generating summary...", *count);

        // Get messages since last summary
        let messages_to_summarize = {
            let history = message_history.lock().await;
            history
                .to_chat_messages()
                .into_iter()
                .filter(|_msg| {
                    // Since we don't have timestamp in ChatMessage, we keep all for now
                    // For now, include all messages (proper filtering would need message metadata)
                    true
                })
                .collect::<Vec<_>>()
        };

        if messages_to_summarize.is_empty() {
            warn!("No messages to summarize");
            return;
        }

        // Get previous summary
        let previous_summary = {
            let summary = summary_text.lock().await;
            if summary.is_empty() {
                None
            } else {
                Some(summary.clone())
            }
        };

        // Generate new summary
        match llm_client
            .generate_summary(previous_summary.as_deref(), &messages_to_summarize)
            .await
        {
            Ok(new_summary) => {
                info!("Summary generated ({} chars)", new_summary.len());

                let now = now_secs();

                // Update state
                {
                    let mut summary = summary_text.lock().await;
                    *summary = new_summary.clone();
                }
                {
                    let mut ts = last_summary_ts.lock().await;
                    *ts = envelope.ts; // Use timestamp of latest message
                }
                *count = 0; // Reset counter

                // Publish summary
                let summary_envelope = Envelope {
                    id: format!("summary_{}", now),
                    message_type: EnvelopeType::Summary,
                    room_id: config.room_id.clone(),
                    from: Sender {
                        kind: SenderKind::System,
                        id: "summarizer".to_string(),
                    },
                    ts: now,
                    payload: serde_json::to_value(SummaryPayload {
                        summary_text: new_summary,
                        covers_until_ts: envelope.ts,
                        message_count: config.summary_interval,
                        generated_at: now,
                    })
                    .unwrap(),
                };

                if let Err(e) = client
                    .publish(
                        topics::summary(&config.room_id),
                        QoS::AtLeastOnce,
                        false,
                        serde_json::to_vec(&summary_envelope).unwrap(),
                    )
                    .await
                {
                    error!("Failed to publish summary: {}", e);
                } else {
                    info!("Summary published to {}", topics::summary(&config.room_id));
                }
            }
            Err(e) => {
                error!("Failed to generate summary: {}", e);
            }
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
