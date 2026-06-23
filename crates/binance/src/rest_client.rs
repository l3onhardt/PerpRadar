use anyhow::Context;
use perp_radar_core::types::Candle;
use perp_radar_state::book_partial::BookLevel;
use url::Url;

#[derive(Debug, Clone)]
pub struct RestClient {
    base: Url,
    client: reqwest::Client,
}

pub fn parse_klines_json(symbol: &str, value: serde_json::Value) -> anyhow::Result<Vec<Candle>> {
    let rows = value
        .as_array()
        .context("klines response must be an array")?;
    rows.iter()
        .enumerate()
        .map(|(idx, row)| parse_kline_row(symbol, idx, row))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthSnapshot {
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PremiumIndex {
    pub symbol: String,
    pub mark_price: f64,
    pub index_price: f64,
    pub funding_rate: f64,
    pub next_funding_time_ms: i64,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenInterest {
    pub symbol: String,
    pub open_interest: f64,
    pub event_time_ms: i64,
}

pub fn parse_depth_snapshot_json(
    symbol: &str,
    value: serde_json::Value,
) -> anyhow::Result<DepthSnapshot> {
    let last_update_id = value
        .get("lastUpdateId")
        .and_then(serde_json::Value::as_u64)
        .context("depth snapshot missing lastUpdateId")?;
    let bids = parse_depth_levels(&value, "bids")?;
    let asks = parse_depth_levels(&value, "asks")?;

    Ok(DepthSnapshot {
        symbol: symbol.to_ascii_uppercase(),
        last_update_id,
        bids,
        asks,
    })
}

pub fn parse_funding_rates_json(value: serde_json::Value) -> anyhow::Result<Vec<f64>> {
    let rows = value
        .as_array()
        .context("funding rates response must be an array")?;
    rows.iter()
        .enumerate()
        .map(|(idx, row)| {
            let raw = row
                .get("fundingRate")
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("funding rate row {idx} missing fundingRate"))?;
            let parsed = raw
                .parse::<f64>()
                .with_context(|| format!("funding rate row {idx} must parse as decimal"))?;
            anyhow::ensure!(parsed.is_finite(), "funding rate row {idx} must be finite");
            Ok(parsed)
        })
        .collect()
}

pub fn parse_open_interest_json(value: serde_json::Value) -> anyhow::Result<OpenInterest> {
    let symbol = value
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .context("openInterest response missing symbol")?
        .to_ascii_uppercase();
    Ok(OpenInterest {
        symbol,
        open_interest: parse_named_decimal_with_context(&value, "openInterest", "openInterest")?,
        event_time_ms: value
            .get("time")
            .and_then(serde_json::Value::as_i64)
            .context("openInterest response missing time")?,
    })
}

pub fn parse_premium_index_json(value: serde_json::Value) -> anyhow::Result<PremiumIndex> {
    let symbol = value
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .context("premiumIndex response missing symbol")?
        .to_ascii_uppercase();
    Ok(PremiumIndex {
        symbol,
        mark_price: parse_named_decimal(&value, "markPrice")?,
        index_price: parse_named_decimal(&value, "indexPrice")?,
        funding_rate: parse_named_decimal(&value, "lastFundingRate")?,
        next_funding_time_ms: value
            .get("nextFundingTime")
            .and_then(serde_json::Value::as_i64)
            .context("premiumIndex response missing nextFundingTime")?,
        event_time_ms: value
            .get("time")
            .and_then(serde_json::Value::as_i64)
            .context("premiumIndex response missing time")?,
    })
}

fn parse_depth_levels(value: &serde_json::Value, side: &str) -> anyhow::Result<Vec<BookLevel>> {
    let levels = value
        .get(side)
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("depth snapshot missing {side}"))?;

    levels
        .iter()
        .enumerate()
        .map(|(idx, level)| {
            let pair = level
                .as_array()
                .with_context(|| format!("{side}[{idx}] must be array"))?;
            anyhow::ensure!(pair.len() >= 2, "{side}[{idx}] must have price and qty");
            Ok(BookLevel {
                price: parse_json_decimal(&pair[0], idx, &format!("{side}.price"))?,
                qty: parse_json_decimal(&pair[1], idx, &format!("{side}.qty"))?,
            })
        })
        .collect()
}

fn parse_kline_row(symbol: &str, idx: usize, row: &serde_json::Value) -> anyhow::Result<Candle> {
    let values = row
        .as_array()
        .with_context(|| format!("kline row {idx} must be an array"))?;
    anyhow::ensure!(values.len() >= 11, "kline row {idx} has too few fields");

    Ok(Candle {
        symbol: symbol.to_ascii_uppercase(),
        open_time_ms: values[0]
            .as_i64()
            .with_context(|| format!("kline row {idx} open time must be i64"))?,
        open: parse_json_decimal(&values[1], idx, "open")?,
        high: parse_json_decimal(&values[2], idx, "high")?,
        low: parse_json_decimal(&values[3], idx, "low")?,
        close: parse_json_decimal(&values[4], idx, "close")?,
        volume_base: parse_json_decimal(&values[5], idx, "volume")?,
        close_time_ms: values[6]
            .as_i64()
            .with_context(|| format!("kline row {idx} close time must be i64"))?,
        volume_quote: parse_json_decimal(&values[7], idx, "quote_volume")?,
        trades: values[8]
            .as_u64()
            .with_context(|| format!("kline row {idx} trades must be u64"))?,
        taker_buy_base: parse_json_decimal(&values[9], idx, "taker_buy_base")?,
        taker_buy_quote: parse_json_decimal(&values[10], idx, "taker_buy_quote")?,
        is_closed: true,
        source: "rest".to_string(),
    })
}

fn parse_json_decimal(value: &serde_json::Value, idx: usize, field: &str) -> anyhow::Result<f64> {
    let raw = value
        .as_str()
        .with_context(|| format!("kline row {idx} {field} must be string decimal"))?;
    let parsed = raw
        .parse::<f64>()
        .with_context(|| format!("kline row {idx} {field} must parse as decimal"))?;
    anyhow::ensure!(
        parsed.is_finite() && parsed >= 0.0,
        "kline row {idx} {field} must be finite and non-negative"
    );
    Ok(parsed)
}

fn parse_named_decimal(value: &serde_json::Value, field: &str) -> anyhow::Result<f64> {
    parse_named_decimal_with_context(value, field, "premiumIndex")
}

fn parse_named_decimal_with_context(
    value: &serde_json::Value,
    field: &str,
    context_name: &str,
) -> anyhow::Result<f64> {
    let raw = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("{context_name} response missing {field}"))?;
    let parsed = raw
        .parse::<f64>()
        .with_context(|| format!("{context_name} {field} must parse as decimal"))?;
    anyhow::ensure!(parsed.is_finite(), "{context_name} {field} must be finite");
    Ok(parsed)
}

impl RestClient {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        Ok(Self {
            base: Url::parse(base)
                .with_context(|| format!("invalid Binance REST base URL: {base}"))?,
            client: reqwest::Client::new(),
        })
    }

    pub fn exchange_info_url(&self) -> Url {
        self.base
            .join("/fapi/v1/exchangeInfo")
            .expect("valid exchange info URL")
    }

    pub fn klines_url(&self, symbol: &str, interval: &str, limit: u16) -> Url {
        let mut url = self.base.join("/fapi/v1/klines").expect("valid klines URL");
        url.query_pairs_mut()
            .append_pair("symbol", &symbol.to_ascii_uppercase())
            .append_pair("interval", interval)
            .append_pair("limit", &limit.to_string());
        url
    }

    pub fn depth_url(&self, symbol: &str, limit: u16) -> Url {
        let mut url = self.base.join("/fapi/v1/depth").expect("valid depth URL");
        url.query_pairs_mut()
            .append_pair("symbol", &symbol.to_ascii_uppercase())
            .append_pair("limit", &limit.to_string());
        url
    }

    pub fn funding_rate_url(&self, symbol: &str, limit: u16) -> Url {
        let mut url = self
            .base
            .join("/fapi/v1/fundingRate")
            .expect("valid fundingRate URL");
        url.query_pairs_mut()
            .append_pair("symbol", &symbol.to_ascii_uppercase())
            .append_pair("limit", &limit.to_string());
        url
    }

    pub fn premium_index_url(&self, symbol: &str) -> Url {
        let mut url = self
            .base
            .join("/fapi/v1/premiumIndex")
            .expect("valid premiumIndex URL");
        url.query_pairs_mut()
            .append_pair("symbol", &symbol.to_ascii_uppercase());
        url
    }

    pub fn open_interest_url(&self, symbol: &str) -> Url {
        let mut url = self
            .base
            .join("/fapi/v1/openInterest")
            .expect("valid openInterest URL");
        url.query_pairs_mut()
            .append_pair("symbol", &symbol.to_ascii_uppercase());
        url
    }

    pub async fn exchange_info_json(&self) -> anyhow::Result<serde_json::Value> {
        let url = self.exchange_info_url();
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send exchangeInfo request to {url}"))?
            .error_for_status()
            .with_context(|| format!("exchangeInfo request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode exchangeInfo JSON from {url}"))
    }

    pub async fn klines_json(
        &self,
        symbol: &str,
        interval: &str,
        limit: u16,
    ) -> anyhow::Result<serde_json::Value> {
        let url = self.klines_url(symbol, interval, limit);
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send klines request to {url}"))?
            .error_for_status()
            .with_context(|| format!("klines request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode klines JSON from {url}"))
    }

    pub async fn depth_json(&self, symbol: &str, limit: u16) -> anyhow::Result<serde_json::Value> {
        let url = self.depth_url(symbol, limit);
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send depth request to {url}"))?
            .error_for_status()
            .with_context(|| format!("depth request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode depth JSON from {url}"))
    }

    pub async fn funding_rates_json(
        &self,
        symbol: &str,
        limit: u16,
    ) -> anyhow::Result<serde_json::Value> {
        let url = self.funding_rate_url(symbol, limit);
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send fundingRate request to {url}"))?
            .error_for_status()
            .with_context(|| format!("fundingRate request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode fundingRate JSON from {url}"))
    }

    pub async fn premium_index_json(&self, symbol: &str) -> anyhow::Result<serde_json::Value> {
        let url = self.premium_index_url(symbol);
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send premiumIndex request to {url}"))?
            .error_for_status()
            .with_context(|| format!("premiumIndex request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode premiumIndex JSON from {url}"))
    }

    pub async fn open_interest_json(&self, symbol: &str) -> anyhow::Result<serde_json::Value> {
        let url = self.open_interest_url(symbol);
        self.client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send openInterest request to {url}"))?
            .error_for_status()
            .with_context(|| format!("openInterest request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode openInterest JSON from {url}"))
    }
}
