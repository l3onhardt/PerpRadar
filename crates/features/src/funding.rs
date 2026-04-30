pub fn z_score(history: &[f64], current: f64) -> Option<f64> {
    if history.len() < 2 || !current.is_finite() || history.iter().any(|value| !value.is_finite()) {
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
        / (history.len() as f64 - 1.0);
    let stddev = variance.sqrt();

    if !stddev.is_finite() || stddev <= 0.0 {
        return None;
    }

    Some((current - mean) / stddev)
}
