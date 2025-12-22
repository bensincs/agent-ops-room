//! Specialist Agent configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "specialist-agent")]
#[command(about = "Specialist Agent - Perform domain-specific work")]
pub struct AgentConfig {
    /// MQTT broker host
    #[arg(long, env = "AOR_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "AOR_MQTT_PORT", default_value = "1883")]
    pub mqtt_port: u16,

    /// MQTT client ID prefix
    #[arg(long, env = "AOR_MQTT_CLIENT_ID_PREFIX", default_value = "aor")]
    pub mqtt_client_id_prefix: String,

    /// MQTT keep alive interval in seconds
    #[arg(long, env = "AOR_MQTT_KEEP_ALIVE_SECS", default_value = "60")]
    pub mqtt_keep_alive_secs: u64,

    /// Room ID
    #[arg(long, env = "AOR_ROOM_ID", default_value = "default")]
    pub room_id: String,

    /// Agent ID (unique identifier for this agent)
    #[arg(long, env = "AOR_AGENT_ID", default_value = "agent")]
    pub agent_id: String,

    /// OpenAI API key
    #[arg(long, env = "AOR_OPENAI_API_KEY")]
    pub openai_api_key: String,

    /// OpenAI model
    #[arg(long, env = "AOR_OPENAI_MODEL", default_value = "gpt-oss-120b")]
    pub openai_model: String,

    /// OpenAI base URL (for Azure or other endpoints)
    #[arg(
        long,
        env = "AOR_OPENAI_BASE_URL",
        default_value = "https://api.openai.com/v1"
    )]
    pub openai_base_url: String,

    /// Maximum messages to keep in conversation memory
    #[arg(long, env = "AOR_MAX_MEMORY_MESSAGES", default_value = "50")]
    pub max_memory_messages: usize,
}
