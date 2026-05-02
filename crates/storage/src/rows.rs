use anyhow::Result;
use chrono::{DateTime, Timelike, Utc};
use clickhouse::Row;
use perp_radar_core::packet::{PacketProfile, StandardPacket};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Row, Serialize)]
pub struct LatestPacketRow {
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub ts: DateTime<Utc>,
    pub symbol: String,
    pub profile: String,
    pub rank: u32,
    pub packet_json: String,
}

impl LatestPacketRow {
    pub fn from_packet(packet: &StandardPacket) -> Result<Self> {
        Ok(Self {
            ts: packet.ts,
            symbol: packet.symbol.clone(),
            profile: profile_name(packet.profile),
            rank: packet.rank as u32,
            packet_json: serde_json::to_string(packet)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Row, Serialize)]
pub struct Feature1mRow {
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub ts: DateTime<Utc>,
    pub symbol: String,
    pub price: Option<f64>,
    pub ret_1m: Option<f64>,
    pub ret_5m: Option<f64>,
    pub ret_15m: Option<f64>,
    pub atr_pct: Option<f64>,
    pub rsi14: Option<f64>,
    pub macd_hist: Option<f64>,
    pub adx14: Option<f64>,
    pub bb_width: Option<f64>,
    pub funding_z_7d: Option<f64>,
    pub basis_bp: Option<f64>,
    pub spread_bp: Option<f64>,
    pub i1: Option<f64>,
    pub i5: Option<f64>,
    pub tcs: Option<f64>,
    pub lri: Option<f64>,
    pub dpi5: Option<f64>,
    pub csi: Option<f64>,
    pub rpi: Option<f64>,
    pub vov: Option<f64>,
    pub quality_json: String,
}

impl Feature1mRow {
    pub fn from_packet(packet: &StandardPacket) -> Result<Self> {
        Ok(Self {
            ts: truncate_to_minute(packet.ts),
            symbol: packet.symbol.clone(),
            price: packet.price.last,
            ret_1m: packet.price.ret_1m,
            ret_5m: packet.price.ret_5m,
            ret_15m: packet.price.ret_15m,
            atr_pct: packet.chart.atr_pct,
            rsi14: packet.chart.rsi_14,
            macd_hist: packet.chart.macd_histogram,
            adx14: packet.chart.adx_14,
            bb_width: packet.chart.bb_width,
            funding_z_7d: packet.carry.funding_z_7d,
            basis_bp: packet.price.basis_bp,
            spread_bp: packet.liquidity.spread_bp,
            i1: packet.liquidity.i1,
            i5: packet.liquidity.i5,
            tcs: packet.scores.tcs,
            lri: packet.scores.lri,
            dpi5: packet.scores.dpi5,
            csi: packet.scores.csi,
            rpi: packet.scores.rpi,
            vov: packet.scores.vov,
            quality_json: serde_json::to_string(&packet.quality)?,
        })
    }
}

pub fn truncate_to_minute(ts: DateTime<Utc>) -> DateTime<Utc> {
    ts.with_second(0)
        .and_then(|ts| ts.with_nanosecond(0))
        .unwrap_or(ts)
}

fn profile_name(profile: PacketProfile) -> String {
    match profile {
        PacketProfile::Compact => "compact",
        PacketProfile::Standard => "standard",
        PacketProfile::Full => "full",
    }
    .to_string()
}
