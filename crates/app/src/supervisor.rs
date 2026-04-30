use crate::config::AppConfig;
use anyhow::{Context, Result};
use perp_radar_storage::clickhouse;

pub async fn verify_required_storage(config: &AppConfig) -> Result<()> {
    let admin = clickhouse::admin_client(&config.storage.clickhouse_url);

    clickhouse::assert_clickhouse_ready(&admin)
        .await
        .with_context(|| {
            format!(
                "verifying ClickHouse readiness at {}",
                config.storage.clickhouse_url
            )
        })?;
    clickhouse::run_migrations(&config.storage.clickhouse_url, &config.storage.database)
        .await
        .with_context(|| {
            format!(
                "running ClickHouse migrations at {} for database {}",
                config.storage.clickhouse_url, config.storage.database
            )
        })?;

    Ok(())
}
