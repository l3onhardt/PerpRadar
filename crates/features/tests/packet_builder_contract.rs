use perp_radar_core::packet::PacketProfile;
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::{Candle, UniverseTier};
use perp_radar_features::packet_builder::build_standard_packet;
use perp_radar_state::book_partial::BookLevel;
use perp_radar_state::symbol_state::{
    ForceOrderUpdate, FullDepthSnapshotUpdate, KlineUpdate, MarkPriceUpdate, PartialDepthUpdate,
    SymbolState, TickerUpdate,
};

fn closed_candle(symbol: &str, open_time_ms: i64, close: f64) -> Candle {
    Candle {
        symbol: symbol.to_string(),
        open_time_ms,
        close_time_ms: open_time_ms + 59_999,
        open: close,
        high: close,
        low: close,
        close,
        volume_base: 1.0,
        volume_quote: close,
        trades: 1,
        taker_buy_base: 0.5,
        taker_buy_quote: close * 0.5,
        is_closed: true,
        source: "test".to_string(),
    }
}

fn closed_colored_candle(symbol: &str, open_time_ms: i64, open: f64, close: f64) -> Candle {
    let mut candle = closed_candle(symbol, open_time_ms, close);
    candle.open = open;
    candle.high = open.max(close);
    candle.low = open.min(close);
    candle
}

fn apply_closes(state: &mut SymbolState, closes: &[f64]) {
    for (idx, close) in closes.iter().enumerate() {
        state.apply_kline(KlineUpdate {
            candle: closed_candle("BTCUSDT", 1_700_000_000_000 + (idx as i64 * 60_000), *close),
        });
    }
}

#[test]
fn standard_packet_uses_symbol_price_and_quality_from_state() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_kline(KlineUpdate {
        candle: closed_candle("BTCUSDT", 1_700_000_000_000, 42_000.0),
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.symbol, "BTCUSDT");
    assert_eq!(packet.packet_schema, "2.1");
    assert_eq!(packet.profile, PacketProfile::Standard);
    assert_eq!(packet.universe.tier, UniverseTier::U2);
    assert_eq!(packet.universe.active_n, 15);
    assert_eq!(packet.universe.focus_n, 3);
    assert_eq!(packet.price.last, Some(42_000.0));
    assert!(packet.price.ret_5m.is_none());
    assert_eq!(packet.chart.signature, Some("1m:DOJI".to_string()));
    assert_eq!(packet.liquidity.book_mode, "none");
    assert!(packet.scores.lri.is_none());
    assert!(packet.scores.dpi10.is_none());
    assert!(packet.score_meta.contains_key("LRI"));
    assert!(packet.score_meta["LRI"]
        .missing
        .contains(&"book_not_full".to_string()));
    assert!(!packet.quality.reasons.is_empty());
    assert!(packet
        .quality
        .reasons
        .contains(&QualityReason::InsufficientKlineHistory));
}

#[test]
fn chart_signature_keeps_only_last_twelve_candle_colors() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    let color_points = [
        (10.0, 11.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (10.0, 11.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (10.0, 11.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (10.0, 11.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (10.0, 11.0),
    ];

    for (idx, (open, close)) in color_points.into_iter().enumerate() {
        state.apply_kline(KlineUpdate {
            candle: closed_colored_candle(
                "BTCUSDT",
                1_700_000_000_000 + (idx as i64 * 60_000),
                open,
                close,
            ),
        });
    }

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(
        packet.chart.signature,
        Some("1m:R,DOJI,G,R,DOJI,G,R,DOJI,G,R,DOJI,G".to_string())
    );
}

#[test]
fn invalid_five_minute_return_input_does_not_add_history_reason() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    apply_closes(&mut state, &[0.0, 101.0, 102.0, 103.0, 104.0, 105.0]);

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert!(packet.price.ret_5m.is_none());
    assert!(!packet
        .quality
        .reasons
        .contains(&QualityReason::InsufficientKlineHistory));
}

#[test]
fn clean_history_computes_one_and_five_minute_returns_from_tail() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    apply_closes(&mut state, &[100.0, 110.0, 120.0, 130.0, 140.0, 150.0]);

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.price.ret_1m, Some((150.0 - 140.0) / 140.0));
    assert_eq!(packet.price.ret_5m, Some(0.5));
}

#[test]
fn standard_packet_includes_v1_technical_indicators_when_warm() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    for idx in 0..64 {
        let close = 100.0 + idx as f64 + ((idx % 5) as f64 * 0.2);
        state.apply_kline(KlineUpdate {
            candle: Candle {
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
                source: "test".to_string(),
            },
        });
    }

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert!(packet.chart.ema_20.unwrap() > 140.0);
    assert!(packet.chart.ema_50.unwrap() > 125.0);
    assert!(packet.chart.rsi_14.unwrap() > 70.0);
    assert!(packet.chart.macd_histogram.unwrap().is_finite());
    assert!(packet.chart.atr_pct.unwrap() > 0.0);
    assert!(packet.chart.bb_width.unwrap() > 0.0);
    assert!(packet.chart.adx_14.unwrap() > 0.0);
    assert!(packet.chart.vwap_20.unwrap() > 140.0);
    assert!(packet.chart.cmf_20.unwrap().abs() <= 1.0);
    assert_eq!(packet.chart.regime.as_deref(), Some("trend_up"));
}

#[test]
fn standard_packet_includes_market_liquidity_carry_and_event_state() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    apply_closes(&mut state, &[100.0, 101.0, 102.0, 103.0, 104.0, 105.0]);
    state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 106.0,
        index_price: 105.5,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    });
    state.apply_funding_history("BTCUSDT", vec![0.00005, 0.0001, 0.00015, 0.0002]);
    state.apply_ticker(TickerUpdate {
        symbol: "BTCUSDT".to_string(),
        last_price: 106.25,
        quote_volume_24h: 123_000_000.0,
        price_change_percent_24h: 1.25,
        event_time_ms: 1_714_521_601_000,
    });
    state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 42,
        bids: vec![
            BookLevel {
                price: 105.9,
                qty: 100.0,
            },
            BookLevel {
                price: 105.8,
                qty: 80.0,
            },
        ],
        asks: vec![
            BookLevel {
                price: 106.1,
                qty: 70.0,
            },
            BookLevel {
                price: 106.2,
                qty: 60.0,
            },
        ],
        event_time_ms: 1_714_521_602_000,
    });
    state.apply_force_order(ForceOrderUpdate {
        symbol: "BTCUSDT".to_string(),
        side: "SELL".to_string(),
        price: 100.0,
        qty: 2.0,
        event_time_ms: 1_714_521_603_000,
        order_time_ms: 1_714_521_602_500,
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.price.last, Some(106.25));
    assert_eq!(packet.price.mark, Some(106.0));
    assert_eq!(packet.price.index, Some(105.5));
    assert_eq!(
        packet.price.basis_bp,
        Some((106.0 - 105.5) / 105.5 * 10_000.0)
    );
    assert!(packet.liquidity.spread_bp.unwrap() > 0.0);
    assert!(packet.liquidity.i1.unwrap() > 0.0);
    assert!(packet.liquidity.i5.unwrap() > 0.0);
    assert!(packet.liquidity.microprice_bp.unwrap().is_finite());
    assert_eq!(packet.carry.funding_now, Some(0.0001));
    assert_eq!(packet.carry.funding_unit.as_deref(), Some("8h_estimate"));
    assert!(packet.carry.funding_z_7d.is_some());
    assert_eq!(packet.events.liq_1m_usd, Some(200.0));
    assert_eq!(packet.events.liq_15m_usd, Some(200.0));
    assert_eq!(packet.events.liq_side.as_deref(), Some("long"));
    assert!(packet.events.volume_spike_z.is_some());
    assert!(packet.legacy_scores.candidate_score.is_some());
}

#[test]
fn standard_packet_tcs_remains_available_without_recent_liquidations() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    for idx in 0..64 {
        let close = 100.0 + idx as f64;
        state.apply_kline(KlineUpdate {
            candle: Candle {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
                close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
                open: close - 0.5,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume_base: 100.0 + idx as f64,
                volume_quote: (100.0 + idx as f64) * close,
                trades: 100 + idx as u64,
                taker_buy_base: (100.0 + idx as f64) * 0.5,
                taker_buy_quote: (100.0 + idx as f64) * close * 0.5,
                is_closed: true,
                source: "test".to_string(),
            },
        });
    }
    state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 164.0,
        index_price: 163.5,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    });
    state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 42,
        bids: vec![BookLevel {
            price: 163.9,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 164.1,
            qty: 8.0,
        }],
        event_time_ms: 1_714_521_602_000,
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.events.liq_5m_usd, None);
    assert_eq!(packet.scores.lri, None);
    assert_eq!(packet.scores.tcs, None);
    assert!(packet.legacy_scores.candidate_score.is_some());
}

#[test]
fn packet_builder_moves_old_score_meanings_to_legacy_scores() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    for idx in 0..64 {
        let close = 100.0 + idx as f64;
        state.apply_kline(KlineUpdate {
            candle: Candle {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
                close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
                open: close - 0.5,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume_base: 100.0 + idx as f64,
                volume_quote: (100.0 + idx as f64) * close,
                trades: 100 + idx as u64,
                taker_buy_base: (100.0 + idx as f64) * 0.5,
                taker_buy_quote: (100.0 + idx as f64) * close * 0.5,
                is_closed: true,
                source: "test".to_string(),
            },
        });
    }
    state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 164.0,
        index_price: 163.5,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    });
    state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 42,
        bids: vec![BookLevel {
            price: 163.9,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 164.1,
            qty: 8.0,
        }],
        event_time_ms: 1_714_521_602_000,
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.scores.tcs, None);
    assert!(packet.legacy_scores.candidate_score.is_some());
    assert_eq!(packet.legacy_scores.notional_imbalance_i5, packet.liquidity.i5);
    assert_eq!(
        packet.legacy_scores.volume_spike_z,
        packet.events.volume_spike_z
    );
}

#[test]
fn packet_builder_uses_full_book_qty_imbalance_for_dpi5_and_dpi10() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 123,
        bids: (0..10)
            .map(|idx| BookLevel {
                price: 100.0 - idx as f64 * 0.01,
                qty: 10.0,
            })
            .collect(),
        asks: (0..10)
            .map(|idx| BookLevel {
                price: 100.1 + idx as f64 * 0.01,
                qty: 5.0,
            })
            .collect(),
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.scores.dpi5, Some((50.0 - 25.0) / 75.0));
    assert_eq!(packet.scores.dpi10, Some((100.0 - 50.0) / 150.0));
    assert_eq!(packet.score_meta["DPI5"].book_source.as_deref(), Some("full"));
    assert!(packet.score_meta["DPI5"].missing.is_empty());
    assert!(packet.score_meta["DPI10"].missing.is_empty());
}

#[test]
fn packet_builder_computes_lri_from_full_book_history_not_liquidations() {
    let mut state = SymbolState::new("BTCUSDT", 240);
    for idx in 0..80 {
        state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
            symbol: "BTCUSDT".to_string(),
            last_update_id: 1_000 + idx as u64,
            bids: vec![
                BookLevel {
                    price: 100.0,
                    qty: 40.0 + idx as f64,
                },
                BookLevel {
                    price: 99.99,
                    qty: 40.0 + idx as f64,
                },
            ],
            asks: vec![
                BookLevel {
                    price: 100.01,
                    qty: 35.0 + idx as f64,
                },
                BookLevel {
                    price: 100.02,
                    qty: 35.0 + idx as f64,
                },
            ],
        });
    }

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert!(packet.events.liq_5m_usd.is_none());
    assert!(packet.scores.lri.is_some());
    assert!(packet.score_meta["LRI"].available);
    assert!(packet.score_meta["LRI"].missing.is_empty());
    assert_eq!(
        packet.score_meta["LRI"].direction.as_deref(),
        Some(
            "higher means stronger observed liquidity / lower immediate execution friction under the defined formula"
        )
    );
}

#[test]
fn packet_builder_computes_csi_rpi_vov_from_formal_inputs() {
    let mut state = SymbolState::new("BTCUSDT", 260);
    state.apply_funding_history("BTCUSDT", vec![-0.0002, -0.0001, 0.0, 0.0001, 0.0002]);
    for idx in 0..90 {
        let oscillation = ((idx % 10) as f64 - 5.0) * 0.6;
        let close = 100.0 + idx as f64 * 0.25 + oscillation;
        state.apply_kline(KlineUpdate {
            candle: Candle {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
                close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
                open: close - 0.6,
                high: close + 1.4 + (idx % 5) as f64 * 0.05,
                low: close - 1.1,
                close,
                volume_base: 100.0 + idx as f64,
                volume_quote: (100.0 + idx as f64) * close,
                trades: 100 + idx as u64,
                taker_buy_base: (100.0 + idx as f64) * 0.6,
                taker_buy_quote: (100.0 + idx as f64) * close * 0.6,
                is_closed: true,
                source: "test".to_string(),
            },
        });
        let mark = close * (1.0 + (idx as f64 - 45.0) / 100_000.0);
        state.apply_mark_price(MarkPriceUpdate {
            symbol: "BTCUSDT".to_string(),
            mark_price: mark,
            index_price: close,
            funding_rate: ((idx as f64 - 45.0) / 45.0) * 0.0002,
            next_funding_time_ms: 1_714_550_400_000,
            event_time_ms: 1_714_521_600_000 + idx as i64 * 60_000,
        });
        state.apply_partial_depth(PartialDepthUpdate {
            symbol: "BTCUSDT".to_string(),
            last_update_id: 42 + idx as u64,
            bids: vec![BookLevel {
                price: 100.0,
                qty: if idx % 3 == 0 { 2.0 } else { 10.0 },
            }],
            asks: vec![BookLevel {
                price: 100.1,
                qty: if idx % 3 == 0 { 10.0 } else { 2.0 },
            }],
            event_time_ms: 1_714_521_602_000 + idx as i64 * 60_000,
        });
    }

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert!(packet.scores.csi.is_some());
    assert!(packet.scores.rpi.is_some());
    assert!(packet.scores.vov.is_some());
    assert!(packet.score_meta["CSI"].available);
    assert!(packet.score_meta["RPI"].available);
    assert!(packet.score_meta["VoV"].available);
    assert!(packet.score_meta["CSI"].missing.is_empty());
    assert!(packet.score_meta["RPI"].missing.is_empty());
    assert!(packet.score_meta["VoV"].missing.is_empty());
    assert_eq!(
        packet.legacy_scores.volume_spike_z,
        packet.events.volume_spike_z
    );
}

#[test]
fn standard_packet_includes_u2_full_book_liquidity_and_slippage() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    apply_closes(&mut state, &[100.0, 101.0, 102.0, 103.0, 104.0, 105.0]);
    state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 123,
        bids: vec![
            BookLevel {
                price: 104.99,
                qty: 100.0,
            },
            BookLevel {
                price: 104.95,
                qty: 100.0,
            },
        ],
        asks: vec![
            BookLevel {
                price: 105.01,
                qty: 100.0,
            },
            BookLevel {
                price: 105.05,
                qty: 100.0,
            },
        ],
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.liquidity.book_mode, "full");
    assert!(packet.liquidity.liq_5bp_usd.unwrap() > 0.0);
    assert!(packet.liquidity.liq_10bp_usd.unwrap() >= packet.liquidity.liq_5bp_usd.unwrap());
    assert!(packet.liquidity.slip_10000_buy_bp.unwrap() > 0.0);
    assert!(packet.liquidity.slip_10000_sell_bp.unwrap() > 0.0);
    assert_eq!(packet.quality.book_seq_ok, Some(true));
}

#[test]
fn insufficient_history_reason_is_not_duplicated() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state
        .quality
        .add_reason(QualityReason::InsufficientKlineHistory);
    state.apply_kline(KlineUpdate {
        candle: closed_candle("BTCUSDT", 1_700_000_000_000, 42_000.0),
    });

    let packet = build_standard_packet(&state, 1, 15, 3);
    let reason_count = packet
        .quality
        .reasons
        .iter()
        .filter(|reason| **reason == QualityReason::InsufficientKlineHistory)
        .count();

    assert_eq!(reason_count, 1);
}

#[test]
fn standard_packet_quality_explains_missing_market_inputs() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    apply_closes(&mut state, &[100.0, 101.0, 102.0, 103.0, 104.0, 105.0]);

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert!(packet
        .quality
        .reasons
        .contains(&QualityReason::MissingMarkPrice));
    assert!(packet
        .quality
        .reasons
        .contains(&QualityReason::MissingIndexPrice));
    assert!(packet
        .quality
        .reasons
        .contains(&QualityReason::InsufficientFundingHistory));
}
