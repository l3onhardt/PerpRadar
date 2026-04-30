use perp_radar::config::AppConfig;
use perp_radar::runtime::{
    build_global_market_streams, build_u1_streams, build_u2_streams, build_ws_urls,
    serve_api_listener,
};
use perp_radar_api::cache::PacketCache;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

#[test]
fn runtime_builds_expected_ws_urls() -> anyhow::Result<()> {
    let config = with_current_dir(workspace_root(), || {
        AppConfig::from_path("config/default.yaml")
    })?;

    let urls = build_ws_urls(&config)?;

    assert_eq!(
        urls.iter().map(url::Url::as_str).collect::<Vec<_>>(),
        vec![
            "wss://fstream.binance.com/market/stream?streams=!markPrice@arr/!ticker@arr/!forceOrder@arr",
            "wss://fstream.binance.com/public/stream?streams=btcusdt@depth@500ms/ethusdt@depth@500ms/solusdt@depth@500ms",
        ]
    );

    Ok(())
}

#[tokio::test]
async fn runtime_serves_health_and_empty_top_export() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(serve_api_listener(listener, PacketCache::default()));

    let health = get(&address.to_string(), "/v1/health").await?;
    assert!(health.starts_with("HTTP/1.1 200 OK"));
    assert!(health.ends_with(r#"{"ok":true}"#));

    let top = get(&address.to_string(), "/v1/export/top.txt?limit=1").await?;
    assert!(top.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(response_body(&top), "");

    server.abort();
    Ok(())
}

async fn get(address: &str, path: &str) -> anyhow::Result<String> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    Ok(String::from_utf8(response)?)
}

fn response_body(response: &str) -> &str {
    response.split_once("\r\n\r\n").map_or("", |(_, body)| body)
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
