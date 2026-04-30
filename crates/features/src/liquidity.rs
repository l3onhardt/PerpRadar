pub fn liquidity_quality(spread_bp: Option<f64>, coverage_bp: Option<f64>) -> Option<f64> {
    let spread = spread_bp?;
    let coverage = coverage_bp?;
    if !spread.is_finite() || !coverage.is_finite() || spread < 0.0 || coverage < 0.0 {
        return None;
    }

    let spread_component = (1.0 - (spread / 20.0)).clamp(0.0, 1.0);
    let coverage_component = (coverage / 10.0).clamp(0.0, 1.0);
    Some((spread_component + coverage_component) / 2.0)
}
