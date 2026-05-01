use chrono::{DateTime, Utc};
use perp_radar_core::quality::{QualityReason, QualityState};
use perp_radar_core::types::Candle;

use crate::book_full::{BookDelta, FullBook};
use crate::book_partial::{BookLevel, PartialBook};
use crate::candle_ring::CandleRing;

#[derive(Debug, Clone)]
pub struct KlineUpdate {
    pub candle: Candle,
}

#[derive(Debug, Clone)]
pub struct MarkPriceUpdate {
    pub symbol: String,
    pub mark_price: f64,
    pub index_price: f64,
    pub funding_rate: f64,
    pub next_funding_time_ms: i64,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TickerUpdate {
    pub symbol: String,
    pub last_price: f64,
    pub quote_volume_24h: f64,
    pub price_change_percent_24h: f64,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PartialDepthUpdate {
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone)]
pub struct FullDepthSnapshotUpdate {
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

#[derive(Debug, Clone)]
pub struct FullDepthDeltaUpdate {
    pub symbol: String,
    pub delta: BookDelta,
}

#[derive(Debug, Clone)]
pub struct ForceOrderUpdate {
    pub symbol: String,
    pub side: String,
    pub price: f64,
    pub qty: f64,
    pub event_time_ms: i64,
    pub order_time_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LiquidationEvent {
    pub side: String,
    pub notional_usd: f64,
    pub event_time_ms: i64,
    pub order_time_ms: i64,
}

#[derive(Debug, Clone)]
pub struct SymbolState {
    pub symbol: String,
    pub candles_1m: CandleRing,
    pub mark_price: Option<f64>,
    pub index_price: Option<f64>,
    pub funding_rate: Option<f64>,
    pub funding_history: Vec<f64>,
    pub next_funding_time: Option<DateTime<Utc>>,
    pub last_price: Option<f64>,
    pub quote_volume_24h: Option<f64>,
    pub price_change_percent_24h: Option<f64>,
    pub partial_book: Option<PartialBook>,
    pub full_book: Option<FullBook>,
    pub liquidations: Vec<LiquidationEvent>,
    pub quality: QualityState,
    last_event_time_ms: Option<i64>,
}

impl SymbolState {
    pub fn new(symbol: impl Into<String>, candle_capacity: usize) -> Self {
        Self {
            symbol: symbol.into(),
            candles_1m: CandleRing::new(candle_capacity),
            mark_price: None,
            index_price: None,
            funding_rate: None,
            funding_history: Vec::new(),
            next_funding_time: None,
            last_price: None,
            quote_volume_24h: None,
            price_change_percent_24h: None,
            partial_book: None,
            full_book: None,
            liquidations: Vec::new(),
            quality: QualityState::cold("none"),
            last_event_time_ms: None,
        }
    }

    pub fn apply_kline(&mut self, update: KlineUpdate) -> bool {
        if !update.candle.is_closed {
            return false;
        }

        if update.candle.symbol != self.symbol {
            return false;
        }

        if let Some(last) = self.candles_1m.last() {
            if update.candle.open_time_ms < last.open_time_ms {
                return false;
            }

            if update.candle.open_time_ms == last.open_time_ms {
                self.candles_1m.replace_last(update.candle);
                self.mark_kline_accepted();
                return true;
            }

            let expected_next = last.open_time_ms + 60_000;
            if update.candle.open_time_ms > expected_next {
                let missed = ((update.candle.open_time_ms - expected_next) / 60_000) as u32;
                self.quality.kline_gap_1m += missed;
            }
        }

        self.candles_1m.push(update.candle);
        self.mark_kline_accepted();
        true
    }

    fn mark_kline_accepted(&mut self) {
        self.last_event_time_ms = self.candles_1m.last().map(|candle| candle.close_time_ms);
        self.quality.warm = self.candles_1m.len() >= 2;
        self.quality.stale = false;
        self.quality.freshness_ms = 0;
        self.quality.clear_reason(QualityReason::StaleMarketData);
    }

    pub fn apply_mark_price(&mut self, update: MarkPriceUpdate) -> bool {
        if update.symbol != self.symbol
            || !update.mark_price.is_finite()
            || !update.index_price.is_finite()
            || !update.funding_rate.is_finite()
        {
            return false;
        }

        self.mark_price = Some(update.mark_price);
        self.index_price = Some(update.index_price);
        self.funding_rate = Some(update.funding_rate);
        self.next_funding_time = DateTime::from_timestamp_millis(update.next_funding_time_ms);
        self.last_event_time_ms = Some(update.event_time_ms);
        self.quality.freshness_ms = 0;
        self.quality.stale = false;
        self.quality.clear_reason(QualityReason::StaleMarketData);
        true
    }

    pub fn apply_ticker(&mut self, update: TickerUpdate) -> bool {
        if update.symbol != self.symbol
            || !update.last_price.is_finite()
            || !update.quote_volume_24h.is_finite()
            || !update.price_change_percent_24h.is_finite()
        {
            return false;
        }

        self.last_price = Some(update.last_price);
        self.quote_volume_24h = Some(update.quote_volume_24h);
        self.price_change_percent_24h = Some(update.price_change_percent_24h);
        self.last_event_time_ms = Some(update.event_time_ms);
        self.quality.freshness_ms = 0;
        self.quality.stale = false;
        self.quality.clear_reason(QualityReason::StaleMarketData);
        true
    }

    pub fn apply_partial_depth(&mut self, update: PartialDepthUpdate) -> bool {
        if update.symbol != self.symbol {
            return false;
        }

        self.partial_book = Some(PartialBook::new(update.symbol, update.bids, update.asks));
        self.quality.book_mode = "partial20".to_string();
        self.quality.book_seq_ok = None;
        self.quality.book_depth_coverage_bp =
            self.partial_book.as_ref().and_then(book_depth_coverage_bp);
        if self.quality.book_depth_coverage_bp.unwrap_or(0.0) < 5.0 {
            self.quality.add_reason(QualityReason::DepthCoverageLt5Bp);
        } else {
            self.quality.clear_reason(QualityReason::DepthCoverageLt5Bp);
        }
        self.last_event_time_ms = Some(update.event_time_ms);
        self.quality.freshness_ms = 0;
        self.quality.stale = false;
        self.quality.clear_reason(QualityReason::StaleMarketData);
        true
    }

    pub fn apply_force_order(&mut self, update: ForceOrderUpdate) -> bool {
        if update.symbol != self.symbol || !update.price.is_finite() || !update.qty.is_finite() {
            return false;
        }

        self.liquidations.push(LiquidationEvent {
            side: update.side,
            notional_usd: update.price * update.qty,
            event_time_ms: update.event_time_ms,
            order_time_ms: update.order_time_ms,
        });
        if self.liquidations.len() > 512 {
            let overflow = self.liquidations.len() - 512;
            self.liquidations.drain(0..overflow);
        }
        self.last_event_time_ms = Some(update.event_time_ms);
        self.quality.freshness_ms = 0;
        self.quality.stale = false;
        self.quality.clear_reason(QualityReason::StaleMarketData);
        true
    }

    pub fn apply_full_depth_snapshot(&mut self, update: FullDepthSnapshotUpdate) -> bool {
        if update.symbol != self.symbol {
            return false;
        }

        if let Some(book) = self.full_book.as_mut() {
            book.reset_from_snapshot(update.last_update_id, update.bids, update.asks);
        } else {
            self.full_book = Some(FullBook::from_snapshot(
                update.symbol,
                update.last_update_id,
                update.bids,
                update.asks,
            ));
        }
        self.quality.book_mode = "full".to_string();
        self.quality.book_seq_ok = Some(true);
        self.quality.book_depth_coverage_bp = self
            .full_book
            .as_ref()
            .and_then(|book| book.visible_liquidity_usd(10.0).map(|_| 10.0));
        self.quality.clear_reason(QualityReason::FullBookSequenceGap);
        self.quality.freshness_ms = 0;
        self.quality.stale = false;
        self.quality.clear_reason(QualityReason::StaleMarketData);
        true
    }

    pub fn apply_full_depth_delta(&mut self, update: FullDepthDeltaUpdate) -> bool {
        if update.symbol != self.symbol {
            return false;
        }
        let Some(book) = self.full_book.as_mut() else {
            return false;
        };

        match book.apply_delta(update.delta) {
            Ok(()) => {
                self.quality.book_mode = "full".to_string();
                self.quality.book_seq_ok = Some(true);
                self.quality.freshness_ms = 0;
                self.quality.stale = false;
                self.quality.clear_reason(QualityReason::FullBookSequenceGap);
                self.quality.clear_reason(QualityReason::StaleMarketData);
                true
            }
            Err(_) => {
                self.quality.book_seq_ok = Some(false);
                self.quality.add_reason(QualityReason::FullBookSequenceGap);
                false
            }
        }
    }

    pub fn apply_funding_history(&mut self, symbol: &str, rates: Vec<f64>) -> bool {
        if symbol != self.symbol || rates.iter().any(|rate| !rate.is_finite()) {
            return false;
        }

        self.funding_history = rates;
        self.quality.funding_history_points = self.funding_history.len();
        true
    }

    pub fn age_quality(&mut self, now_ms: i64, stale_after_ms: u64) {
        let Some(last_event_time_ms) = self.last_event_time_ms else {
            self.quality.freshness_ms = u64::MAX;
            self.quality.stale = true;
            self.quality.add_reason(QualityReason::StaleMarketData);
            return;
        };
        let freshness = now_ms.saturating_sub(last_event_time_ms).max(0) as u64;
        self.quality.freshness_ms = freshness;
        if freshness > stale_after_ms {
            self.quality.stale = true;
            self.quality.add_reason(QualityReason::StaleMarketData);
        } else {
            self.quality.stale = false;
            self.quality.clear_reason(QualityReason::StaleMarketData);
        }
    }

    pub fn ret_15m(&self) -> Option<f64> {
        let candles = self.candles_1m.items();
        let end = candles.last()?.close;
        let start = candles.get(candles.len().checked_sub(16)?)?.close;
        if !start.is_finite() || !end.is_finite() || start == 0.0 {
            return None;
        }
        Some((end - start) / start)
    }
}

fn book_depth_coverage_bp(book: &PartialBook) -> Option<f64> {
    let mid = book.mid()?;
    if mid <= 0.0 {
        return None;
    }
    let bid_coverage = book
        .bids
        .last()
        .map(|level| (mid - level.price).max(0.0) / mid * 10_000.0)?;
    let ask_coverage = book
        .asks
        .last()
        .map(|level| (level.price - mid).max(0.0) / mid * 10_000.0)?;
    Some(bid_coverage.min(ask_coverage))
}
