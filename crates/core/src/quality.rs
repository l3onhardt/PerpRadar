use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityReason {
    InsufficientKlineHistory,
    InsufficientFundingHistory,
    #[serde(rename = "depth_coverage_lt_5bp")]
    DepthCoverageLt5Bp,
    FullBookSequenceGap,
    StaleMarketData,
    MissingMarkPrice,
    MissingIndexPrice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityState {
    pub freshness_ms: u64,
    pub warm: bool,
    pub kline_gap_1m: u32,
    pub book_mode: String,
    pub book_seq_ok: Option<bool>,
    pub book_depth_coverage_bp: Option<f64>,
    pub funding_history_points: usize,
    pub stale: bool,
    pub reasons: Vec<QualityReason>,
}

impl QualityState {
    pub fn cold(book_mode: impl Into<String>) -> Self {
        Self {
            freshness_ms: u64::MAX,
            warm: false,
            kline_gap_1m: 0,
            book_mode: book_mode.into(),
            book_seq_ok: None,
            book_depth_coverage_bp: None,
            funding_history_points: 0,
            stale: true,
            reasons: Vec::new(),
        }
    }

    pub fn add_reason(&mut self, reason: QualityReason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    pub fn clear_reason(&mut self, reason: QualityReason) {
        self.reasons.retain(|existing| *existing != reason);
    }
}
