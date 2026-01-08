//! Sink configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "sink")]
#[command(about = "AOR Sink - Stores messages to a file")]
pub struct SinkConfig {
    /// MQTT broker host
    #[arg(long, env = "AOR_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "AOR_MQTT_PORT", default_value = "1883")]
    pub mqtt_port: u16,

    /// Room ID
    #[arg(long, env = "AOR_ROOM_ID", default_value = "default")]
    pub room_id: String,

    /// Output file path
    #[arg(long, env = "AOR_SINK_FILE", default_value = "messages.jsonl")]
    pub output_file: String,

    /// Append to existing file (default: true)
    #[arg(long, env = "AOR_SINK_APPEND", default_value = "true")]
    pub append: bool,
}
