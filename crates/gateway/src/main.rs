//! Gateway - Deterministic moderation and enforcement
//!
//! Responsibilities:
//! - Validate agent messages
//! - Enforce mic grants
//! - Enforce rate limits and schemas
//! - Republish approved messages
//!
//! Key property: No AI, fully deterministic, enforceable via ACLs

mod config;
mod mic_grant;
mod validator;

use clap::Parser;
use common::message::HeartbeatPayload;
use common::{topics, Envelope, EnvelopeType, RejectPayload, Sender, SenderKind};
use config::GatewayConfig;
use mic_grant::MicGrantTracker;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Gateway starting...");

    let config = GatewayConfig::parse();

    info!("Configuration loaded:");
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  Room ID: {}", config.room_id);
    info!("  Max validation time: {}ms", config.max_validation_time_ms);
    info!("  Verbose rejections: {}", config.verbose_rejections);

    // Initialize MQTT client
    let mut mqttoptions = MqttOptions::new(
        format!("{}-gateway", config.mqtt_client_id_prefix),
        &config.mqtt_host,
        config.mqtt_port,
    );
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(config.mqtt_keep_alive_secs));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Subscribe to topics
    let public_candidates = topics::public_candidates(&config.room_id);
    let control = topics::control(&config.room_id);

    client
        .subscribe(&public_candidates, QoS::AtLeastOnce)
        .await?;
    client.subscribe(&control, QoS::AtLeastOnce).await?;

    info!("Subscribed to:");
    info!("  {}", public_candidates);
    info!("  {}", control);

    // Initialize mic grant tracker
    let mut tracker = MicGrantTracker::new();

    // Spawn heartbeat task
    let client_clone = client.clone();
    let room_id = config.room_id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        let mut counter = 0u64;
        loop {
            interval.tick().await;
            counter += 1;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let payload = if counter % 3 == 0 {
                HeartbeatPayload {
                    ts: now,
                    description: Some("Gateway - validates and moderates agent messages".to_string()),
                    can_accept_tasks: false,
                }
            } else {
                HeartbeatPayload {
                    ts: now,
                    description: None,
                    can_accept_tasks: false,
                }
            };
            
            let heartbeat = Envelope {
                id: format!("gateway_heartbeat_{}", counter),
                message_type: EnvelopeType::Heartbeat,
                room_id: room_id.clone(),
                from: Sender {
                    kind: SenderKind::System,
                    id: "gateway".to_string(),
                },
                ts: now,
                payload: serde_json::to_value(payload).unwrap(),
            };
            let topic = format!("rooms/{}/agents/gateway/heartbeat", room_id);
            let _ = client_clone
                .publish(
                    topic,
                    QoS::AtLeastOnce,
                    false,
                    serde_json::to_vec(&heartbeat).unwrap(),
                )
                .await;
        }
    });

    info!("Gateway running");

    // Event loop
    loop {
        match eventloop.poll().await {
            Ok(notification) => {
                if let Event::Incoming(Packet::Publish(p)) = notification {
                    let topic = p.topic.clone();
                    let payload = p.payload.to_vec();

                    // Parse envelope
                    let envelope: Envelope = match serde_json::from_slice(&payload) {
                        Ok(e) => e,
                        Err(e) => {
                            warn!("Failed to parse envelope from {}: {}", topic, e);
                            continue;
                        }
                    };

                    // Handle message based on topic
                    if topic == control {
                        handle_control_message(&envelope, &mut tracker);
                    } else if topic == public_candidates {
                        handle_candidate_message(&envelope, &mut tracker, &client, &config).await;
                    }
                }
            }
            Err(e) => {
                error!("MQTT error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

fn handle_control_message(envelope: &Envelope, tracker: &mut MicGrantTracker) {
    match envelope.message_type {
        EnvelopeType::MicGrant => {
            if let Ok(payload) =
                serde_json::from_value::<common::MicGrantPayload>(envelope.payload.clone())
            {
                info!(
                    "Mic grant: agent={}, task={}, max_messages={}",
                    payload.agent_id, payload.task_id, payload.max_messages
                );
                tracker.grant(payload);
            } else {
                warn!("Failed to parse MicGrant payload");
            }
        }
        EnvelopeType::MicRevoke => {
            if let Ok(payload) =
                serde_json::from_value::<common::MicRevokePayload>(envelope.payload.clone())
            {
                info!(
                    "Mic revoke: agent={}, task={}",
                    payload.agent_id, payload.task_id
                );
                tracker.revoke(&payload.agent_id, &payload.task_id);
            } else {
                warn!("Failed to parse MicRevoke payload");
            }
        }
        _ => {
            // Ignore other control messages
        }
    }
}

async fn handle_candidate_message(
    envelope: &Envelope,
    tracker: &mut MicGrantTracker,
    client: &AsyncClient,
    _config: &GatewayConfig,
) {
    let current_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Validate the message
    match validator::validate_message(envelope, tracker, current_ts) {
        Ok(()) => {
            // Republish to public topic
            let public_topic = topics::public(&envelope.room_id);
            let payload = serde_json::to_vec(&envelope).unwrap();

            if let Err(e) = client
                .publish(public_topic.clone(), QoS::AtLeastOnce, false, payload)
                .await
            {
                error!("Failed to publish to {}: {}", public_topic, e);
            } else {
                info!(
                    "Approved message {} from {} to public",
                    envelope.id, envelope.from.id
                );
            }
        }
        Err(e) => {
            // Publish rejection
            warn!(
                "Rejected message {} from {}: {}",
                envelope.id, envelope.from.id, e
            );

            let reject_envelope = create_rejection(envelope, &e.to_string(), current_ts);
            let control_topic = topics::control(&envelope.room_id);
            let payload = serde_json::to_vec(&reject_envelope).unwrap();

            if let Err(err) = client
                .publish(control_topic, QoS::AtLeastOnce, false, payload)
                .await
            {
                error!("Failed to publish rejection: {}", err);
            }
        }
    }
}

fn create_rejection(original: &Envelope, reason: &str, ts: u64) -> Envelope {
    let task_id = if original.message_type == EnvelopeType::Result {
        serde_json::from_value::<common::ResultPayload>(original.payload.clone())
            .ok()
            .map(|r| r.task_id)
            .unwrap_or_default()
    } else {
        String::new()
    };

    let reject_payload = RejectPayload {
        message_id: original.id.clone(),
        task_id,
        reason: reason.to_string(),
    };

    Envelope {
        id: format!("reject_{}", original.id),
        message_type: EnvelopeType::Reject,
        room_id: original.room_id.clone(),
        from: common::Sender {
            kind: common::SenderKind::System,
            id: "gateway".to_string(),
        },
        ts,
        payload: serde_json::to_value(reject_payload).unwrap(),
    }
}
