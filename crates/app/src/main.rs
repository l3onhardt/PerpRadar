#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = perp_radar::config::AppConfig::from_path("config/default.yaml")?;
    perp_radar::supervisor::verify_required_storage(&config).await?;

    tracing::info!(
        api_bind = %config.api.bind,
        database = %config.storage.database,
        "perp-radar ready"
    );

    Ok(())
}
