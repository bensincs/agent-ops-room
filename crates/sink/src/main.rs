//! Sink - Stores messages to a file
//!
//! Subscribes to public messages and writes them to a JSONL file for persistence.

mod config;

use clap::Parser;
use common::{topics, Envelope};
use config::SinkConfig;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = SinkConfig::parse();

    info!("Sink starting");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  Output file: {}", config.output_file);
    info!("  Append mode: {}", config.append);

    // Open output file
    let file = Arc::new(Mutex::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .append(config.append)
            .truncate(!config.append)
            .open(&config.output_file)?,
    ));

    info!("Output file opened successfully");

    // Connect to MQTT
    let mut mqtt_options = MqttOptions::new("sink", &config.mqtt_host, config.mqtt_port);
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(30));
    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    // Subscribe to public topic
    let public_topic = topics::public(&config.room_id);
    client.subscribe(&public_topic, QoS::AtLeastOnce).await?;

    info!("Subscribed to: {}", public_topic);
    info!("Sink running - writing messages to {}", config.output_file);

    // Main event loop
    loop {
        match event_loop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if p.topic == public_topic {
                    handle_message(&p.payload, &file).await;
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

async fn handle_message(payload: &[u8], file: &Arc<Mutex<std::fs::File>>) {
    // Parse envelope
    let Ok(envelope) = serde_json::from_slice::<Envelope>(payload) else {
        error!("Failed to parse envelope");
        return;
    };

    // Serialize to JSONL (one JSON object per line)
    let json_line = match serde_json::to_string(&envelope) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize envelope: {}", e);
            return;
        }
    };

    // Write to file
    let mut file_guard = file.lock().await;
    if let Err(e) = writeln!(file_guard, "{}", json_line) {
        error!("Failed to write to file: {}", e);
        return;
    }

    // Ensure data is flushed to disk
    if let Err(e) = file_guard.flush() {
        error!("Failed to flush file: {}", e);
        return;
    }

    info!(
        "Wrote message: id={}, from={}, type={:?}",
        envelope.id, envelope.from.id, envelope.message_type
    );
}
