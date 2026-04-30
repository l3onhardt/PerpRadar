use perp_radar::config::AppConfig;

#[test]
fn loads_default_config_contract() -> anyhow::Result<()> {
    let config = AppConfig::from_path("config/default.yaml")?;

    assert_eq!(config.universe.active_n, 15);
    assert_eq!(config.universe.focus_n, 3);
    assert_eq!(
        config.universe.always_focus,
        vec![
            "BTCUSDT".to_string(),
            "ETHUSDT".to_string(),
            "SOLUSDT".to_string()
        ]
    );
    assert_eq!(config.storage.database, "perp_radar");
    assert_eq!(config.api.bind, "127.0.0.1:8080");

    Ok(())
}
