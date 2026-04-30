#[derive(Debug, Clone, PartialEq)]
pub struct ScoreInputs {
    pub volume_accel_z: Option<f64>,
    pub ret_15m_z_abs: Option<f64>,
    pub atr_pctile: Option<f64>,
    pub funding_z_abs: Option<f64>,
    pub liquidation_event_score: Option<f64>,
    pub squeeze_or_breakout_score: Option<f64>,
    pub liquidity_quality: Option<f64>,
}

pub fn composite_candidate_score(inputs: &ScoreInputs) -> Option<f64> {
    let volume_accel_z = inputs.volume_accel_z?;
    let ret_15m_z_abs = inputs.ret_15m_z_abs?;
    let atr_pctile = inputs.atr_pctile?;
    let funding_z_abs = inputs.funding_z_abs?;
    let liquidation_event_score = inputs.liquidation_event_score?;
    let squeeze_or_breakout_score = inputs.squeeze_or_breakout_score?;
    let liquidity_quality = inputs.liquidity_quality?;

    if [
        volume_accel_z,
        ret_15m_z_abs,
        atr_pctile,
        funding_z_abs,
        liquidation_event_score,
        squeeze_or_breakout_score,
        liquidity_quality,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return None;
    }

    let score = 0.25 * volume_accel_z
        + 0.20 * ret_15m_z_abs
        + 0.15 * atr_pctile
        + 0.15 * funding_z_abs
        + 0.10 * liquidation_event_score
        + 0.10 * squeeze_or_breakout_score
        + 0.05 * liquidity_quality;

    score.is_finite().then_some(score)
}
