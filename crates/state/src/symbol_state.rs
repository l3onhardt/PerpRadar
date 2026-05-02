use chrono::{DateTime, Utc};
use perp_radar_core::quality::{QualityReason, QualityState};
use perp_radar_core::types::Candle;

use crate::book_full::{BookDelta, FullBook};
use crate::book_partial::{BookLevel, PartialBook};
use crate::candle_ring::CandleRing;
use crate::score_history::ScoreHistoryState;

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
    pub event_time_ms: i64,
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
    pub score_history: ScoreHistoryState,
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
            score_history: ScoreHistoryState::new(120),
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
        self.record_chart_score_components();
        self.record_rpi_score_components();
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
        self.record_carry_score_components();
        self.record_rpi_score_components();
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
        if self.full_book.is_none() {
            self.quality.book_mode = "partial20".to_string();
            self.quality.book_seq_ok = None;
        }
        if self.full_book.is_none() || self.quality.book_depth_coverage_bp.is_none() {
            self.quality.book_depth_coverage_bp =
                self.partial_book.as_ref().and_then(book_depth_coverage_bp);
        }
        if self.quality.book_depth_coverage_bp.unwrap_or(0.0) < 5.0 {
            self.quality.add_reason(QualityReason::DepthCoverageLt5Bp);
        } else {
            self.quality.clear_reason(QualityReason::DepthCoverageLt5Bp);
        }
        self.record_rpi_score_components();
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
        self.quality
            .clear_reason(QualityReason::FullBookSequenceGap);
        self.record_lri_score_components();
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
                self.last_event_time_ms = Some(update.event_time_ms);
                self.quality.freshness_ms = 0;
                self.quality.stale = false;
                self.quality
                    .clear_reason(QualityReason::FullBookSequenceGap);
                self.quality.clear_reason(QualityReason::StaleMarketData);
                self.record_lri_score_components();
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
        self.record_carry_score_components();
        self.record_rpi_score_components();
        true
    }

    pub fn needs_full_book_resync(&self) -> bool {
        self.full_book.is_some() && self.quality.book_seq_ok == Some(false)
    }

    pub fn age_quality(&mut self, now_ms: i64, stale_after_ms: u64) {
        self.quality =
            self.quality_with_freshness(self.quality.clone(), now_ms, stale_after_ms);
    }

    pub fn quality_with_freshness(
        &self,
        mut quality: QualityState,
        now_ms: i64,
        stale_after_ms: u64,
    ) -> QualityState {
        let Some(last_event_time_ms) = self.last_event_time_ms else {
            quality.freshness_ms = u64::MAX;
            quality.stale = true;
            quality.add_reason(QualityReason::StaleMarketData);
            return quality;
        };
        let freshness = now_ms.saturating_sub(last_event_time_ms).max(0) as u64;
        quality.freshness_ms = freshness;
        if freshness > stale_after_ms {
            quality.stale = true;
            quality.add_reason(QualityReason::StaleMarketData);
        } else {
            quality.stale = false;
            quality.clear_reason(QualityReason::StaleMarketData);
        }
        quality
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

    fn trusted_full_book(&self) -> bool {
        self.quality.book_mode == "full" && self.quality.book_seq_ok == Some(true)
    }

    fn record_lri_score_components(&mut self) {
        self.score_history.record_lri_book_components(
            self.full_book.as_ref(),
            self.trusted_full_book(),
            10_000.0,
        );
    }

    fn record_chart_score_components(&mut self) {
        let candles = self.candles_1m.items();
        self.score_history.record_chart_components(
            adx14(&candles),
            ema_slope(&candles, 50, 10),
            bollinger_width(&candles, 20),
            atr_pct(&candles, 14),
        );
    }

    fn record_carry_score_components(&mut self) {
        let funding_z_7d = self
            .funding_rate
            .and_then(|rate| funding_z_score(&self.funding_history, rate));
        let basis_bp = basis_bp(self.mark_price, self.index_price);
        self.score_history
            .record_carry_components(funding_z_7d, basis_bp);
    }

    fn record_rpi_score_components(&mut self) {
        let candles = self.candles_1m.items();
        let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
        let funding_z_7d = self
            .funding_rate
            .and_then(|rate| funding_z_score(&self.funding_history, rate));
        let i1 = if self.trusted_full_book() {
            self.full_book
                .as_ref()
                .and_then(|book| book.notional_imbalance_top_n(1))
        } else {
            self.partial_book
                .as_ref()
                .and_then(|book| book.imbalance_top_n(1))
        };
        self.score_history.record_rpi_components(
            simple_rsi(&closes, 14),
            funding_z_7d,
            tail_return(&candles, 60),
            i1,
        );
    }
}

fn basis_bp(mark: Option<f64>, index: Option<f64>) -> Option<f64> {
    let mark = mark?;
    let index = index?;
    if !mark.is_finite() || !index.is_finite() || index == 0.0 {
        return None;
    }
    Some((mark - index) / index * 10_000.0)
}

fn tail_return(candles: &[Candle], minutes: usize) -> Option<f64> {
    let end = candles.last()?.close;
    let start = candles.get(candles.len().checked_sub(minutes + 1)?)?.close;
    if !start.is_finite() || !end.is_finite() || start == 0.0 {
        return None;
    }
    Some((end - start) / start)
}

fn funding_z_score(history: &[f64], current: f64) -> Option<f64> {
    if history.len() < 2 || !current.is_finite() || history.iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let mean = history.iter().sum::<f64>() / history.len() as f64;
    let variance = history
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (history.len() - 1) as f64;
    let stddev = variance.sqrt();
    if stddev == 0.0 {
        return None;
    }
    Some((current - mean) / stddev)
}

fn simple_rsi(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period + 1 || period == 0 || closes.iter().any(|close| !close.is_finite()) {
        return None;
    }
    let window = &closes[closes.len() - period - 1..];
    let mut gains = 0.0;
    let mut losses = 0.0;
    for pair in window.windows(2) {
        let change = pair[1] - pair[0];
        if change >= 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    if gains == 0.0 && losses == 0.0 {
        return Some(50.0);
    }
    if losses == 0.0 {
        return Some(100.0);
    }
    let rs = gains / losses;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

fn ema_slope(candles: &[Candle], period: usize, lookback: usize) -> Option<f64> {
    if lookback == 0 || candles.len() < period + lookback {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    let now = ema_last(&closes, period)?;
    let past_end = closes.len().checked_sub(lookback)?;
    let past = ema_last(&closes[..past_end], period)?;
    if past == 0.0 {
        return None;
    }
    Some((now - past) / past)
}

fn ema_last(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let seed = values[..period].iter().sum::<f64>() / period as f64;
    let multiplier = 2.0 / (period as f64 + 1.0);
    Some(
        values[period..]
            .iter()
            .fold(seed, |ema, value| ((value - ema) * multiplier) + ema),
    )
}

fn bollinger_width(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    let window = &closes[closes.len() - period..];
    let mean = window.iter().sum::<f64>() / period as f64;
    if mean == 0.0 {
        return None;
    }
    let variance = window
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / period as f64;
    Some((4.0 * variance.sqrt()) / mean)
}

fn atr_pct(candles: &[Candle], period: usize) -> Option<f64> {
    let ranges = true_ranges(candles)?;
    if ranges.len() < period {
        return None;
    }
    let atr = ranges[ranges.len() - period..].iter().sum::<f64>() / period as f64;
    let close = candles.last()?.close;
    (close > 0.0).then_some(atr / close)
}

fn true_ranges(candles: &[Candle]) -> Option<Vec<f64>> {
    if candles.len() < 2 {
        return None;
    }
    Some(
        candles
            .windows(2)
            .map(|window| {
                let previous_close = window[0].close;
                let current = &window[1];
                (current.high - current.low)
                    .max((current.high - previous_close).abs())
                    .max((current.low - previous_close).abs())
            })
            .collect(),
    )
}

fn adx14(candles: &[Candle]) -> Option<f64> {
    adx(candles, 14)
}

fn adx(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < (period * 2) + 1 || period == 0 {
        return None;
    }
    let mut plus_dm = Vec::new();
    let mut minus_dm = Vec::new();
    let ranges = true_ranges(candles)?;
    for window in candles.windows(2) {
        let up_move = window[1].high - window[0].high;
        let down_move = window[0].low - window[1].low;
        plus_dm.push(if up_move > down_move && up_move > 0.0 {
            up_move
        } else {
            0.0
        });
        minus_dm.push(if down_move > up_move && down_move > 0.0 {
            down_move
        } else {
            0.0
        });
    }
    let start = ranges.len().checked_sub(period)?;
    let atr_sum = ranges[start..].iter().sum::<f64>();
    if atr_sum == 0.0 {
        return None;
    }
    let plus_di = 100.0 * plus_dm[start..].iter().sum::<f64>() / atr_sum;
    let minus_di = 100.0 * minus_dm[start..].iter().sum::<f64>() / atr_sum;
    let denom = plus_di + minus_di;
    if denom == 0.0 {
        return Some(0.0);
    }
    Some(((plus_di - minus_di).abs() / denom) * 100.0)
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
