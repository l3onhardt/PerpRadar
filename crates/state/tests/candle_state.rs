use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::Candle;
use perp_radar_state::book_full::{BookDelta, LevelDelta};
use perp_radar_state::book_partial::BookLevel;
use perp_radar_state::candle_ring::CandleRing;
use perp_radar_state::symbol_state::{
    ForceOrderUpdate, FullDepthDeltaUpdate, FullDepthSnapshotUpdate, KlineUpdate, MarkPriceUpdate,
    PartialDepthUpdate, SymbolState, TickerUpdate,
};

fn candle(open_time_ms: i64, close: f64) -> Candle {
    Candle {
        symbol: "BTCUSDT".to_string(),
        open_time_ms,
        close_time_ms: open_time_ms + 59_999,
        open: close,
        high: close,
        low: close,
        close,
        volume_base: 1.0,
        volume_quote: close,
        trades: 10,
        taker_buy_base: 0.5,
        taker_buy_quote: close * 0.5,
        is_closed: true,
        source: "test".to_string(),
    }
}

fn candle_for(symbol: &str, open_time_ms: i64, close: f64) -> Candle {
    Candle {
        symbol: symbol.to_string(),
        ..candle(open_time_ms, close)
    }
}

#[test]
fn ring_keeps_most_recent_items() {
    let mut ring = CandleRing::new(2);
    ring.push(candle(60_000, 100.0));
    ring.push(candle(120_000, 101.0));
    ring.push(candle(180_000, 102.0));

    assert_eq!(ring.len(), 2);
    assert_eq!(ring.items()[0].open_time_ms, 120_000);
    assert_eq!(ring.items()[1].open_time_ms, 180_000);
}

#[test]
fn symbol_state_only_stores_closed_klines() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    assert!(!state.apply_kline(KlineUpdate {
        candle: Candle {
            is_closed: false,
            ..candle(60_000, 100.0)
        },
    }));
    assert_eq!(state.candles_1m.len(), 0);

    assert!(state.apply_kline(KlineUpdate {
        candle: candle(60_000, 100.0),
    }));
    assert_eq!(state.candles_1m.len(), 1);
}

#[test]
fn symbol_state_counts_1m_gaps() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_kline(KlineUpdate {
        candle: candle(60_000, 100.0),
    });
    state.apply_kline(KlineUpdate {
        candle: candle(180_000, 102.0),
    });

    assert_eq!(state.quality.kline_gap_1m, 1);
}

#[test]
fn symbol_state_ignores_mismatched_symbols() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    let changed = state.apply_kline(KlineUpdate {
        candle: candle_for("ETHUSDT", 60_000, 100.0),
    });

    assert!(!changed);
    assert_eq!(state.candles_1m.len(), 0);
}

#[test]
fn symbol_state_ignores_older_closed_klines() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_kline(KlineUpdate {
        candle: candle(120_000, 101.0),
    });

    let changed = state.apply_kline(KlineUpdate {
        candle: candle(60_000, 100.0),
    });

    assert!(!changed);
    assert_eq!(state.candles_1m.len(), 1);
    assert_eq!(state.candles_1m.items()[0].open_time_ms, 120_000);
}

#[test]
fn symbol_state_replaces_latest_kline_with_same_open_time() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_kline(KlineUpdate {
        candle: candle(60_000, 100.0),
    });

    let changed = state.apply_kline(KlineUpdate {
        candle: candle(60_000, 101.0),
    });

    assert!(changed);
    assert_eq!(state.candles_1m.len(), 1);
    assert_eq!(state.candles_1m.items()[0].close, 101.0);
}

#[test]
fn symbol_state_resets_freshness_when_closed_kline_is_accepted() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    let changed = state.apply_kline(KlineUpdate {
        candle: candle(60_000, 100.0),
    });

    assert!(changed);
    assert!(!state.quality.stale);
    assert_eq!(state.quality.freshness_ms, 0);
}

#[test]
fn symbol_state_tracks_mark_ticker_depth_and_liquidations() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    assert!(state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 64_100.0,
        index_price: 64_080.0,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    }));
    assert!(state.apply_ticker(TickerUpdate {
        symbol: "BTCUSDT".to_string(),
        last_price: 64_120.0,
        quote_volume_24h: 123_000_000.0,
        price_change_percent_24h: 1.25,
        event_time_ms: 1_714_521_601_000,
    }));
    assert!(state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 42,
        bids: vec![
            BookLevel {
                price: 64_100.0,
                qty: 2.0,
            },
            BookLevel {
                price: 64_090.0,
                qty: 3.0,
            },
        ],
        asks: vec![
            BookLevel {
                price: 64_110.0,
                qty: 1.0,
            },
            BookLevel {
                price: 64_120.0,
                qty: 4.0,
            },
        ],
        event_time_ms: 1_714_521_602_000,
    }));
    assert!(state.apply_force_order(ForceOrderUpdate {
        symbol: "BTCUSDT".to_string(),
        side: "SELL".to_string(),
        price: 64_000.0,
        qty: 2.5,
        event_time_ms: 1_714_521_603_000,
        order_time_ms: 1_714_521_602_500,
    }));

    assert_eq!(state.mark_price, Some(64_100.0));
    assert_eq!(state.index_price, Some(64_080.0));
    assert_eq!(state.funding_rate, Some(0.0001));
    assert_eq!(state.last_price, Some(64_120.0));
    assert_eq!(state.quote_volume_24h, Some(123_000_000.0));
    assert!(state.partial_book.is_some());
    assert_eq!(state.liquidations.len(), 1);
    assert_eq!(state.quality.book_mode, "partial20");
}

#[test]
fn symbol_state_marks_market_data_stale_from_event_time() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 100.0,
        index_price: 99.9,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    });

    state.age_quality(1_714_521_607_500, 5_000);

    assert_eq!(state.quality.freshness_ms, 7_500);
    assert!(state.quality.stale);
    assert!(state
        .quality
        .reasons
        .contains(&QualityReason::StaleMarketData));
}

#[test]
fn symbol_state_flags_partial_depth_with_insufficient_coverage() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 1,
        bids: vec![BookLevel {
            price: 99.99,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 100.01,
            qty: 10.0,
        }],
        event_time_ms: 1_714_521_600_000,
    });

    assert_eq!(state.quality.book_mode, "partial20");
    assert!(state.quality.book_depth_coverage_bp.unwrap() < 5.0);
    assert!(state
        .quality
        .reasons
        .contains(&QualityReason::DepthCoverageLt5Bp));
}

#[test]
fn symbol_state_tracks_funding_history_points() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    assert!(state.apply_funding_history("BTCUSDT", vec![0.0001, 0.0002, 0.0003]));

    assert_eq!(state.funding_history, vec![0.0001, 0.0002, 0.0003]);
    assert_eq!(state.quality.funding_history_points, 3);
}

#[test]
fn symbol_state_tracks_full_depth_snapshot_for_u2_liquidity() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    assert!(state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 123,
        bids: vec![BookLevel {
            price: 100.0,
            qty: 10.0,
        }],
        asks: vec![BookLevel {
            price: 100.1,
            qty: 6.0,
        }],
    }));

    assert!(state.full_book.is_some());
    assert_eq!(state.quality.book_mode, "full");
    assert_eq!(state.quality.book_seq_ok, Some(true));
}

#[test]
fn symbol_state_keeps_full_book_quality_when_partial_depth_arrives() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
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
    });

    assert!(state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 11,
        bids: vec![BookLevel {
            price: 99.9,
            qty: 1.0,
        }],
        asks: vec![BookLevel {
            price: 100.2,
            qty: 1.0,
        }],
        event_time_ms: 1_714_521_600_000,
    }));

    assert_eq!(state.quality.book_mode, "full");
    assert_eq!(state.quality.book_seq_ok, Some(true));
}

#[test]
fn symbol_state_applies_full_depth_delta_and_marks_sequence_gap() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
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
    });

    assert!(state.apply_full_depth_delta(FullDepthDeltaUpdate {
        symbol: "BTCUSDT".to_string(),
        delta: BookDelta {
            first_update_id: 8,
            final_update_id: 11,
            previous_final_update_id: 7,
            bids: vec![LevelDelta {
                price: 100.0,
                qty: 12.0,
            }],
            asks: vec![],
        },
    }));
    assert!(!state.apply_full_depth_delta(FullDepthDeltaUpdate {
        symbol: "BTCUSDT".to_string(),
        delta: BookDelta {
            first_update_id: 13,
            final_update_id: 14,
            previous_final_update_id: 999,
            bids: vec![],
            asks: vec![],
        },
    }));
    assert_eq!(state.quality.book_seq_ok, Some(false));
}
