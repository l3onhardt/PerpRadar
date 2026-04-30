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
    Some(
        0.25 * inputs.volume_accel_z?
            + 0.20 * inputs.ret_15m_z_abs?
            + 0.15 * inputs.atr_pctile?
            + 0.15 * inputs.funding_z_abs?
            + 0.10 * inputs.liquidation_event_score?
            + 0.10 * inputs.squeeze_or_breakout_score?
            + 0.05 * inputs.liquidity_quality?,
    )
}
