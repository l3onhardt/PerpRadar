#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tracing::info!("perp-radar starting");
    Ok(())
}
