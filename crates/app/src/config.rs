use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
        let path = resolve_config_path(path.as_ref());
        let config = ::config::Config::builder()
            .add_source(::config::File::from(path.as_path()))
            .build()?;

        Ok(config.try_deserialize()?)
    }
}

fn resolve_config_path(path: &Path) -> PathBuf {
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}
