use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{TimeZone, Utc};
use perp_radar_api::cache::PacketCache;
use perp_radar_api::routes::router;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityState;
use perp_radar_core::types::UniverseTier;
use tower::ServiceExt;

fn fixture_packet() -> StandardPacket {
    packet_with("BTCUSDT", 1)
}

fn packet_with(symbol: &str, rank: usize) -> StandardPacket {
    StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        symbol: symbol.to_string(),
        rank,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 42,
            focus_n: 10,
        },
        price: PriceBlock {
            last: Some(64210.5),
            mark: Some(64208.9),
            index: Some(64193.2),
            basis_bp: Some(2.45),
            ret_1m: Some(0.01),
            ret_5m: Some(0.12),
            ret_15m: None,
            ret_1h: Some(-0.2),
        },
        chart: ChartBlock {
            regime: Some("trend".to_string()),
            signature: Some("higher_high".to_string()),
            rsi_14: Some(58.0),
            atr_pct: Some(0.012),
            bb_width: Some(0.05),
            ..ChartBlock::default()
        },
        liquidity: LiquidityBlock {
            book_mode: "partial20".to_string(),
            spread_bp: Some(0.62),
            i1: Some(0.16),
            i5: Some(0.09),
            microprice_bp: Some(0.31),
            liq_5bp_usd: Some(1_250_000.0),
            liq_10bp_usd: Some(2_500_000.0),
            slip_10000_buy_bp: Some(0.8),
            slip_10000_sell_bp: Some(0.9),
        },
        carry: CarryBlock {
            funding_now: Some(0.0001),
            funding_unit: Some("8h".to_string()),
            funding_interval_hours: Some(8),
            funding_z_7d: Some(0.7),
            next_funding_time: Some(Utc.with_ymd_and_hms(2026, 5, 1, 8, 0, 0).unwrap()),
        },
        events: EventsBlock {
            liq_1m_usd: Some(10_000.0),
            liq_5m_usd: Some(50_000.0),
            liq_15m_usd: Some(80_000.0),
            liq_side: Some("long".to_string()),
            volume_spike_z: Some(1.8),
        },
        scores: ScoresBlock {
            tcs: Some(0.81),
            lri: Some(0.22),
            dpi5: Some(0.33),
            csi: Some(0.44),
            rpi: Some(0.55),
            vov: Some(1.42),
        },
        quality: QualityState {
            freshness_ms: 384,
            warm: true,
            kline_gap_1m: 0,
            book_mode: "partial20".to_string(),
            book_seq_ok: Some(true),
            book_depth_coverage_bp: Some(5.0),
            funding_history_points: 200,
            stale: false,
            reasons: Vec::new(),
        },
    }
}

fn app() -> axum::Router {
    app_with(vec![fixture_packet()])
}

fn app_with(packets: Vec<StandardPacket>) -> axum::Router {
    let cache = PacketCache::default();
    for packet in packets {
        cache.upsert(packet);
    }
    router(cache)
}

async fn get(path: &str) -> axum::response::Response {
    get_from(app(), path).await
}

async fn get_from(app: axum::Router, path: &str) -> axum::response::Response {
    app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn packet_route_returns_cached_packet_json() {
    let response = get("/v1/packet/BTCUSDT").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["symbol"], "BTCUSDT");
}

#[tokio::test]
async fn export_top_text_returns_llm_readable_packet() {
    let response = get("/v1/export/top.txt?limit=1").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    assert!(text.contains("[BTCUSDT]"));
    assert!(text.contains("chart:"));
    assert!(text.contains("rsi14=58"));
    assert!(text.contains("quality:"));
}

#[tokio::test]
async fn schema_route_returns_ok() {
    let response = get("/v1/schema").await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn export_top_jsonl_returns_packet_json_lines() {
    let response = get("/v1/export/top.jsonl?limit=1").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/x-ndjson; charset=utf-8"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let json: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(json["symbol"], "BTCUSDT");
}

#[tokio::test]
async fn missing_packet_returns_not_found() {
    let response = get("/v1/packet/ETHUSDT").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn packet_route_accepts_lowercase_symbol() {
    let response = get("/v1/packet/btcusdt").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["symbol"], "BTCUSDT");
}

#[tokio::test]
async fn top_packets_are_ranked_by_rank_then_symbol_and_truncated() {
    let app = app_with(vec![
        packet_with("SOLUSDT", 2),
        packet_with("ETHUSDT", 2),
        packet_with("BTCUSDT", 1),
    ]);

    let response = get_from(app, "/v1/packets/top?limit=2").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json.as_array().unwrap().len(), 2);
    assert_eq!(json[0]["symbol"], "BTCUSDT");
    assert_eq!(json[1]["symbol"], "ETHUSDT");
}

#[tokio::test]
async fn export_limit_zero_returns_empty_response() {
    let response = get("/v1/export/top.jsonl?limit=0").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    assert!(body.is_empty());
}

#[tokio::test]
async fn huge_export_limit_is_clamped_to_maximum() {
    let packets = (0..105)
        .map(|idx| packet_with(&format!("T{idx:03}USDT"), idx + 1))
        .collect();
    let response = get_from(app_with(packets), "/v1/export/top.jsonl?limit=1000").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    let lines = text.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 100);
    assert!(lines
        .iter()
        .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()));
}
