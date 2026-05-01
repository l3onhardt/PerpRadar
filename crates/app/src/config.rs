use anyhow::{bail, Result};
use serde::Deserialize;
use std::{net::SocketAddr, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub binance: BinanceConfig,
    pub universe: UniverseConfig,
    pub storage: StorageConfig,
    pub api: ApiConfig,
    pub packets: PacketConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceConfig {
    pub market_ws_base: String,
    pub public_ws_base: String,
    pub rest_base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniverseConfig {
    pub quote_assets: Vec<String>,
    pub contract_type: String,
    pub include_status: Vec<String>,
    pub active_n: usize,
    pub focus_n: usize,
    pub refresh_sec: u64,
    pub hysteresis_rank_buffer: usize,
    pub always_focus: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub clickhouse_url: String,
    pub database: String,
    pub batch_rows: usize,
    pub batch_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PacketConfig {
    pub standard_interval_ms: u64,
    pub topk_refresh_ms: u64,
}

impl AppConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let config = ::config::Config::builder()
            .add_source(::config::File::from(path.as_ref()))
            .add_source(::config::Environment::with_prefix("PERP_RADAR").separator("__"))
            .build()?;

        let config = config.try_deserialize()?;
        validate_config(&config)?;
        Ok(config)
    }
}

fn validate_config(config: &AppConfig) -> Result<()> {
    if config.storage.clickhouse_url.trim().is_empty() {
        bail!("storage.clickhouse_url must not be empty");
    }
    if config.storage.database.trim().is_empty() {
        bail!("storage.database must not be empty");
    }
    if config.storage.batch_rows == 0 {
        bail!("storage.batch_rows must be greater than 0");
    }
    if config.storage.batch_interval_ms == 0 {
        bail!("storage.batch_interval_ms must be greater than 0");
    }
    if config.universe.active_n == 0 {
        bail!("universe.active_n must be greater than 0");
    }
    if config.universe.focus_n == 0 {
        bail!("universe.focus_n must be greater than 0");
    }
    if config.universe.focus_n > config.universe.active_n {
        bail!("universe.focus_n must be less than or equal to universe.active_n");
    }
    if config.api.bind.trim().is_empty() {
        bail!("api.bind must not be empty");
    }
    if config.api.bind.parse::<SocketAddr>().is_err() {
        bail!("api.bind must be a valid socket address");
    }
    if config.packets.standard_interval_ms == 0 {
        bail!("packets.standard_interval_ms must be greater than 0");
    }
    if config.packets.topk_refresh_ms == 0 {
        bail!("packets.topk_refresh_ms must be greater than 0");
    }

    Ok(())
}
