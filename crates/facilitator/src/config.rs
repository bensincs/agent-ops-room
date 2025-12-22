//! Facilitator configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "facilitator")]
#[command(about = "AOR Facilitator - Coordination and leadership")]
pub struct FacilitatorConfig {
    /// OpenAI/Azure API key
    #[arg(long, env = "AOR_OPENAI_API_KEY")]
    pub openai_api_key: String,

    /// Model/deployment name to use
    #[arg(long, env = "AOR_OPENAI_MODEL", default_value = "gpt-oss-120b")]
    pub openai_model: String,

    /// API base URL (OpenAI or Azure AI Foundry)
    #[arg(
        long = "openai-base-url",
        env = "AOR_OPENAI_BASE_URL",
        default_value = "https://api.openai.com/v1"
    )]
    pub openai_base_url: String,

    /// Agent heartbeat timeout in seconds
    #[arg(long, env = "AOR_AGENT_HEARTBEAT_TIMEOUT_SECS", default_value = "30")]
    pub agent_heartbeat_timeout_secs: u64,

    /// MQTT broker host
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

    /// Default mic duration in seconds
    #[arg(
        long,
        env = "AOR_FACILITATOR_DEFAULT_MIC_DURATION_SECS",
        default_value = "300"
    )]
    pub default_mic_duration_secs: u64,

    /// Default maximum messages per mic grant
    #[arg(
        long,
        env = "AOR_FACILITATOR_DEFAULT_MAX_MESSAGES",
        default_value = "10"
    )]
    pub default_max_messages: u32,
}
