//! Replay - Browse and replay archived messages
//!
//! Interactive TUI for viewing messages stored by sink and replaying them to MQTT.

mod config;
mod tui;

use clap::Parser;
use common::Envelope;
use config::ReplayConfig;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::io::{BufRead, BufReader};
use tokio::sync::mpsc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = ReplayConfig::parse();

    info!("Starting replay component");
    info!("  Room ID: {}", config.room_id);
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  Input file: {}", config.input_file);

    // Load messages from file
    let messages = load_messages_from_file(&config.input_file)?;
    info!("Loaded {} messages", messages.len());

    // Set up MQTT client
    let mut mqtt_opts = MqttOptions::new("replay", &config.mqtt_host, config.mqtt_port);
    mqtt_opts.set_keep_alive(std::time::Duration::from_secs(5));

    let (client, mut event_loop) = AsyncClient::new(mqtt_opts, 10);

    // Channel for replay commands from TUI
    let (replay_tx, mut replay_rx) = mpsc::unbounded_channel();

    // Spawn MQTT connection handler
    tokio::spawn(async move {
        loop {
            if let Err(e) = event_loop.poll().await {
                tracing::error!("MQTT error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    });

    // Handle replay commands
    let replay_client = client.clone();
    let replay_room_id = config.room_id.clone();
    let replay_handle = tokio::spawn(async move {
        while let Some(cmd) = replay_rx.recv().await {
            match cmd {
                tui::TuiCommand::Replay(messages) => {
                    info!("Replaying {} messages", messages.len());
                    for msg in messages {
                        // Republish to public topic
                        let topic = common::topics::public(&replay_room_id);
                        if let Ok(payload) = serde_json::to_vec(&msg) {
                            if let Err(e) = replay_client
                                .publish(topic, QoS::AtLeastOnce, false, payload)
                                .await
                            {
                                tracing::error!("Failed to replay message {}: {}", msg.id, e);
                            } else {
                                info!("Replayed message: {}", msg.id);
                            }
                        }
                        // Small delay between messages
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }
    });

    // Run TUI (blocking)
    tui::run_tui(replay_tx, messages).await?;

    // Cleanup
    replay_handle.abort();

    Ok(())
}

fn load_messages_from_file(path: &str) -> Result<Vec<Envelope>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        if let Ok(line) = line {
            if let Ok(envelope) = serde_json::from_str::<Envelope>(&line) {
                messages.push(envelope);
            }
        }
    }

    Ok(messages)
}
