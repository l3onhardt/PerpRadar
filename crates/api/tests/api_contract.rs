use axum::body::Body;
use axum::http::{Request, StatusCode};
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
    StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
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
    let cache = PacketCache::default();
    cache.upsert(fixture_packet());
    router(cache)
}

async fn get(path: &str) -> axum::response::Response {
    app()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
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
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    assert!(text.contains("\"symbol\":\"BTCUSDT\""));
}

#[tokio::test]
async fn missing_packet_returns_not_found() {
    let response = get("/v1/packet/ETHUSDT").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
