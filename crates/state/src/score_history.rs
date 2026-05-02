use std::collections::VecDeque;

use crate::book_full::FullBook;

#[derive(Debug, Clone, PartialEq)]
pub struct RingWindow {
    capacity: usize,
    values: VecDeque<f64>,
}

impl RingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.capacity == 0 || !value.is_finite() {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values_recent(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }

    pub fn percentile_rank(&self, current: f64) -> Option<f64> {
        if !current.is_finite() || self.values.is_empty() {
            return None;
        }
        let count = self
            .values
            .iter()
            .filter(|value| **value <= current)
            .count();
        Some(count as f64 / self.values.len() as f64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreHistoryState {
    pub neg_spread_bp: RingWindow,
    pub liq_5bp_usd: RingWindow,
    pub neg_slip_bp: RingWindow,
    pub lri_raw: RingWindow,
    pub adx14: RingWindow,
    pub ema50_slope: RingWindow,
    pub bb_width_pctile: RingWindow,
    pub abs_fundz_7d: RingWindow,
    pub abs_basis_bp: RingWindow,
    pub rsi_extreme: RingWindow,
    pub funding_same_side: RingWindow,
    pub book_against_move: RingWindow,
    pub atr_delta_ratio: RingWindow,
    pub bb_width: RingWindow,
    pub atr_pct: RingWindow,
    pub latest_neg_spread_bp: Option<f64>,
    pub latest_liq_5bp_usd: Option<f64>,
    pub latest_neg_slip_bp: Option<f64>,
    pub latest_lri_raw: Option<f64>,
    pub latest_bb_width_pctile: Option<f64>,
    pub latest_atr_1h_pct: Option<f64>,
    pub latest_atr_1h_pct_prev: Option<f64>,
    pub latest_atr_delta_ratio: Option<f64>,
}

impl ScoreHistoryState {
    pub fn new(capacity: usize) -> Self {
        Self {
            neg_spread_bp: RingWindow::new(capacity),
            liq_5bp_usd: RingWindow::new(capacity),
            neg_slip_bp: RingWindow::new(capacity),
            lri_raw: RingWindow::new(capacity),
            adx14: RingWindow::new(capacity),
            ema50_slope: RingWindow::new(capacity),
            bb_width_pctile: RingWindow::new(capacity),
            abs_fundz_7d: RingWindow::new(capacity),
            abs_basis_bp: RingWindow::new(capacity),
            rsi_extreme: RingWindow::new(capacity),
            funding_same_side: RingWindow::new(capacity),
            book_against_move: RingWindow::new(capacity),
            atr_delta_ratio: RingWindow::new(capacity),
            bb_width: RingWindow::new(capacity),
            atr_pct: RingWindow::new(capacity),
            latest_neg_spread_bp: None,
            latest_liq_5bp_usd: None,
            latest_neg_slip_bp: None,
            latest_lri_raw: None,
            latest_bb_width_pctile: None,
            latest_atr_1h_pct: None,
            latest_atr_1h_pct_prev: None,
            latest_atr_delta_ratio: None,
        }
    }

    pub fn record_lri_book_components(
        &mut self,
        book: Option<&FullBook>,
        trusted: bool,
        slip_notional_usd: f64,
    ) {
        if !trusted {
            return;
        }
        let Some(book) = book else {
            return;
        };
        let Some(spread_bp) = book.spread_bp() else {
            return;
        };
        let Some(liq_5bp_usd) = book.visible_liquidity_usd(5.0) else {
            return;
        };
        let Some(slip_buy_bp) = book.slippage_bp_for_notional(slip_notional_usd, true) else {
            return;
        };
        let Some(slip_sell_bp) = book.slippage_bp_for_notional(slip_notional_usd, false) else {
            return;
        };
        let slip_bp = slip_buy_bp.max(slip_sell_bp);
        let neg_spread_bp = -spread_bp;
        let neg_slip_bp = -slip_bp;

        self.latest_neg_spread_bp = Some(neg_spread_bp);
        self.latest_liq_5bp_usd = Some(liq_5bp_usd);
        self.latest_neg_slip_bp = Some(neg_slip_bp);
        self.neg_spread_bp.push(neg_spread_bp);
        self.liq_5bp_usd.push(liq_5bp_usd);
        self.neg_slip_bp.push(neg_slip_bp);

        if let (Some(spread_z), Some(liq_z), Some(slip_z)) = (
            robust_z(&self.neg_spread_bp, neg_spread_bp, 30, 5.0),
            robust_z(&self.liq_5bp_usd, liq_5bp_usd, 30, 5.0),
            robust_z(&self.neg_slip_bp, neg_slip_bp, 30, 5.0),
        ) {
            self.record_lri_raw(0.4 * spread_z + 0.3 * liq_z + 0.3 * slip_z);
        }
    }

    pub fn record_lri_raw(&mut self, value: f64) {
        self.latest_lri_raw = value.is_finite().then_some(value);
        self.lri_raw.push(value);
    }

    pub fn record_chart_components(
        &mut self,
        adx14: Option<f64>,
        ema50_slope: Option<f64>,
        bb_width: Option<f64>,
        atr_pct: Option<f64>,
    ) {
        if let Some(value) = adx14 {
            self.adx14.push(value);
        }
        if let Some(value) = ema50_slope {
            self.ema50_slope.push(value);
        }
        if let Some(width) = bb_width {
            self.bb_width.push(width);
            if let Some(percentile) = self.bb_width.percentile_rank(width) {
                self.latest_bb_width_pctile = Some(percentile);
                self.bb_width_pctile.push(percentile);
            }
        }
        if let Some(current_atr_pct) = atr_pct.filter(|value| value.is_finite()) {
            let previous = self.latest_atr_1h_pct;
            self.latest_atr_1h_pct_prev = previous;
            self.latest_atr_1h_pct = Some(current_atr_pct);
            self.atr_pct.push(current_atr_pct);
            if let Some(previous) = previous.filter(|value| *value > 0.0) {
                let ratio = (current_atr_pct - previous) / previous;
                self.latest_atr_delta_ratio = ratio.is_finite().then_some(ratio);
                self.atr_delta_ratio.push(ratio);
            }
        }
    }

    pub fn record_carry_components(
        &mut self,
        funding_z_7d: Option<f64>,
        basis_bp: Option<f64>,
    ) {
        if let Some(value) = funding_z_7d {
            self.abs_fundz_7d.push(value.abs());
        }
        if let Some(value) = basis_bp {
            self.abs_basis_bp.push(value.abs());
        }
    }

    pub fn record_rpi_components(
        &mut self,
        rsi14: Option<f64>,
        funding_z_7d: Option<f64>,
        ret_1h: Option<f64>,
        i1: Option<f64>,
    ) {
        let Some(rsi14) = rsi14.filter(|value| value.is_finite()) else {
            return;
        };
        let rsi_signed = rsi14 - 50.0;
        self.rsi_extreme.push(rsi_signed.abs());

        if let Some(funding_z_7d) = funding_z_7d.filter(|value| value.is_finite()) {
            self.funding_same_side
                .push((rsi_signed.signum() * funding_z_7d).max(0.0));
        }
        if let (Some(ret_1h), Some(i1)) = (
            ret_1h.filter(|value| value.is_finite()),
            i1.filter(|value| value.is_finite()),
        ) {
            self.book_against_move
                .push((-ret_1h.signum() * i1).max(0.0));
        }
    }
}

fn robust_z(window: &RingWindow, current: f64, min_samples: usize, z_clip: f64) -> Option<f64> {
    let values = window.values_recent();
    if !current.is_finite() || min_samples == 0 || values.len() < min_samples {
        return None;
    }
    let median_value = median(values.clone())?;
    let deviations = values
        .iter()
        .map(|value| (value - median_value).abs())
        .collect::<Vec<_>>();
    let mad = median(deviations)?;
    let mut scale = 1.4826 * mad;
    if scale == 0.0 {
        scale = sample_stddev(&values)?;
    }
    if scale == 0.0 && (current - median_value).abs() == 0.0 {
        return Some(0.0);
    }
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    let clip = if z_clip.is_finite() && z_clip > 0.0 {
        z_clip
    } else {
        f64::INFINITY
    };
    Some(((current - median_value) / scale).clamp(-clip, clip))
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn sample_stddev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    Some(variance.sqrt())
}
