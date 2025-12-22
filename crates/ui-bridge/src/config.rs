//! UI Bridge configuration

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "ui-bridge")]
#[command(about = "UI Bridge - Interface between humans and MQTT")]
pub struct UiBridgeConfig {
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

    /// HTTP server host
    #[arg(long, env = "AOR_HTTP_HOST", default_value = "0.0.0.0")]
    pub http_host: String,

    /// HTTP server port
    #[arg(long, env = "AOR_HTTP_PORT", default_value = "3000")]
    pub http_port: u16,

    /// CORS allowed origins
    #[arg(long, env = "AOR_CORS_ORIGINS", default_value = "*")]
    pub cors_origins: String,
}
