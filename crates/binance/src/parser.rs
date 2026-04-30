use anyhow::Context;
use perp_radar_core::types::Candle;
use perp_radar_state::book_full::{BookDelta, LevelDelta};
use perp_radar_state::book_partial::BookLevel;
use perp_radar_state::symbol_state::KlineUpdate;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum BinanceEvent {
    Kline(KlineEvent),
    Depth(DepthEvent),
    PartialDepth(PartialDepthEvent),
    Ignored,
}

#[derive(Debug, Clone)]
pub struct KlineEvent {
    pub stream: String,
    pub update: KlineUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthEvent {
    pub stream: String,
    pub symbol: String,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: u64,
    pub bids: Vec<LevelDelta>,
    pub asks: Vec<LevelDelta>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialDepthEvent {
    pub stream: String,
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

#[derive(Debug, Deserialize)]
struct CombinedPayload {
    stream: String,
    data: serde_json::Value,
}

pub fn parse_combined_event(payload: &str) -> anyhow::Result<BinanceEvent> {
    let combined: CombinedPayload = serde_json::from_str(payload)?;
    match combined.data.get("e").and_then(|value| value.as_str()) {
        Some("kline") => parse_kline(combined.stream, combined.data),
        Some("depthUpdate") => parse_depth(combined.stream, combined.data),
        _ if is_partial_depth_stream(&combined.stream) => {
            parse_partial_depth(combined.stream, combined.data)
        }
        _ if is_empty_symbol_partial_depth_stream(&combined.stream) => {
            parse_partial_depth(combined.stream, combined.data)
        }
        _ => Ok(BinanceEvent::Ignored),
    }
}

fn parse_kline(stream: String, data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
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
    validate_stream_symbol(&stream, &k.symbol)?;
    Ok(BinanceEvent::Kline(KlineEvent {
        stream,
        update: KlineUpdate {
            candle: Candle {
                symbol: k.symbol,
                open_time_ms: k.open_time_ms,
                close_time_ms: k.close_time_ms,
                open: parse_positive_decimal(&k.open, "o")?,
                high: parse_positive_decimal(&k.high, "h")?,
                low: parse_positive_decimal(&k.low, "l")?,
                close: parse_positive_decimal(&k.close, "c")?,
                volume_base: parse_non_negative_decimal(&k.volume_base, "v")?,
                volume_quote: parse_non_negative_decimal(&k.volume_quote, "q")?,
                trades: k.trades,
                taker_buy_base: parse_non_negative_decimal(&k.taker_buy_base, "V")?,
                taker_buy_quote: parse_non_negative_decimal(&k.taker_buy_quote, "Q")?,
                is_closed: k.is_closed,
                source: "ws".to_string(),
            },
        },
    }))
}

fn parse_depth(stream: String, data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
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
    validate_stream_symbol(&stream, &depth.symbol)?;
    Ok(BinanceEvent::Depth(DepthEvent {
        stream,
        symbol: depth.symbol,
        first_update_id: depth.first_update_id,
        final_update_id: depth.final_update_id,
        previous_final_update_id: depth.previous_final_update_id,
        bids: parse_level_deltas("b", depth.bids)?,
        asks: parse_level_deltas("a", depth.asks)?,
    }))
}

fn parse_partial_depth(stream: String, data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
    #[derive(Debug, Deserialize)]
    struct PartialDepthPayload {
        #[serde(rename = "s")]
        symbol: Option<String>,
        #[serde(rename = "lastUpdateId")]
        last_update_id: u64,
        bids: Vec<[String; 2]>,
        asks: Vec<[String; 2]>,
    }

    let depth: PartialDepthPayload = serde_json::from_value(data)?;
    let symbol = match depth.symbol {
        Some(symbol) => {
            validate_stream_symbol(&stream, &symbol)?;
            symbol
        }
        None => stream_symbol(&stream)
            .context("partial depth stream is missing symbol")?
            .to_ascii_uppercase(),
    };
    Ok(BinanceEvent::PartialDepth(PartialDepthEvent {
        stream,
        symbol,
        last_update_id: depth.last_update_id,
        bids: parse_book_levels("bids", depth.bids)?,
        asks: parse_book_levels("asks", depth.asks)?,
    }))
}

fn parse_level_deltas(side: &str, levels: Vec<[String; 2]>) -> anyhow::Result<Vec<LevelDelta>> {
    levels
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            let price_field = format!("{side}[{index}].price");
            let qty_field = format!("{side}[{index}].qty");
            Ok(LevelDelta {
                price: parse_positive_decimal(&level[0], &price_field)?,
                qty: parse_non_negative_decimal(&level[1], &qty_field)?,
            })
        })
        .collect()
}

fn parse_book_levels(side: &str, levels: Vec<[String; 2]>) -> anyhow::Result<Vec<BookLevel>> {
    levels
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            let price_field = format!("{side}[{index}].price");
            let qty_field = format!("{side}[{index}].qty");
            Ok(BookLevel {
                price: parse_positive_decimal(&level[0], &price_field)?,
                qty: parse_non_negative_decimal(&level[1], &qty_field)?,
            })
        })
        .collect()
}

fn parse_positive_decimal(raw: &str, field: &str) -> anyhow::Result<f64> {
    let value = parse_finite_decimal(raw, field)?;
    anyhow::ensure!(value > 0.0, "{field} must be greater than 0.0");
    Ok(value)
}

fn parse_non_negative_decimal(raw: &str, field: &str) -> anyhow::Result<f64> {
    let value = parse_finite_decimal(raw, field)?;
    anyhow::ensure!(value >= 0.0, "{field} must be non-negative");
    Ok(value)
}

fn parse_finite_decimal(raw: &str, field: &str) -> anyhow::Result<f64> {
    let value = raw
        .parse::<f64>()
        .with_context(|| format!("failed to parse {field} as decimal"))?;
    anyhow::ensure!(value.is_finite(), "{field} must be finite");
    Ok(value)
}

fn validate_stream_symbol(stream: &str, symbol: &str) -> anyhow::Result<()> {
    let stream_symbol = stream_symbol(stream).context("stream is missing symbol")?;
    anyhow::ensure!(
        stream_symbol.eq_ignore_ascii_case(symbol),
        "stream symbol {stream_symbol} does not match payload symbol {symbol}"
    );
    Ok(())
}

fn stream_symbol(stream: &str) -> Option<&str> {
    stream
        .split_once('@')
        .and_then(|(symbol, _)| (!symbol.is_empty()).then_some(symbol))
}

fn is_partial_depth_stream(stream: &str) -> bool {
    stream
        .split_once('@')
        .map(|(symbol, name)| !symbol.is_empty() && name == "depth20@500ms")
        .unwrap_or(false)
}

fn is_empty_symbol_partial_depth_stream(stream: &str) -> bool {
    stream == "@depth20@500ms"
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
