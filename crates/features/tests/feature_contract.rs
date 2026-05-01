use perp_radar_core::types::Candle;
use perp_radar_features::funding::z_score;
use perp_radar_features::liquidity::liquidity_quality;
use perp_radar_features::ranking::{
    rank_candidates, rank_u0_universe, Candidate, UniverseRankingInput,
};
use perp_radar_features::scores::{composite_candidate_score, ScoreInputs};
use perp_radar_features::ta::{return_pct, simple_rsi, technical_snapshot};

fn candle(idx: usize, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Candle {
    Candle {
        symbol: "BTCUSDT".to_string(),
        open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
        close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
        open,
        high,
        low,
        close,
        volume_base: volume,
        volume_quote: volume * close,
        trades: volume as u64,
        taker_buy_base: volume * 0.6,
        taker_buy_quote: volume * close * 0.6,
        is_closed: true,
        source: "test".to_string(),
    }
}

fn fixture_candles(count: usize) -> Vec<Candle> {
    (0..count)
        .map(|idx| {
            let close = 100.0 + idx as f64 + ((idx % 5) as f64 * 0.2);
            candle(
                idx,
                close - 0.7,
                close + 1.2,
                close - 1.4,
                close,
                100.0 + idx as f64,
            )
        })
        .collect()
}

#[test]
fn return_pct_uses_decimal_return() {
    assert!((return_pct(100.0, 105.0).unwrap() - 0.05).abs() < 0.0001);
}

#[test]
fn return_pct_rejects_non_finite_inputs() {
    assert!(return_pct(f64::NAN, 105.0).is_none());
    assert!(return_pct(100.0, f64::INFINITY).is_none());
}

#[test]
fn rsi_is_high_for_monotonic_up_series() {
    let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert!(simple_rsi(&closes, 5).unwrap() > 99.0);
}

#[test]
fn rsi_is_neutral_for_flat_window() {
    let closes = vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0];
    assert_eq!(simple_rsi(&closes, 5), Some(50.0));
}

#[test]
fn rsi_rejects_non_finite_closes() {
    let closes = vec![1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0];
    assert!(simple_rsi(&closes, 5).is_none());
}

#[test]
fn funding_z_score_uses_sample_mean_and_stddev() {
    let history = vec![0.0001, 0.0002, 0.0003, 0.0004];
    let z = z_score(&history, 0.0005).unwrap();
    assert!((z - 1.9364916731037085).abs() < 0.0000000001);
}

#[test]
fn funding_z_score_rejects_non_finite_values() {
    assert!(z_score(&[0.0001, f64::NAN, 0.0003], 0.0004).is_none());
    assert!(z_score(&[0.0001, f64::INFINITY, 0.0003], 0.0004).is_none());
    assert!(z_score(&[0.0001, 0.0002, 0.0003], f64::NAN).is_none());
    assert!(z_score(&[0.0001, 0.0002, 0.0003], f64::INFINITY).is_none());
}

#[test]
fn composite_score_returns_none_when_required_input_missing() {
    let inputs = ScoreInputs {
        volume_accel_z: Some(1.0),
        ret_15m_z_abs: None,
        atr_pctile: Some(0.5),
        funding_z_abs: Some(0.4),
        liquidation_event_score: Some(0.2),
        squeeze_or_breakout_score: Some(0.3),
        liquidity_quality: Some(0.9),
    };

    assert!(composite_candidate_score(&inputs).is_none());
}

#[test]
fn composite_score_rejects_non_finite_inputs() {
    let finite = ScoreInputs {
        volume_accel_z: Some(1.0),
        ret_15m_z_abs: Some(1.0),
        atr_pctile: Some(0.5),
        funding_z_abs: Some(0.4),
        liquidation_event_score: Some(0.2),
        squeeze_or_breakout_score: Some(0.3),
        liquidity_quality: Some(0.9),
    };

    let mut nan_input = finite.clone();
    nan_input.funding_z_abs = Some(f64::NAN);
    assert!(composite_candidate_score(&nan_input).is_none());

    let mut infinite_input = finite;
    infinite_input.volume_accel_z = Some(f64::INFINITY);
    assert!(composite_candidate_score(&infinite_input).is_none());
}

#[test]
fn ranking_orders_highest_score_first() {
    let ranked = rank_candidates(vec![
        Candidate {
            symbol: "ETHUSDT".to_string(),
            score: 0.8,
        },
        Candidate {
            symbol: "BTCUSDT".to_string(),
            score: 1.2,
        },
    ]);

    assert_eq!(ranked[0].symbol, "BTCUSDT");
    assert_eq!(ranked[0].rank, 1);
}

#[test]
fn ranking_excludes_non_finite_scores() {
    let ranked = rank_candidates(vec![
        Candidate {
            symbol: "NANUSDT".to_string(),
            score: f64::NAN,
        },
        Candidate {
            symbol: "INFUSDT".to_string(),
            score: f64::INFINITY,
        },
        Candidate {
            symbol: "BTCUSDT".to_string(),
            score: 1.2,
        },
        Candidate {
            symbol: "ETHUSDT".to_string(),
            score: 0.8,
        },
    ]);

    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].symbol, "BTCUSDT");
    assert_eq!(ranked[0].rank, 1);
    assert_eq!(ranked[1].symbol, "ETHUSDT");
    assert_eq!(ranked[1].rank, 2);
}

#[test]
fn u0_ranking_combines_liquidity_stress_liquidations_and_momentum() {
    let ranked = rank_u0_universe(
        vec![
            UniverseRankingInput {
                symbol: "SLOWUSDT".to_string(),
                quote_volume_24h: Some(80_000_000.0),
                price_change_percent_24h: Some(0.2),
                funding_rate: Some(0.0001),
                liquidation_5m_usd: Some(0.0),
                ret_15m: Some(0.001),
            },
            UniverseRankingInput {
                symbol: "HOTUSDT".to_string(),
                quote_volume_24h: Some(120_000_000.0),
                price_change_percent_24h: Some(1.8),
                funding_rate: Some(0.00035),
                liquidation_5m_usd: Some(1_500_000.0),
                ret_15m: Some(0.021),
            },
            UniverseRankingInput {
                symbol: "BROKENUSDT".to_string(),
                quote_volume_24h: Some(f64::NAN),
                price_change_percent_24h: Some(9.9),
                funding_rate: Some(0.001),
                liquidation_5m_usd: Some(10_000_000.0),
                ret_15m: Some(0.2),
            },
        ],
        2,
    );

    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].symbol, "HOTUSDT");
    assert_eq!(ranked[0].rank, 1);
    assert!(ranked[0].score > ranked[1].score);
    assert_eq!(ranked[1].symbol, "SLOWUSDT");
}

#[test]
fn u0_ranking_keeps_order_stable_with_alphabetic_tiebreak() {
    let ranked = rank_u0_universe(
        vec![
            UniverseRankingInput {
                symbol: "ETHUSDT".to_string(),
                quote_volume_24h: Some(100_000_000.0),
                price_change_percent_24h: Some(1.0),
                funding_rate: Some(0.0001),
                liquidation_5m_usd: Some(0.0),
                ret_15m: None,
            },
            UniverseRankingInput {
                symbol: "BTCUSDT".to_string(),
                quote_volume_24h: Some(100_000_000.0),
                price_change_percent_24h: Some(1.0),
                funding_rate: Some(0.0001),
                liquidation_5m_usd: Some(0.0),
                ret_15m: None,
            },
        ],
        2,
    );

    assert_eq!(
        ranked
            .into_iter()
            .map(|candidate| candidate.symbol)
            .collect::<Vec<_>>(),
        vec!["BTCUSDT", "ETHUSDT"]
    );
}

#[test]
fn liquidity_quality_rejects_invalid_inputs() {
    assert!(liquidity_quality(Some(-1.0), Some(5.0)).is_none());
    assert!(liquidity_quality(Some(1.0), Some(-5.0)).is_none());
    assert!(liquidity_quality(Some(f64::NAN), Some(5.0)).is_none());
    assert!(liquidity_quality(Some(1.0), Some(f64::NAN)).is_none());
    assert!(liquidity_quality(Some(f64::INFINITY), Some(5.0)).is_none());
    assert!(liquidity_quality(Some(1.0), Some(f64::INFINITY)).is_none());
}

#[test]
fn technical_snapshot_computes_v1_chart_indicators() {
    let candles = fixture_candles(64);

    let snapshot = technical_snapshot(&candles).unwrap();

    assert!(snapshot.ema_20.unwrap() > 140.0);
    assert!(snapshot.ema_50.unwrap() > 125.0);
    assert!(snapshot.rsi_14.unwrap() > 70.0);
    assert!(snapshot.macd_histogram.unwrap().is_finite());
    assert!(snapshot.atr_pct.unwrap() > 0.0);
    assert!(snapshot.bb_width.unwrap() > 0.0);
    assert!(snapshot.adx_14.unwrap() > 0.0);
    assert!(snapshot.vwap_20.unwrap() > 140.0);
    assert!(snapshot.cmf_20.unwrap().abs() <= 1.0);
    assert_eq!(snapshot.regime.as_deref(), Some("trend_up"));
}

#[test]
fn technical_snapshot_requires_enough_closed_candles() {
    let candles = fixture_candles(10);

    assert_eq!(technical_snapshot(&candles), None);
}
