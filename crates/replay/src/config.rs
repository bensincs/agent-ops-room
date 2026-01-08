use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "replay")]
#[command(about = "Browse and replay archived messages from sink", long_about = None)]
pub struct ReplayConfig {
    /// MQTT broker host
    #[arg(long, env = "AOR_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "AOR_MQTT_PORT", default_value = "1883")]
    pub mqtt_port: u16,

    /// Room ID
    #[arg(long, env = "AOR_ROOM_ID")]
    pub room_id: String,

    /// Input file path (JSONL from sink)
    #[arg(long, env = "AOR_REPLAY_FILE", default_value = "messages.jsonl")]
    pub input_file: String,
}
