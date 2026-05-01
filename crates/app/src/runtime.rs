use std::collections::HashMap;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use perp_radar_api::{cache::PacketCache, routes};
use perp_radar_binance::parser::{parse_combined_event, BinanceEvent};
use perp_radar_binance::rest_client::{
    parse_depth_snapshot_json, parse_funding_rates_json, parse_klines_json, RestClient,
};
use perp_radar_binance::streams::{combined_stream_url, WsBase};
use perp_radar_binance::ws_client::stream_text_messages;
use perp_radar_core::types::Candle;
use perp_radar_features::packet_builder::build_standard_packet;
use perp_radar_state::book_partial::BookLevel;
use perp_radar_state::symbol_state::{
    ForceOrderUpdate, FullDepthDeltaUpdate, FullDepthSnapshotUpdate, KlineUpdate, MarkPriceUpdate,
    PartialDepthUpdate, SymbolState, TickerUpdate,
};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use url::Url;

use crate::config::AppConfig;

pub fn build_global_market_streams() -> Vec<&'static str> {
    vec!["!markPrice@arr", "!ticker@arr", "!forceOrder@arr"]
}

pub fn build_u1_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .flat_map(|symbol| {
            let symbol = symbol.to_ascii_lowercase();
            [
                format!("{symbol}@kline_1m"),
                format!("{symbol}@depth20@500ms"),
            ]
        })
        .collect()
}

pub fn build_u1_kline_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@kline_1m", symbol.to_ascii_lowercase()))
        .collect()
}

pub fn build_u1_depth20_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth20@500ms", symbol.to_ascii_lowercase()))
        .collect()
}

pub fn build_u2_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth@500ms", symbol.to_ascii_lowercase()))
        .collect()
}

pub fn build_ws_urls(config: &AppConfig) -> Result<Vec<Url>> {
    let global_streams = build_global_market_streams();
    let u1_symbols = config.universe.always_focus.clone();
    let u1_kline_streams = build_u1_kline_streams(&u1_symbols);
    let u1_kline_refs = u1_kline_streams
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let u1_depth20_streams = build_u1_depth20_streams(&u1_symbols);
    let u1_depth20_refs = u1_depth20_streams
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let u2_streams = build_u2_streams(&config.universe.always_focus);
    let u2_stream_refs = u2_streams.iter().map(String::as_str).collect::<Vec<_>>();

    let global_url = combined_stream_url(
        WsBase::Market(config.binance.market_ws_base.clone()),
        &global_streams,
    )?;
    let u1_kline_url = combined_stream_url(
        WsBase::Market(config.binance.market_ws_base.clone()),
        &u1_kline_refs,
    )?;
    let u1_depth20_url = combined_stream_url(
        WsBase::Public(config.binance.public_ws_base.clone()),
        &u1_depth20_refs,
    )?;
    let u2_url = combined_stream_url(
        WsBase::Public(config.binance.public_ws_base.clone()),
        &u2_stream_refs,
    )?;

    Ok(vec![global_url, u1_kline_url, u1_depth20_url, u2_url])
}

#[derive(Debug, Clone)]
pub struct RuntimeEngine {
    states: HashMap<String, SymbolState>,
    cache: PacketCache,
    active_n: usize,
    focus_n: usize,
}

#[derive(Debug, Clone)]
pub struct DepthBootstrapSnapshot {
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

impl RuntimeEngine {
    pub fn new(symbols: Vec<String>, cache: PacketCache, candle_capacity: usize) -> Self {
        let active_n = symbols.len();
        let focus_n = symbols.len();
        let states = symbols
            .into_iter()
            .map(|symbol| {
                let canonical = symbol.to_ascii_uppercase();
                (
                    canonical.clone(),
                    SymbolState::new(canonical, candle_capacity),
                )
            })
            .collect();

        Self {
            states,
            cache,
            active_n,
            focus_n,
        }
    }

    pub fn apply_json(&mut self, payload: &str) -> Result<()> {
        let event = parse_combined_event(payload)?;
        self.apply_event(event);
        Ok(())
    }

    pub fn apply_event(&mut self, event: BinanceEvent) {
        match event {
            BinanceEvent::Kline(event) => {
                let symbol = event.update.candle.symbol.clone();
                if let Some(state) = self.states.get_mut(&symbol) {
                    if state.apply_kline(event.update) {
                        self.refresh_symbol(&symbol);
                    }
                }
            }
            BinanceEvent::PartialDepth(event) => {
                let symbol = event.symbol.clone();
                if let Some(state) = self.states.get_mut(&symbol) {
                    if state.apply_partial_depth(PartialDepthUpdate {
                        symbol: event.symbol,
                        last_update_id: event.last_update_id,
                        bids: event.bids,
                        asks: event.asks,
                        event_time_ms: 0,
                    }) {
                        self.refresh_symbol(&symbol);
                    }
                }
            }
            BinanceEvent::MarkPrices(events) => {
                for event in events {
                    let symbol = event.symbol.clone();
                    if let Some(state) = self.states.get_mut(&symbol) {
                        if state.apply_mark_price(MarkPriceUpdate {
                            symbol: event.symbol,
                            mark_price: event.mark_price,
                            index_price: event.index_price,
                            funding_rate: event.funding_rate,
                            next_funding_time_ms: event.next_funding_time_ms,
                            event_time_ms: event.event_time_ms,
                        }) {
                            self.refresh_symbol(&symbol);
                        }
                    }
                }
            }
            BinanceEvent::Tickers(events) => {
                for event in events {
                    let symbol = event.symbol.clone();
                    if let Some(state) = self.states.get_mut(&symbol) {
                        if state.apply_ticker(TickerUpdate {
                            symbol: event.symbol,
                            last_price: event.last_price,
                            quote_volume_24h: event.quote_volume_24h,
                            price_change_percent_24h: event.price_change_percent_24h,
                            event_time_ms: event.event_time_ms,
                        }) {
                            self.refresh_symbol(&symbol);
                        }
                    }
                }
            }
            BinanceEvent::ForceOrder(event) => {
                let symbol = event.symbol.clone();
                if let Some(state) = self.states.get_mut(&symbol) {
                    if state.apply_force_order(ForceOrderUpdate {
                        symbol: event.symbol,
                        side: event.side,
                        price: event.price,
                        qty: event.qty,
                        event_time_ms: event.event_time_ms,
                        order_time_ms: event.order_time_ms,
                    }) {
                        self.refresh_symbol(&symbol);
                    }
                }
            }
            BinanceEvent::Depth(event) => {
                let symbol = event.symbol.clone();
                if let Some(state) = self.states.get_mut(&symbol) {
                    state.apply_full_depth_delta(FullDepthDeltaUpdate {
                        symbol: symbol.clone(),
                        delta: event.into(),
                    });
                    self.refresh_symbol(&symbol);
                }
            }
            BinanceEvent::Ignored => {}
        }
    }

    pub fn bootstrap_klines(&mut self, symbol: &str, candles: Vec<Candle>) -> usize {
        let symbol = symbol.to_ascii_uppercase();
        let Some(state) = self.states.get_mut(&symbol) else {
            return 0;
        };

        let mut accepted = 0;
        for candle in candles {
            if state.apply_kline(KlineUpdate { candle }) {
                accepted += 1;
            }
        }
        if accepted > 0 {
            self.refresh_symbol(&symbol);
        }
        accepted
    }

    pub fn bootstrap_depth_snapshot(&mut self, snapshot: DepthBootstrapSnapshot) -> bool {
        let symbol = snapshot.symbol.to_ascii_uppercase();
        let Some(state) = self.states.get_mut(&symbol) else {
            return false;
        };

        let accepted = state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
            symbol: snapshot.symbol,
            last_update_id: snapshot.last_update_id,
            bids: snapshot.bids,
            asks: snapshot.asks,
        });
        if accepted {
            self.refresh_symbol(&symbol);
        }
        accepted
    }

    pub fn bootstrap_funding_history(&mut self, symbol: &str, rates: Vec<f64>) -> bool {
        let symbol = symbol.to_ascii_uppercase();
        let Some(state) = self.states.get_mut(&symbol) else {
            return false;
        };
        let accepted = state.apply_funding_history(&symbol, rates);
        if accepted {
            self.refresh_symbol(&symbol);
        }
        accepted
    }

    fn refresh_symbol(&self, symbol: &str) {
        if let Some(state) = self.states.get(symbol) {
            self.cache
                .upsert(build_standard_packet(state, 1, self.active_n, self.focus_n));
        }
    }
}

pub fn start_ingestion_tasks(
    config: &AppConfig,
    cache: PacketCache,
    urls: Vec<Url>,
) -> Vec<JoinHandle<()>> {
    let (tx, mut rx) = mpsc::channel::<String>(4096);
    let symbols = config.universe.always_focus.clone();
    let mut engine = RuntimeEngine::new(symbols, cache, 1500);

    let bootstrap_config = config.clone();
    let bootstrap_handle = tokio::spawn(async move {
        match bootstrap_focus_klines(&bootstrap_config, &mut engine, 500).await {
            Ok(accepted) => tracing::info!(accepted, "bootstrapped focus klines"),
            Err(error) => tracing::warn!(%error, "focus kline bootstrap failed"),
        }
        match bootstrap_focus_depths(&bootstrap_config, &mut engine, 1000).await {
            Ok(accepted) => tracing::info!(accepted, "bootstrapped focus depth snapshots"),
            Err(error) => tracing::warn!(%error, "focus depth bootstrap failed"),
        }
        match bootstrap_focus_funding_history(&bootstrap_config, &mut engine, 126).await {
            Ok(accepted) => tracing::info!(accepted, "bootstrapped focus funding histories"),
            Err(error) => tracing::warn!(%error, "focus funding history bootstrap failed"),
        }

        while let Some(payload) = rx.recv().await {
            if let Err(error) = engine.apply_json(&payload) {
                tracing::warn!(%error, "failed to apply Binance event");
            }
        }
    });

    let mut handles = urls
        .into_iter()
        .map(|url| {
            let tx = tx.clone();
            tokio::spawn(async move {
                loop {
                    if let Err(error) = stream_text_messages(url.clone(), tx.clone()).await {
                        tracing::warn!(%url, %error, "Binance websocket stream ended");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            })
        })
        .collect::<Vec<_>>();
    handles.push(bootstrap_handle);
    handles
}

pub async fn bootstrap_focus_klines(
    config: &AppConfig,
    engine: &mut RuntimeEngine,
    limit: u16,
) -> Result<usize> {
    let client = RestClient::new(&config.binance.rest_base)?;
    let mut accepted = 0;
    for symbol in &config.universe.always_focus {
        let json = client
            .klines_json(symbol, "1m", limit)
            .await
            .with_context(|| format!("bootstrap 1m klines for {symbol}"))?;
        let candles = parse_klines_json(symbol, json)
            .with_context(|| format!("parse bootstrap 1m klines for {symbol}"))?;
        accepted += engine.bootstrap_klines(symbol, candles);
    }
    Ok(accepted)
}

pub async fn bootstrap_focus_depths(
    config: &AppConfig,
    engine: &mut RuntimeEngine,
    limit: u16,
) -> Result<usize> {
    let client = RestClient::new(&config.binance.rest_base)?;
    let mut accepted = 0;
    for symbol in &config.universe.always_focus {
        let json = client
            .depth_json(symbol, limit)
            .await
            .with_context(|| format!("bootstrap depth snapshot for {symbol}"))?;
        let snapshot = parse_depth_snapshot_json(symbol, json)
            .with_context(|| format!("parse bootstrap depth snapshot for {symbol}"))?;
        if engine.bootstrap_depth_snapshot(DepthBootstrapSnapshot {
            symbol: snapshot.symbol,
            last_update_id: snapshot.last_update_id,
            bids: snapshot.bids,
            asks: snapshot.asks,
        }) {
            accepted += 1;
        }
    }
    Ok(accepted)
}

pub async fn bootstrap_focus_funding_history(
    config: &AppConfig,
    engine: &mut RuntimeEngine,
    limit: u16,
) -> Result<usize> {
    let client = RestClient::new(&config.binance.rest_base)?;
    let mut accepted = 0;
    for symbol in &config.universe.always_focus {
        let json = client
            .funding_rates_json(symbol, limit)
            .await
            .with_context(|| format!("bootstrap funding history for {symbol}"))?;
        let rates = parse_funding_rates_json(json)
            .with_context(|| format!("parse bootstrap funding history for {symbol}"))?;
        if engine.bootstrap_funding_history(symbol, rates) {
            accepted += 1;
        }
    }
    Ok(accepted)
}

pub async fn serve_api(config: &AppConfig, cache: PacketCache) -> Result<()> {
    let address = config
        .api
        .bind
        .parse::<SocketAddr>()
        .with_context(|| format!("parsing api.bind address {}", config.api.bind))?;
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("binding API listener at {address}"))?;

    serve_api_listener(listener, cache).await
}

pub async fn serve_api_listener(listener: TcpListener, cache: PacketCache) -> Result<()> {
    let address = listener
        .local_addr()
        .context("reading API listener local address")?;

    axum::serve(listener, routes::router(cache))
        .await
        .with_context(|| format!("serving API at {address}"))?;
    Ok(())
}
