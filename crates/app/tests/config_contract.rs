use perp_radar::config::AppConfig;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[test]
fn loads_default_config_contract() -> anyhow::Result<()> {
    let config = with_current_dir(workspace_root(), || {
        AppConfig::from_path("config/default.yaml")
    })?;

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

#[test]
fn environment_overrides_default_config_values() -> anyhow::Result<()> {
    let _guard = cwd_lock().lock().expect("cwd lock poisoned");
    let root = workspace_root();
    let original = std::env::current_dir()?;
    std::env::set_current_dir(&root)?;
    std::env::set_var("PERP_RADAR__API__BIND", "127.0.0.1:19090");
    std::env::set_var("PERP_RADAR__UNIVERSE__ACTIVE_N", "15");
    std::env::set_var("PERP_RADAR__UNIVERSE__FOCUS_N", "3");
    std::env::set_var("PERP_RADAR__BINANCE__REST_BASE", "http://mock-binance:9000");

    let config = AppConfig::from_path("config/default.yaml");

    std::env::remove_var("PERP_RADAR__API__BIND");
    std::env::remove_var("PERP_RADAR__UNIVERSE__ACTIVE_N");
    std::env::remove_var("PERP_RADAR__UNIVERSE__FOCUS_N");
    std::env::remove_var("PERP_RADAR__BINANCE__REST_BASE");
    std::env::set_current_dir(original)?;

    let config = config?;
    assert_eq!(config.api.bind, "127.0.0.1:19090");
    assert_eq!(config.universe.active_n, 15);
    assert_eq!(config.universe.focus_n, 3);
    assert_eq!(config.binance.rest_base, "http://mock-binance:9000");
    Ok(())
}

#[test]
fn missing_relative_config_does_not_fall_back_to_checkout_root() -> anyhow::Result<()> {
    let cwd = unique_temp_dir("missing-relative-config")?;
    let result = with_current_dir(&cwd, || AppConfig::from_path("config/default.yaml"));

    assert!(result.is_err());

    fs::remove_dir_all(cwd)?;
    Ok(())
}

#[test]
fn rejects_invalid_startup_critical_config_values() -> anyhow::Result<()> {
    type ConfigCase = (&'static str, fn(&mut CriticalConfig), &'static str);

    let cases: [ConfigCase; 11] = [
        (
            "empty-clickhouse-url",
            |config: &mut CriticalConfig| config.storage_clickhouse_url = "".to_string(),
            "storage.clickhouse_url",
        ),
        (
            "empty-database",
            |config: &mut CriticalConfig| config.storage_database = "".to_string(),
            "storage.database",
        ),
        (
            "zero-batch-rows",
            |config: &mut CriticalConfig| config.storage_batch_rows = 0,
            "storage.batch_rows",
        ),
        (
            "zero-batch-interval",
            |config: &mut CriticalConfig| config.storage_batch_interval_ms = 0,
            "storage.batch_interval_ms",
        ),
        (
            "zero-active",
            |config: &mut CriticalConfig| config.universe_active_n = 0,
            "universe.active_n",
        ),
        (
            "zero-focus",
            |config: &mut CriticalConfig| config.universe_focus_n = 0,
            "universe.focus_n",
        ),
        (
            "focus-above-active",
            |config: &mut CriticalConfig| {
                config.universe_active_n = 2;
                config.universe_focus_n = 3;
            },
            "universe.focus_n",
        ),
        (
            "empty-api-bind",
            |config: &mut CriticalConfig| config.api_bind = "".to_string(),
            "api.bind",
        ),
        (
            "invalid-api-bind",
            |config: &mut CriticalConfig| config.api_bind = "not a socket".to_string(),
            "api.bind",
        ),
        (
            "zero-standard-interval",
            |config: &mut CriticalConfig| config.packets_standard_interval_ms = 0,
            "packets.standard_interval_ms",
        ),
        (
            "zero-topk-interval",
            |config: &mut CriticalConfig| config.packets_topk_refresh_ms = 0,
            "packets.topk_refresh_ms",
        ),
    ];

    for (name, mutate, expected) in cases {
        let path = write_config_variant(name, mutate)?;
        let error = AppConfig::from_path(&path).expect_err("invalid config should be rejected");
        assert!(
            error.to_string().contains(expected),
            "expected {name} error to mention {expected}, got: {error:#}"
        );
        fs::remove_file(path)?;
    }

    Ok(())
}

fn with_current_dir<T>(
    dir: impl AsRef<Path>,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let _guard = cwd_lock().lock().expect("cwd lock poisoned");
    let original = std::env::current_dir()?;
    std::env::set_current_dir(dir.as_ref())?;
    let result = f();
    std::env::set_current_dir(original)?;
    result
}

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn unique_temp_dir(name: &str) -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("perp-radar-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct CriticalConfig {
    storage_clickhouse_url: String,
    storage_database: String,
    storage_batch_rows: usize,
    storage_batch_interval_ms: u64,
    universe_active_n: usize,
    universe_focus_n: usize,
    api_bind: String,
    packets_standard_interval_ms: u64,
    packets_topk_refresh_ms: u64,
}

impl Default for CriticalConfig {
    fn default() -> Self {
        Self {
            storage_clickhouse_url: "http://localhost:8123".to_string(),
            storage_database: "perp_radar".to_string(),
            storage_batch_rows: 2000,
            storage_batch_interval_ms: 1000,
            universe_active_n: 15,
            universe_focus_n: 3,
            api_bind: "127.0.0.1:8080".to_string(),
            packets_standard_interval_ms: 1000,
            packets_topk_refresh_ms: 1000,
        }
    }
}

fn write_config_variant(
    name: &str,
    mutate: impl FnOnce(&mut CriticalConfig),
) -> anyhow::Result<PathBuf> {
    let mut config = CriticalConfig::default();
    mutate(&mut config);

    let path = std::env::temp_dir().join(format!("perp-radar-{name}-{}.yaml", std::process::id()));
    fs::write(
        &path,
        format!(
            r#"
binance:
  market_ws_base: "wss://fstream.binance.com/market"
  public_ws_base: "wss://fstream.binance.com"
  rest_base: "https://fapi.binance.com"
universe:
  quote_assets: ["USDT"]
  contract_type: "PERPETUAL"
  include_status: ["TRADING"]
  active_n: {universe_active_n}
  focus_n: {universe_focus_n}
  refresh_sec: 300
  hysteresis_rank_buffer: 5
  always_focus: ["BTCUSDT", "ETHUSDT", "SOLUSDT"]
storage:
  clickhouse_url: "{storage_clickhouse_url}"
  database: "{storage_database}"
  batch_rows: {storage_batch_rows}
  batch_interval_ms: {storage_batch_interval_ms}
api:
  bind: "{api_bind}"
packets:
  standard_interval_ms: {packets_standard_interval_ms}
  topk_refresh_ms: {packets_topk_refresh_ms}
"#,
            universe_active_n = config.universe_active_n,
            universe_focus_n = config.universe_focus_n,
            storage_clickhouse_url = config.storage_clickhouse_url,
            storage_database = config.storage_database,
            storage_batch_rows = config.storage_batch_rows,
            storage_batch_interval_ms = config.storage_batch_interval_ms,
            api_bind = config.api_bind,
            packets_standard_interval_ms = config.packets_standard_interval_ms,
            packets_topk_refresh_ms = config.packets_topk_refresh_ms,
        ),
    )?;
    Ok(path)
}
