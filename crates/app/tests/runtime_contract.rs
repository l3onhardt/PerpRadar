use perp_radar::config::AppConfig;
use perp_radar::runtime::{
    build_global_market_streams, build_u1_streams, build_u2_streams, build_ws_urls,
    serve_api_listener, start_ingestion_tasks, DepthBootstrapSnapshot, RuntimeEngine,
    RuntimeEngineConfig,
};
use perp_radar_api::cache::PacketCache;
use perp_radar_core::types::Candle;
use perp_radar_state::book_partial::BookLevel;
use perp_radar_storage::sink::{PersistEvent, StorageSink};
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
            "wss://fstream.binance.com/stream?streams=btcusdt@depth20@500ms/ethusdt@depth20@500ms/solusdt@depth20@500ms",
            "wss://fstream.binance.com/stream?streams=btcusdt@depth@500ms/ethusdt@depth@500ms/solusdt@depth@500ms",
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
    engine.apply_open_interest("BTCUSDT", 105_000.0, 1714521604000);
    let packet = cache.get("BTCUSDT").expect("packet is refreshed after oi");
    assert_eq!(packet.derivatives.oi, Some(105_000.0));
    assert_eq!(packet.derivatives.oi_notional_usd, Some(17_220_000.0));
    assert_eq!(packet.derivatives.oi_z, None);
    assert_eq!(packet.events.liq_1m_usd, Some(400.0));
    assert_eq!(packet.scores.tcs, None);
    assert!(packet.legacy_scores.candidate_score.is_some());
    assert!(packet.quality.warm);

    Ok(())
}

#[test]
fn runtime_engine_promotes_u0_symbols_into_active_and_focus_pools() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::with_config(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 2,
            focus_n: 1,
            stale_after_ms: 5_000,
            funding_interval_hours: 8,
        },
    );

    engine.apply_json(
        r#"{
          "stream":"!ticker@arr",
          "data":[
            {"e":"24hrTicker","E":1714521601000,"s":"BTCUSDT","c":"100.0","q":"100000000","P":"1.0"},
            {"e":"24hrTicker","E":1714521601000,"s":"HOTUSDT","c":"50.0","q":"200000000","P":"3.0"},
            {"e":"24hrTicker","E":1714521601000,"s":"MIDUSDT","c":"20.0","q":"90000000","P":"2.0"}
          ]
        }"#,
    )?;
    engine.apply_json(
        r#"{
          "stream":"!markPrice@arr",
          "data":[
            {"e":"markPriceUpdate","E":1714521601000,"s":"BTCUSDT","p":"100.0","i":"99.9","r":"0.0001","T":1714550400000},
            {"e":"markPriceUpdate","E":1714521601000,"s":"HOTUSDT","p":"50.0","i":"49.9","r":"0.0004","T":1714550400000},
            {"e":"markPriceUpdate","E":1714521601000,"s":"MIDUSDT","p":"20.0","i":"19.9","r":"0.0001","T":1714550400000}
          ]
        }"#,
    )?;
    engine.recompute_universe();

    let debug = engine.debug_snapshot();
    assert_eq!(debug.active_symbols.len(), 2);
    assert_eq!(debug.focus_symbols, vec!["HOTUSDT"]);
    assert!(debug.active_symbols.contains(&"BTCUSDT".to_string()));
    assert!(debug.active_symbols.contains(&"HOTUSDT".to_string()));
    assert!(cache.get("HOTUSDT").is_some());
    assert!(cache.get("MIDUSDT").is_none());

    Ok(())
}

#[test]
fn runtime_engine_does_not_cache_non_active_global_ticker_symbols() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::with_config(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 1,
            focus_n: 1,
            stale_after_ms: 5_000,
            funding_interval_hours: 8,
        },
    );

    engine.apply_json(
        r#"{
          "stream":"!ticker@arr",
          "data":[
            {"e":"24hrTicker","E":1714521601000,"s":"BTCUSDT","c":"100.0","q":"100000000","P":"1.0"},
            {"e":"24hrTicker","E":1714521601000,"s":"COLDUSDT","c":"50.0","q":"200000000","P":"3.0"}
          ]
        }"#,
    )?;

    assert!(cache.get("BTCUSDT").is_some());
    assert!(cache.get("COLDUSDT").is_none());

    Ok(())
}

#[test]
fn runtime_engine_ages_packets_and_debugs_runtime_state() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::with_config(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 1,
            focus_n: 1,
            stale_after_ms: 5_000,
            funding_interval_hours: 4,
        },
    );
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

    engine.age_all(1_714_521_607_500);
    let packet = cache.get("BTCUSDT").expect("packet is cached");
    let debug = engine.debug_snapshot();

    assert!(packet.quality.stale);
    assert_eq!(packet.quality.freshness_ms, 7_500);
    assert_eq!(packet.carry.funding_interval_hours, Some(4));
    assert_eq!(debug.packet_count, 1);
    assert!(debug.stale_symbols.contains(&"BTCUSDT".to_string()));

    Ok(())
}

#[test]
fn runtime_engine_updates_non_stale_packet_freshness() -> anyhow::Result<()> {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::with_config(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 1,
            focus_n: 1,
            stale_after_ms: 5_000,
            funding_interval_hours: 8,
        },
    );

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

    engine.age_all(1_714_521_601_250);
    let packet = cache.get("BTCUSDT").expect("packet is cached");

    assert!(!packet.quality.stale);
    assert_eq!(packet.quality.freshness_ms, 1_250);
    assert!(!packet
        .quality
        .reasons
        .contains(&perp_radar_core::quality::QualityReason::StaleMarketData));

    Ok(())
}

#[test]
fn runtime_engine_marks_full_book_gap_and_recovers_after_snapshot() -> anyhow::Result<()> {
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
            "U":14,
            "u":15,
            "pu":12,
            "b":[],
            "a":[]
          }
        }"#,
    )?;
    engine.age_all(1_714_521_601_000);
    let gap_packet = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(gap_packet.quality.book_seq_ok, Some(false));
    assert_eq!(engine.debug_snapshot().full_book_resync_needed, 1);

    assert!(engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 20,
        bids: vec![BookLevel {
            price: 101.0,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 101.1,
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
            "U":18,
            "u":21,
            "pu":17,
            "b":[["101.0","12.0"]],
            "a":[]
          }
        }"#,
    )?;
    engine.age_all(1_714_521_602_000);

    let recovered = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(recovered.quality.book_seq_ok, Some(true));
    assert_eq!(engine.debug_snapshot().full_book_resync_needed, 0);
    assert!(recovered.liquidity.liq_5bp_usd.unwrap() > 1_000.0);

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

#[tokio::test]
async fn runtime_engine_emits_persistence_event_after_packet_refresh() {
    let cache = PacketCache::default();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    let mut engine = RuntimeEngine::with_config_and_storage(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 1,
            focus_n: 1,
            stale_after_ms: 15_000,
            funding_interval_hours: 8,
        },
        StorageSink::channel(sender),
    );

    assert!(engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
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
    }));

    let cached = cache.get("BTCUSDT").expect("packet is cached");
    let event = receiver.recv().await.expect("persistence event is queued");

    match event {
        PersistEvent::Packet(packet) => {
            assert_eq!(packet.symbol, cached.symbol);
            assert_eq!(packet.ts, cached.ts);
        }
    }
}

#[tokio::test]
async fn runtime_engine_coalesces_depth_events_until_age_tick() {
    let cache = PacketCache::default();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let mut engine = RuntimeEngine::with_config_and_storage(
        vec!["BTCUSDT".to_string()],
        cache.clone(),
        RuntimeEngineConfig {
            candle_capacity: 100,
            active_n: 1,
            focus_n: 1,
            stale_after_ms: 15_000,
            funding_interval_hours: 8,
        },
        StorageSink::channel(sender),
    );

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
    receiver.recv().await.expect("bootstrap emits one packet");

    engine
        .apply_json(
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
        )
        .unwrap();
    engine
        .apply_json(
            r#"{
          "stream":"btcusdt@depth@500ms",
          "data":{
            "e":"depthUpdate",
            "E":1714521600100,
            "T":1714521600100,
            "s":"BTCUSDT",
            "U":12,
            "u":12,
            "pu":11,
            "b":[["100.0","14.0"]],
            "a":[]
          }
        }"#,
        )
        .unwrap();

    assert!(receiver.try_recv().is_err());

    engine.age_all(1_714_521_601_000);

    let event = receiver
        .recv()
        .await
        .expect("age tick emits coalesced packet");
    match event {
        PersistEvent::Packet(packet) => {
            assert_eq!(packet.symbol, "BTCUSDT");
            assert!(packet.liquidity.liq_5bp_usd.unwrap() > 1_700.0);
        }
    }
    assert!(receiver.try_recv().is_err());
}

#[test]
fn runtime_engine_disabled_storage_sink_preserves_cache_behavior() {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);

    assert!(engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
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
    }));

    assert!(cache.get("BTCUSDT").is_some());
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
fn runtime_engine_bootstraps_premium_index_into_packet_cache() {
    let cache = PacketCache::default();
    let mut engine = RuntimeEngine::new(vec!["BTCUSDT".to_string()], cache.clone(), 100);

    assert!(engine.bootstrap_premium_index(
        "BTCUSDT",
        78493.9,
        78529.48,
        -0.00001334,
        1_777_651_200_000,
        1_777_649_155_003,
    ));

    let packet = cache.get("BTCUSDT").expect("packet is cached");
    assert_eq!(packet.price.mark, Some(78493.9));
    assert_eq!(packet.price.index, Some(78529.48));
    assert_eq!(packet.carry.funding_now, Some(-0.00001334));
    assert!(packet.carry.next_funding_time.is_some());
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
    engine.age_all(1_714_521_601_000);

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

    assert_eq!(handles.len(), urls.len() + 2);
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
