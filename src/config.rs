use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "nanit-proxy",
    about = "Lightweight Nanit Baby Monitor Proxy for Home Assistant & go2rtc",
    version,
    author = "IdleCharm"
)]
pub struct Config {
    /// Long-lived Nanit refresh token (obtained via `nanit-proxy auth`)
    #[arg(long, env = "NANIT_REFRESH_TOKEN")]
    pub refresh_token: Option<String>,

    /// Nanit account email (used for initial login or `auth` subcommand)
    #[arg(long, env = "NANIT_EMAIL")]
    pub email: Option<String>,

    /// Nanit account password (used for initial login or `auth` subcommand)
    #[arg(long, env = "NANIT_PASSWORD")]
    pub password: Option<String>,

    /// Target RTMP host:port where the Nanit camera pushes its stream
    #[arg(long, env = "NANIT_RTMP_ADDR", default_value = "127.0.0.1:1935")]
    pub rtmp_addr: String,

    /// Unique ID of the baby/camera to stream (optional, auto-detected if only one camera exists)
    #[arg(long, env = "NANIT_BABY_UID")]
    pub baby_uid: Option<String>,

    /// Custom device name for Home Assistant MQTT Discovery
    #[arg(
        long,
        env = "NANIT_DEVICE_NAME",
        default_value = "Nanit Nursery Camera"
    )]
    pub device_name: String,

    /// Temperature unit for Home Assistant: 'c' or 'f'
    #[arg(long, env = "NANIT_TEMP_UNIT", default_value = "f")]
    pub temp_unit: String,

    /// Enable MQTT publishing for Home Assistant sensors
    #[arg(long, env = "NANIT_MQTT_ENABLED", default_value_t = false)]
    pub mqtt_enabled: bool,

    /// MQTT broker URL (e.g. mqtt://127.0.0.1:1883)
    #[arg(
        long,
        env = "NANIT_MQTT_BROKER_URL",
        default_value = "mqtt://127.0.0.1:1883"
    )]
    pub mqtt_broker_url: String,

    /// MQTT topic prefix for Home Assistant Discovery
    #[arg(long, env = "NANIT_MQTT_PREFIX", default_value = "homeassistant")]
    pub mqtt_prefix: String,

    /// MQTT Client ID
    #[arg(long, env = "NANIT_MQTT_CLIENT_ID", default_value = "nanit-proxy")]
    pub mqtt_client_id: String,

    /// MQTT username
    #[arg(long, env = "NANIT_MQTT_USERNAME")]
    pub mqtt_username: Option<String>,

    /// MQTT password
    #[arg(long, env = "NANIT_MQTT_PASSWORD")]
    pub mqtt_password: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Interactively log in and obtain a long-lived refresh token
    Auth,
    /// List registered babies and cameras on your Nanit account
    ListBabies,
    /// Run the streaming proxy and sensor telemetry daemon (default)
    Run,
}
