use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UniverseTier {
    #[serde(rename = "U0")]
    U0,
    #[serde(rename = "U1")]
    U1,
    #[serde(rename = "U2")]
    U2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub open_time_ms: i64,
    pub close_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume_base: f64,
    pub volume_quote: f64,
    pub trades: u64,
    pub taker_buy_base: f64,
    pub taker_buy_quote: f64,
    pub is_closed: bool,
    pub source: String,
}
