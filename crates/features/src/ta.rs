pub fn return_pct(start: f64, end: f64) -> Option<f64> {
    if start == 0.0 {
        return None;
    }
    Some((end - start) / start)
}

pub fn simple_rsi(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period + 1 || period == 0 {
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

    if losses == 0.0 {
        return Some(100.0);
    }

    let rs = gains / losses;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

pub fn volume_z_score(samples: &[f64], current: f64) -> Option<f64> {
    crate::funding::z_score(samples, current)
}
