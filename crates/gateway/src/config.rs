//! Gateway configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "gateway")]
#[command(about = "Gateway - Deterministic moderation and enforcement")]
pub struct GatewayConfig {
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

    /// Maximum message validation time in milliseconds
    #[arg(long, env = "AOR_GATEWAY_MAX_VALIDATION_TIME_MS", default_value = "100")]
    pub max_validation_time_ms: u64,

    /// Whether to emit detailed rejection reasons
    #[arg(long, env = "AOR_GATEWAY_VERBOSE_REJECTIONS", default_value = "true")]
    pub verbose_rejections: bool,
}
