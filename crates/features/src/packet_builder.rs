use chrono::Utc;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::{Candle, UniverseTier};
use perp_radar_state::symbol_state::SymbolState;

use crate::ta::return_pct;

pub fn build_standard_packet(
    state: &SymbolState,
    rank: usize,
    active_n: usize,
    focus_n: usize,
) -> StandardPacket {
    let candles = state.candles_1m.items();
    let last = candles.last();
    let mut quality = state.quality.clone();
    let price = PriceBlock {
        last: last.map(|candle| candle.close),
        ret_1m: tail_return(&candles, 1),
        ret_5m: tail_return(&candles, 5),
        ret_15m: tail_return(&candles, 15),
        ret_1h: tail_return(&candles, 60),
        ..PriceBlock::default()
    };

    if price.ret_5m.is_none() {
        quality.add_reason(QualityReason::InsufficientKlineHistory);
    }

    StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc::now(),
        symbol: state.symbol.clone(),
        rank,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n,
            focus_n,
        },
        price,
        chart: ChartBlock {
            signature: chart_signature(&candles),
            ..ChartBlock::default()
        },
        liquidity: LiquidityBlock {
            book_mode: state.quality.book_mode.clone(),
            ..LiquidityBlock::default()
        },
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock::default(),
        quality,
    }
}

fn tail_return(candles: &[Candle], minutes: usize) -> Option<f64> {
    let end = candles.last()?.close;
    let start = candles
        .get(candles.len().checked_sub(minutes + 1)?)
        .map(|candle| candle.close)?;
    return_pct(start, end)
}

fn chart_signature(candles: &[Candle]) -> Option<String> {
    if candles.is_empty() {
        return None;
    }

    let colors = candles
        .iter()
        .map(candle_color)
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("1m:{colors}"))
}

fn candle_color(candle: &Candle) -> &'static str {
    if candle.close > candle.open {
        "G"
    } else if candle.close < candle.open {
        "R"
    } else {
        "DOJI"
    }
}
