use perp_radar::config::AppConfig;
use perp_radar::runtime::{
    build_global_market_streams, build_u1_streams, build_u2_streams, build_ws_urls,
    serve_api_listener, start_ingestion_tasks, DepthBootstrapSnapshot, RuntimeEngine,
};
use perp_radar_api::cache::PacketCache;
use perp_radar_core::types::Candle;
use perp_radar_state::book_partial::BookLevel;
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
            "wss://fstream.binance.com/market/stream?streams=btcusdt@kline_1m/ethusdt@kline_1m/solusdt@kline_1m",
            "wss://fstream.binance.com/public/stream?streams=btcusdt@depth20@500ms/ethusdt@depth20@500ms/solusdt@depth20@500ms",
            "wss://fstream.binance.com/public/stream?streams=btcusdt@depth@500ms/ethusdt@depth@500ms/solusdt@depth@500ms",
        ]
    );

    Ok(())
}

#[test]
fn runtime_engine_applies_events_and_refreshes_packet_cache() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);

    for idx in 0..64 {
        let close = 100.0 + idx as f64 + ((idx % 5) as f64 * 0.2);
        engine.apply_json(&format!(
            r#"{{
              "stream":"btcusdt@kline_1m",
              "data":{{
                "e":"kline",
                "E":1714521600000,
                "s":"BTCUSDT",
                "k":{{
                  "t":{},
                  "T":{},
                  "s":"BTCUSDT",
                  "i":"1m",
                  "o":"{}",
                  "c":"{}",
                  "h":"{}",
                  "l":"{}",
                  "v":"{}",
                  "q":"{}",
                  "n":120,
                  "V":"{}",
                  "Q":"{}",
                  "x":true
                }}
              }}
            }}"#,
            1_700_000_000_000_i64 + (idx as i64 * 60_000),
            1_700_000_059_999_i64 + (idx as i64 * 60_000),
            close - 0.7,
            close,
            close + 1.2,
            close - 1.4,
            100.0 + idx as f64,
            (100.0 + idx as f64) * close,
            (100.0 + idx as f64) * 0.6,
            (100.0 + idx as f64) * close * 0.6,
        ))?;
    }

    engine.apply_json(
        r#"{
          "stream":"!markPrice@arr",
          "data":[{
            "e":"markPriceUpdate",
            "E":1714521600000,
            "s":"BTCUSDT",
            "p":"164.0",
            "i":"163.5",
            "r":"0.0001",
            "T":1714550400000
          }]
        }"#,
    )?;
    engine.apply_json(
        r#"{
          "stream":"!ticker@arr",
          "data":[{
            "e":"24hrTicker",
            "E":1714521601000,
            "s":"BTCUSDT",
            "c":"164.2",
            "q":"123456789.5",
            "P":"1.25"
          }]
        }"#,
    )?;
    engine.apply_json(
        r#"{
          "stream":"btcusdt@depth20@500ms",
          "data":{
            "lastUpdateId":110,
            "E":1714521602000,
            "T":1714521602000,
            "bids":[["164.0","10"],["163.9","8"]],
            "asks":[["164.2","7"],["164.3","5"]]
          }
        }"#,
    )?;
    engine.apply_json(
        r#"{
          "stream":"!forceOrder@arr",
          "data":{
            "e":"forceOrder",
            "E":1714521603000,
            "o":{
              "s":"BTCUSDT",
              "S":"SELL",
              "p":"160.0",
              "q":"2.5",
              "T":1714521602500
            }
          }
        }"#,
    )?;

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(packet.price.last, Some(164.2));
    assert_eq!(packet.price.mark, Some(164.0));
    assert!(packet.chart.rsi_14.is_some());
    assert!(packet.liquidity.spread_bp.is_some());
    assert_eq!(packet.carry.funding_now, Some(0.0001));
    assert_eq!(packet.events.liq_1m_usd, Some(400.0));
    assert!(packet.scores.tcs.is_some());
    assert!(packet.quality.warm);

    Ok(())
}

#[test]
fn runtime_engine_bootstraps_closed_rest_klines_into_packet_cache() {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);
    let candles = (0..64)
        .map(|idx| {
            let close = 100.0 + idx as f64 + ((idx % 5) as f64 * 0.2);
            Candle {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
                close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
                open: close - 0.7,
                high: close + 1.2,
                low: close - 1.4,
                close,
                volume_base: 100.0 + idx as f64,
                volume_quote: (100.0 + idx as f64) * close,
                trades: 100 + idx as u64,
                taker_buy_base: (100.0 + idx as f64) * 0.6,
                taker_buy_quote: (100.0 + idx as f64) * close * 0.6,
                is_closed: true,
                source: "rest".to_string(),
            }
        })
        .collect::<Vec<_>>();

    let accepted = engine.bootstrap_klines("BTCUSDT", candles);

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(accepted, 64);
    assert!(packet.chart.ema_20.is_some());
    assert!(packet.chart.rsi_14.is_some());
    assert!(packet.quality.warm);
}

#[test]
fn runtime_engine_bootstraps_depth_snapshot_into_packet_cache() {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);

    let accepted = engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 123,
        bids: vec![BookLevel {
            price: 104.99,
            qty: 100.0,
        }],
        asks: vec![BookLevel {
            price: 105.01,
            qty: 100.0,
        }],
    });

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert!(accepted);
    assert_eq!(packet.liquidity.book_mode, "full");
    assert!(packet.liquidity.liq_5bp_usd.is_some());
}

#[test]
fn runtime_engine_bootstraps_funding_history_into_packet_cache() {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);
    engine
        .apply_json(
            r#"{
          "stream":"!markPrice@arr",
          "data":[{
            "e":"markPriceUpdate",
            "E":1714521600000,
            "s":"BTCUSDT",
            "p":"164.0",
            "i":"163.5",
            "r":"0.00015",
            "T":1714550400000
          }]
        }"#,
        )
        .unwrap();

    assert!(engine.bootstrap_funding_history("BTCUSDT", vec![0.00005, 0.0001, 0.00015, 0.0002]));

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert!(packet.carry.funding_z_7d.is_some());
    assert_eq!(packet.quality.funding_history_points, 4);
}

#[test]
fn runtime_engine_applies_full_depth_delta_after_snapshot() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);
    assert!(engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 10,
        bids: vec![BookLevel {
            price: 100.0,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 100.1,
            qty: 6.0,
        }],
    }));

    engine.apply_json(
        r#"{
          "stream":"btcusdt@depth@500ms",
          "data":{
            "e":"depthUpdate",
            "E":1714521600000,
            "T":1714521600000,
            "s":"BTCUSDT",
            "U":8,
            "u":11,
            "pu":7,
            "b":[["100.0","12.0"]],
            "a":[]
          }
        }"#,
    )?;

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(packet.quality.book_seq_ok, Some(true));
    assert!(packet.liquidity.liq_5bp_usd.unwrap() > 1_700.0);

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

#[tokio::test]
async fn start_ingestion_tasks_accepts_ws_urls_and_returns_task_handles() -> anyhow::Result<()> {
    let config = with_current_dir(workspace_root(), || {
        AppConfig::from_path("config/default.yaml")
    })?;
    let urls = build_ws_urls(&config)?;
    let cache = PacketCache::default();

    let handles = start_ingestion_tasks(&config, cache, urls.clone());

    assert_eq!(handles.len(), urls.len() + 1);
    for handle in handles {
        handle.abort();
    }

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
