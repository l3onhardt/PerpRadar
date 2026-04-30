use chrono::{TimeZone, Utc};
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::{QualityReason, QualityState};
use perp_radar_core::types::UniverseTier;

#[test]
fn standard_packet_serializes_null_metrics_and_reasons() {
    let packet = StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 15,
            focus_n: 3,
        },
        price: PriceBlock {
            last: Some(64210.5),
            mark: Some(64208.9),
            index: Some(64193.2),
            basis_bp: Some(2.45),
            ret_1m: None,
            ret_5m: None,
            ret_15m: None,
            ret_1h: None,
        },
        chart: ChartBlock::default(),
        liquidity: LiquidityBlock {
            book_mode: "partial20".to_string(),
            spread_bp: Some(0.62),
            i1: Some(0.16),
            i5: Some(0.09),
            microprice_bp: Some(0.31),
            liq_5bp_usd: None,
            liq_10bp_usd: None,
            slip_10000_buy_bp: None,
            slip_10000_sell_bp: None,
        },
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock::default(),
        quality: QualityState {
            freshness_ms: 384,
            warm: true,
            kline_gap_1m: 0,
            book_mode: "partial20".to_string(),
            book_seq_ok: None,
            book_depth_coverage_bp: Some(3.1),
            funding_history_points: 0,
            stale: false,
            reasons: vec![QualityReason::DepthCoverageLt5Bp],
        },
    };

    let json = serde_json::to_value(&packet).unwrap();

    assert_eq!(json["packet_schema"], "2.0");
    assert_eq!(json["profile"], "standard");
    assert!(json["liquidity"]["liq_5bp_usd"].is_null());
    assert_eq!(json["quality"]["reasons"][0], "depth_coverage_lt_5bp");
}

#[test]
fn quality_reasons_are_unique() {
    let mut quality = QualityState::cold("partial20");
    quality.add_reason(QualityReason::InsufficientFundingHistory);
    quality.add_reason(QualityReason::InsufficientFundingHistory);

    assert_eq!(
        quality.reasons,
        vec![QualityReason::InsufficientFundingHistory]
    );
}
