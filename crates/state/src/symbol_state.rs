use perp_radar_core::quality::QualityState;
use perp_radar_core::types::Candle;

use crate::candle_ring::CandleRing;

#[derive(Debug, Clone)]
pub struct KlineUpdate {
    pub candle: Candle,
}

#[derive(Debug, Clone)]
pub struct SymbolState {
    pub symbol: String,
    pub candles_1m: CandleRing,
    pub quality: QualityState,
}

impl SymbolState {
    pub fn new(symbol: impl Into<String>, candle_capacity: usize) -> Self {
        Self {
            symbol: symbol.into(),
            candles_1m: CandleRing::new(candle_capacity),
            quality: QualityState::cold("none"),
        }
    }

    pub fn apply_kline(&mut self, update: KlineUpdate) {
        if !update.candle.is_closed {
            return;
        }

        if let Some(last) = self.candles_1m.last() {
            let expected_next = last.open_time_ms + 60_000;
            if update.candle.open_time_ms > expected_next {
                let missed = ((update.candle.open_time_ms - expected_next) / 60_000) as u32;
                self.quality.kline_gap_1m += missed;
            }
        }

        self.candles_1m.push(update.candle);
        self.quality.warm = self.candles_1m.len() >= 2;
        self.quality.stale = false;
    }
}
