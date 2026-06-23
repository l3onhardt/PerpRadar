use chrono::{TimeZone, Utc};
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, DerivativesBlock, EventsBlock, LegacyScoresBlock, LiquidityBlock,
    OrderflowBlock, PacketProfile, PriceBlock, ScoreMeta, ScoresBlock, StandardPacket,
    StructureBlock, UniverseBlock,
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
        structure: StructureBlock::default(),
        derivatives: DerivativesBlock::default(),
        orderflow: OrderflowBlock::default(),
        scores: ScoresBlock {
            tcs: Some(0.81),
            vov: Some(1.42),
            ..ScoresBlock::default()
        },
        score_meta: std::collections::BTreeMap::new(),
        legacy_scores: LegacyScoresBlock::default(),
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
    assert_eq!(json["ts"], "2026-05-01T00:00:00Z");
    assert_eq!(json["profile"], "standard");
    assert_eq!(json["universe"]["tier"], "U2");
    assert!(json["liquidity"]["liq_5bp_usd"].is_null());
    assert_eq!(json["scores"]["TCS"], 0.81);
    assert_eq!(json["scores"]["VoV"], 1.42);
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

#[test]
fn packet_21_serializes_formal_scores_meta_and_legacy_scores() {
    let mut score_meta = std::collections::BTreeMap::new();
    score_meta.insert(
        "LRI".to_string(),
        ScoreMeta {
            available: false,
            formula: Some(
                "robust_z(0.4*z(-spread_bp)+0.3*z(liq_5bp_usd)+0.3*z(-slip_bp))"
                    .to_string(),
            ),
            direction: Some(
                "higher means stronger observed liquidity / lower immediate execution friction under the defined formula"
                    .to_string(),
            ),
            book_source: Some("full".to_string()),
            slip_notional_usd: Some(10_000.0),
            raw: None,
            z: None,
            components: serde_json::json!({}),
            missing: vec!["book_not_full".to_string()],
        },
    );

    let packet = StandardPacket {
        packet_schema: "2.1".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 15,
            focus_n: 3,
        },
        price: PriceBlock::default(),
        chart: ChartBlock {
            ema_200: Some(64100.0),
            ema50_slope: Some(0.012),
            bb_width_pctile: Some(0.42),
            atr_1h_pct: Some(0.018),
            atr_1h_pct_prev: Some(0.017),
            atr_1h_pct_delta_ratio: Some((0.018 - 0.017) / 0.017),
            ..ChartBlock::default()
        },
        liquidity: LiquidityBlock::default(),
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        structure: StructureBlock::default(),
        derivatives: DerivativesBlock::default(),
        orderflow: OrderflowBlock::default(),
        scores: ScoresBlock {
            dpi10: Some(0.12),
            ..ScoresBlock::default()
        },
        score_meta,
        legacy_scores: LegacyScoresBlock {
            candidate_score: Some(0.81),
            liquidation_event_score: Some(1.2),
            compression_score: Some(0.3),
            momentum_abs_score: Some(0.04),
            volume_spike_z: Some(1.42),
            notional_imbalance_i5: Some(0.09),
        },
        quality: QualityState::cold("full"),
    };

    let json = serde_json::to_value(&packet).unwrap();

    assert_eq!(json["packet_schema"], "2.1");
    assert!(json["scores"]["DPI10"].is_number());
    assert!(json["scores"]["LRI"].is_null());
    assert_eq!(json["score_meta"]["LRI"]["missing"][0], "book_not_full");
    assert_eq!(json["legacy_scores"]["candidate_score"], 0.81);
    assert_eq!(json["chart"]["ema_200"], 64100.0);
    assert!(json["chart"]["atr_1h_pct_delta_ratio"].is_number());
}
