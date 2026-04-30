use perp_radar_core::packet::PacketProfile;
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::{Candle, UniverseTier};
use perp_radar_features::packet_builder::build_standard_packet;
use perp_radar_state::symbol_state::{KlineUpdate, SymbolState};

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

#[test]
fn standard_packet_uses_symbol_price_and_quality_from_state() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_kline(KlineUpdate {
        candle: closed_candle("BTCUSDT", 1_700_000_000_000, 42_000.0),
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.symbol, "BTCUSDT");
    assert_eq!(packet.packet_schema, "2.0");
    assert_eq!(packet.profile, PacketProfile::Standard);
    assert_eq!(packet.universe.tier, UniverseTier::U2);
    assert_eq!(packet.universe.active_n, 15);
    assert_eq!(packet.universe.focus_n, 3);
    assert_eq!(packet.price.last, Some(42_000.0));
    assert!(packet.price.ret_5m.is_none());
    assert_eq!(packet.chart.signature, Some("1m:DOJI".to_string()));
    assert_eq!(packet.liquidity.book_mode, "none");
    assert!(!packet.quality.reasons.is_empty());
    assert!(packet
        .quality
        .reasons
        .contains(&QualityReason::InsufficientKlineHistory));
}
