//! Summarizer configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "summarizer")]
#[command(about = "AOR Summarizer - Generates conversation summaries for context management")]
pub struct SummarizerConfig {
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

    /// MQTT broker host
    #[arg(long, env = "AOR_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "AOR_MQTT_PORT", default_value = "1883")]
    pub mqtt_port: u16,

    /// Room ID
    #[arg(long, env = "AOR_ROOM_ID", default_value = "default")]
    pub room_id: String,

    /// Number of messages before generating a summary
    #[arg(long, env = "AOR_SUMMARY_INTERVAL", default_value = "3")]
    pub summary_interval: u64,
}
