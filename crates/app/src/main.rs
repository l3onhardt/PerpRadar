use perp_radar::config::AppConfig;
use perp_radar::runtime::{build_ws_urls, serve_api, start_ingestion_tasks_with_storage};
use perp_radar::supervisor::verify_required_storage;
use perp_radar_api::cache::PacketCache;
use perp_radar_storage::batcher::BatchConfig;
use perp_radar_storage::clickhouse;
use perp_radar_storage::sink::{PersistEvent, StorageSink};
use perp_radar_storage::writer::spawn_clickhouse_writer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = AppConfig::from_path("config/default.yaml")?;
    verify_required_storage(&config).await?;
    let ws_urls = build_ws_urls(&config)?;

    tracing::info!(
        api_bind = %config.api.bind,
        database = %config.storage.database,
        ws_urls = ?ws_urls,
        "perp-radar serving API"
    );

    let cache = PacketCache::default();
    let storage_client =
        clickhouse::client(&config.storage.clickhouse_url, &config.storage.database);
    let (storage_tx, storage_rx) = tokio::sync::mpsc::channel::<PersistEvent>(
        config.storage.batch_rows.saturating_mul(2).max(1024),
    );
    let _storage_writer = spawn_clickhouse_writer(
        storage_client,
        BatchConfig::new(config.storage.batch_rows, config.storage.batch_interval_ms),
        storage_rx,
    );
    let _ingestion_tasks = start_ingestion_tasks_with_storage(
        &config,
        cache.clone(),
        ws_urls,
        StorageSink::channel(storage_tx),
    );

    serve_api(&config, cache).await
}
