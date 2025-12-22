//! UI Bridge - Interface between humans and MQTT
//!
//! Responsibilities:
//! - Streams room events to clients (SSE)
//! - Accepts user messages via HTTP
//! - Publishes user chat into MQTT

mod config;

use axum::{routing::get, Router};
use clap::Parser;
use config::UiBridgeConfig;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    info!("UI Bridge starting...");

    let config = UiBridgeConfig::parse();

    info!("Configuration loaded:");
    info!("  MQTT: {}:{}", config.mqtt_host, config.mqtt_port);
    info!("  Room ID: {}", config.room_id);
    info!("  HTTP: {}:{}", config.http_host, config.http_port);

    // TODO: Initialize MQTT client
    // TODO: Create HTTP/SSE server
    // TODO: Set up routes for user messages and event streaming

    let _app: Router = Router::new().route("/health", get(health_check));

    info!("UI Bridge placeholder running (not yet implemented)");

    // Placeholder - prevent exit
    tokio::signal::ctrl_c().await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "UI Bridge OK (placeholder)"
}
