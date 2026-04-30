use crate::config::AppConfig;
use anyhow::Result;
use perp_radar_storage::clickhouse;

pub async fn verify_required_storage(config: &AppConfig) -> Result<()> {
    let admin = clickhouse::admin_client(&config.storage.clickhouse_url);

    clickhouse::assert_clickhouse_ready(&admin).await?;
    clickhouse::run_migrations(&config.storage.clickhouse_url, &config.storage.database).await?;

    Ok(())
}
