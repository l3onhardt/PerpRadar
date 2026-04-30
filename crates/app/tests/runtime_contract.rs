use perp_radar::config::AppConfig;
use perp_radar::runtime::{build_global_market_streams, build_u1_streams, build_u2_streams};
use std::path::{Path, PathBuf};

#[test]
fn runtime_builds_expected_stream_sets() -> anyhow::Result<()> {
    let config = with_current_dir(workspace_root(), || {
        AppConfig::from_path("config/default.yaml")
    })?;

    assert_eq!(
        build_global_market_streams(),
        vec!["!markPrice@arr", "!ticker@arr", "!forceOrder@arr"]
    );

    let u1_symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
    let u1_streams = build_u1_streams(&u1_symbols);
    assert!(u1_streams.contains(&"btcusdt@kline_1m".to_string()));
    assert!(u1_streams.contains(&"ethusdt@depth20@500ms".to_string()));

    assert_eq!(
        build_u2_streams(&config.universe.always_focus),
        vec![
            "btcusdt@depth@500ms".to_string(),
            "ethusdt@depth@500ms".to_string(),
            "solusdt@depth@500ms".to_string(),
        ]
    );

    Ok(())
}

fn with_current_dir<T>(
    dir: impl AsRef<Path>,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let original = std::env::current_dir()?;
    std::env::set_current_dir(dir.as_ref())?;
    let result = f();
    std::env::set_current_dir(original)?;
    result
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
