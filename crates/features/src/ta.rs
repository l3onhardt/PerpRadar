use perp_radar_core::types::Candle;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TechnicalSnapshot {
    pub regime: Option<String>,
    pub ema_20: Option<f64>,
    pub ema_50: Option<f64>,
    pub rsi_14: Option<f64>,
    pub macd_histogram: Option<f64>,
    pub atr_pct: Option<f64>,
    pub bb_width: Option<f64>,
    pub adx_14: Option<f64>,
    pub vwap_20: Option<f64>,
    pub cmf_20: Option<f64>,
}

pub fn return_pct(start: f64, end: f64) -> Option<f64> {
    if !start.is_finite() || !end.is_finite() || start == 0.0 {
        return None;
    }
    Some((end - start) / start)
}

pub fn simple_rsi(closes: &[f64], period: usize) -> Option<f64> {
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

pub fn volume_z_score(samples: &[f64], current: f64) -> Option<f64> {
    crate::funding::z_score(samples, current)
}

pub fn technical_snapshot(candles: &[Candle]) -> Option<TechnicalSnapshot> {
    if candles.len() < 50 || candles.iter().any(|candle| !valid_candle(candle)) {
        return None;
    }

    let closes = candles
        .iter()
        .map(|candle| candle.close)
        .collect::<Vec<_>>();
    let ema_20 = ema_last(&closes, 20);
    let ema_50 = ema_last(&closes, 50);
    let rsi_14 = simple_rsi(&closes, 14);
    let macd_histogram = macd_histogram(&closes);
    let atr_pct = atr_pct(candles, 14);
    let bb_width = bollinger_width(&closes, 20);
    let adx_14 = adx(candles, 14);
    let vwap_20 = vwap(candles, 20);
    let cmf_20 = cmf(candles, 20);
    let regime = classify_regime(ema_20, ema_50, adx_14, bb_width);

    Some(TechnicalSnapshot {
        regime,
        ema_20,
        ema_50,
        rsi_14,
        macd_histogram,
        atr_pct,
        bb_width,
        adx_14,
        vwap_20,
        cmf_20,
    })
}

fn valid_candle(candle: &Candle) -> bool {
    candle.open.is_finite()
        && candle.high.is_finite()
        && candle.low.is_finite()
        && candle.close.is_finite()
        && candle.volume_base.is_finite()
        && candle.volume_quote.is_finite()
        && candle.high >= candle.low
        && candle.high > 0.0
        && candle.low > 0.0
        && candle.close > 0.0
        && candle.volume_base >= 0.0
        && candle.volume_quote >= 0.0
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

fn macd_histogram(closes: &[f64]) -> Option<f64> {
    if closes.len() < 35 {
        return None;
    }

    let mut macd_values = Vec::new();
    for end in 26..=closes.len() {
        let ema_12 = ema_last(&closes[..end], 12)?;
        let ema_26 = ema_last(&closes[..end], 26)?;
        macd_values.push(ema_12 - ema_26);
    }

    let macd = *macd_values.last()?;
    let signal = ema_last(&macd_values, 9)?;
    Some(macd - signal)
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

fn bollinger_width(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period || period == 0 {
        return None;
    }
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
    let stddev = variance.sqrt();
    Some((4.0 * stddev) / mean)
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

fn vwap(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let window = &candles[candles.len() - period..];
    let total_volume = window.iter().map(|candle| candle.volume_base).sum::<f64>();
    if total_volume == 0.0 {
        return None;
    }
    let total_price_volume = window
        .iter()
        .map(|candle| typical_price(candle) * candle.volume_base)
        .sum::<f64>();
    Some(total_price_volume / total_volume)
}

fn cmf(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let window = &candles[candles.len() - period..];
    let total_volume = window.iter().map(|candle| candle.volume_base).sum::<f64>();
    if total_volume == 0.0 {
        return None;
    }
    let mfv = window
        .iter()
        .map(|candle| {
            let range = candle.high - candle.low;
            if range == 0.0 {
                0.0
            } else {
                (((candle.close - candle.low) - (candle.high - candle.close)) / range)
                    * candle.volume_base
            }
        })
        .sum::<f64>();
    Some(mfv / total_volume)
}

fn typical_price(candle: &Candle) -> f64 {
    (candle.high + candle.low + candle.close) / 3.0
}

fn classify_regime(
    ema_20: Option<f64>,
    ema_50: Option<f64>,
    adx_14: Option<f64>,
    bb_width: Option<f64>,
) -> Option<String> {
    let ema_20 = ema_20?;
    let ema_50 = ema_50?;
    let adx_14 = adx_14?;
    let bb_width = bb_width?;

    let direction = if ema_20 > ema_50 {
        "trend_up"
    } else if ema_20 < ema_50 {
        "trend_down"
    } else {
        "range"
    };

    if adx_14 >= 20.0 {
        Some(direction.to_string())
    } else if bb_width < 0.03 {
        Some("compression".to_string())
    } else {
        Some("range".to_string())
    }
}
