use perp_radar_core::types::Candle;
use perp_radar_state::candle_ring::CandleRing;
use perp_radar_state::symbol_state::{KlineUpdate, SymbolState};

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
