use once_cell::sync::Lazy;
use serde::Deserialize;
use std::time::Instant;

pub static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

pub const LOG_LEVEL_FILTER_RELEASE: log::LevelFilter = log::LevelFilter::Info;
pub const LOG_LEVEL_FILTER_DEBUG: log::LevelFilter = log::LevelFilter::Debug;
pub const OUTPUT_DIR: &str = "output";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub scanning: ScanningConfig,
    pub timeouts: TimeoutsConfig,
    pub networking: NetworkingConfig,
    pub minecraft: MinecraftConfig,
    pub test_servers: TestServersConfig,
    pub stats: StatsConfig,
    pub discord: DiscordConfig,
}

#[derive(Debug, Deserialize)]
pub struct ScanningConfig {
    pub port: u16,
    pub num_tasks: usize,
    pub max_range_size: usize,
    pub consecutive_threshold: usize,
    pub chunk_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct TimeoutsConfig {
    pub port_check_ms: u64,
    pub connection_ms: u64,
    pub protocol_response_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct NetworkingConfig {
    pub base_source_port: u16,
    pub port_range_per_task: u16,
}

#[derive(Debug, Deserialize)]
pub struct MinecraftConfig {
    pub protocol_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct TestServersConfig {
    pub test_ips: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatsConfig {
    pub stats_interval_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DiscordConfig {
    pub webhook_121_active: String,
    pub webhook_120_active: String,
    pub webhook_119_active: String,
    pub webhook_other_active: String,

    pub webhook_121_empty: String,
    pub webhook_120_empty: String,
    pub webhook_119_empty: String,
    pub webhook_other_empty: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string("config.toml")?;
        let config: Config = toml::from_str(&config_str)?;
        Ok(config)
    }
}
