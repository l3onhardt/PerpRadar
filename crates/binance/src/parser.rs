use perp_radar_core::types::Candle;
use perp_radar_state::book_full::{BookDelta, LevelDelta};
use perp_radar_state::symbol_state::KlineUpdate;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum BinanceEvent {
    Kline(KlineUpdate),
    Depth(DepthEvent),
    Ignored,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthEvent {
    pub symbol: String,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: u64,
    pub bids: Vec<LevelDelta>,
    pub asks: Vec<LevelDelta>,
}

#[derive(Debug, Deserialize)]
struct CombinedPayload {
    data: serde_json::Value,
}

pub fn parse_combined_event(payload: &str) -> anyhow::Result<BinanceEvent> {
    let combined: CombinedPayload = serde_json::from_str(payload)?;
    match combined.data.get("e").and_then(|value| value.as_str()) {
        Some("kline") => parse_kline(combined.data),
        Some("depthUpdate") => parse_depth(combined.data),
        _ => Ok(BinanceEvent::Ignored),
    }
}

fn parse_kline(data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
    #[derive(Debug, Deserialize)]
    struct KlineEnvelope {
        k: KlinePayload,
    }

    #[derive(Debug, Deserialize)]
    struct KlinePayload {
        #[serde(rename = "t")]
        open_time_ms: i64,
        #[serde(rename = "T")]
        close_time_ms: i64,
        #[serde(rename = "s")]
        symbol: String,
        #[serde(rename = "o")]
        open: String,
        #[serde(rename = "h")]
        high: String,
        #[serde(rename = "l")]
        low: String,
        #[serde(rename = "c")]
        close: String,
        #[serde(rename = "v")]
        volume_base: String,
        #[serde(rename = "q")]
        volume_quote: String,
        #[serde(rename = "n")]
        trades: u64,
        #[serde(rename = "V")]
        taker_buy_base: String,
        #[serde(rename = "Q")]
        taker_buy_quote: String,
        #[serde(rename = "x")]
        is_closed: bool,
    }

    let envelope: KlineEnvelope = serde_json::from_value(data)?;
    let k = envelope.k;
    Ok(BinanceEvent::Kline(KlineUpdate {
        candle: Candle {
            symbol: k.symbol,
            open_time_ms: k.open_time_ms,
            close_time_ms: k.close_time_ms,
            open: k.open.parse()?,
            high: k.high.parse()?,
            low: k.low.parse()?,
            close: k.close.parse()?,
            volume_base: k.volume_base.parse()?,
            volume_quote: k.volume_quote.parse()?,
            trades: k.trades,
            taker_buy_base: k.taker_buy_base.parse()?,
            taker_buy_quote: k.taker_buy_quote.parse()?,
            is_closed: k.is_closed,
            source: "ws".to_string(),
        },
    }))
}

fn parse_depth(data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
    #[derive(Debug, Deserialize)]
    struct DepthPayload {
        #[serde(rename = "s")]
        symbol: String,
        #[serde(rename = "U")]
        first_update_id: u64,
        #[serde(rename = "u")]
        final_update_id: u64,
        #[serde(rename = "pu")]
        previous_final_update_id: u64,
        #[serde(rename = "b")]
        bids: Vec<[String; 2]>,
        #[serde(rename = "a")]
        asks: Vec<[String; 2]>,
    }

    let depth: DepthPayload = serde_json::from_value(data)?;
    Ok(BinanceEvent::Depth(DepthEvent {
        symbol: depth.symbol,
        first_update_id: depth.first_update_id,
        final_update_id: depth.final_update_id,
        previous_final_update_id: depth.previous_final_update_id,
        bids: parse_levels(depth.bids)?,
        asks: parse_levels(depth.asks)?,
    }))
}

fn parse_levels(levels: Vec<[String; 2]>) -> anyhow::Result<Vec<LevelDelta>> {
    levels
        .into_iter()
        .map(|level| {
            Ok(LevelDelta {
                price: level[0].parse()?,
                qty: level[1].parse()?,
            })
        })
        .collect()
}

impl From<DepthEvent> for BookDelta {
    fn from(value: DepthEvent) -> Self {
        BookDelta {
            first_update_id: value.first_update_id,
            final_update_id: value.final_update_id,
            previous_final_update_id: value.previous_final_update_id,
            bids: value.bids,
            asks: value.asks,
        }
    }
}
