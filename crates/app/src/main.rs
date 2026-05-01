use perp_radar::config::AppConfig;
use perp_radar::runtime::{build_ws_urls, serve_api, start_ingestion_tasks};
use perp_radar::supervisor::verify_required_storage;
use perp_radar_api::cache::PacketCache;

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
    let _ingestion_tasks = start_ingestion_tasks(&config, cache.clone(), ws_urls);

    serve_api(&config, cache).await
}
