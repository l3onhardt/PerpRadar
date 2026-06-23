use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::quality::QualityState;
use crate::types::UniverseTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketProfile {
    Compact,
    Standard,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniverseBlock {
    pub tier: UniverseTier,
    pub active_n: usize,
    pub focus_n: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PriceBlock {
    pub last: Option<f64>,
    pub mark: Option<f64>,
    pub index: Option<f64>,
    pub basis_bp: Option<f64>,
    pub ret_1m: Option<f64>,
    pub ret_5m: Option<f64>,
    pub ret_15m: Option<f64>,
    pub ret_1h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChartBlock {
    pub regime: Option<String>,
    pub signature: Option<String>,
    pub ema_20: Option<f64>,
    pub ema_50: Option<f64>,
    pub ema_200: Option<f64>,
    pub ema50_slope: Option<f64>,
    pub rsi_14: Option<f64>,
    pub macd_histogram: Option<f64>,
    pub atr_pct: Option<f64>,
    pub atr_1h_pct: Option<f64>,
    pub atr_1h_pct_prev: Option<f64>,
    pub atr_1h_pct_delta_ratio: Option<f64>,
    pub bb_width: Option<f64>,
    pub bb_width_pctile: Option<f64>,
    pub adx_14: Option<f64>,
    pub vwap_20: Option<f64>,
    pub cmf_20: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LiquidityBlock {
    pub book_mode: String,
    pub spread_bp: Option<f64>,
    pub i1: Option<f64>,
    pub i5: Option<f64>,
    pub microprice_bp: Option<f64>,
    pub liq_5bp_usd: Option<f64>,
    pub liq_10bp_usd: Option<f64>,
    pub slip_10000_buy_bp: Option<f64>,
    pub slip_10000_sell_bp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CarryBlock {
    pub funding_now: Option<f64>,
    pub funding_unit: Option<String>,
    pub funding_interval_hours: Option<u32>,
    pub funding_z_7d: Option<f64>,
    pub next_funding_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EventsBlock {
    pub liq_1m_usd: Option<f64>,
    pub liq_5m_usd: Option<f64>,
    pub liq_15m_usd: Option<f64>,
    pub liq_side: Option<String>,
    pub volume_spike_z: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StructureBlock {
    pub donchian20_hi: Option<f64>,
    pub donchian20_lo: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DerivativesBlock {
    pub oi: Option<f64>,
    pub oi_notional_usd: Option<f64>,
    pub oi_z: Option<f64>,
    pub oi_delta_5m: Option<f64>,
    pub crowded_side: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OrderflowBlock {
    pub ofi: Option<f64>,
    pub ofi_1m: Option<f64>,
    pub ofi_5m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScoresBlock {
    #[serde(rename = "TCS")]
    pub tcs: Option<f64>,
    #[serde(rename = "LRI")]
    pub lri: Option<f64>,
    #[serde(rename = "DPI5")]
    pub dpi5: Option<f64>,
    #[serde(rename = "DPI10")]
    pub dpi10: Option<f64>,
    #[serde(rename = "CSI")]
    pub csi: Option<f64>,
    #[serde(rename = "RPI")]
    pub rpi: Option<f64>,
    #[serde(rename = "VoV")]
    pub vov: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScoreMeta {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slip_notional_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(default)]
    pub components: serde_json::Value,
    #[serde(default)]
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LegacyScoresBlock {
    pub candidate_score: Option<f64>,
    pub liquidation_event_score: Option<f64>,
    pub compression_score: Option<f64>,
    pub momentum_abs_score: Option<f64>,
    pub volume_spike_z: Option<f64>,
    pub notional_imbalance_i5: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardPacket {
    pub packet_schema: String,
    pub ts: DateTime<Utc>,
    pub symbol: String,
    pub rank: usize,
    pub profile: PacketProfile,
    pub universe: UniverseBlock,
    pub price: PriceBlock,
    pub chart: ChartBlock,
    pub liquidity: LiquidityBlock,
    pub carry: CarryBlock,
    pub events: EventsBlock,
    pub structure: StructureBlock,
    pub derivatives: DerivativesBlock,
    pub orderflow: OrderflowBlock,
    pub scores: ScoresBlock,
    pub score_meta: BTreeMap<String, ScoreMeta>,
    pub legacy_scores: LegacyScoresBlock,
    pub quality: QualityState,
}
