use perp_radar_features::funding::z_score;
use perp_radar_features::ranking::{rank_candidates, Candidate};
use perp_radar_features::scores::{composite_candidate_score, ScoreInputs};
use perp_radar_features::ta::{return_pct, simple_rsi};

#[test]
fn return_pct_uses_decimal_return() {
    assert!((return_pct(100.0, 105.0).unwrap() - 0.05).abs() < 0.0001);
}

#[test]
fn rsi_is_high_for_monotonic_up_series() {
    let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert!(simple_rsi(&closes, 5).unwrap() > 99.0);
}

#[test]
fn funding_z_score_uses_sample_mean_and_stddev() {
    let history = vec![0.0001, 0.0002, 0.0003, 0.0004];
    let z = z_score(&history, 0.0005).unwrap();
    assert!(z > 1.0);
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
