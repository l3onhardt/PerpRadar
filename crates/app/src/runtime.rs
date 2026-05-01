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
use perp_radar_features::packet_builder::build_standard_packet_with_funding_interval;
use perp_radar_features::ranking::{rank_u0_universe, UniverseRankingInput};
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
    pinned_symbols: Vec<String>,
    active_symbols: Vec<String>,
    focus_symbols: Vec<String>,
    stale_after_ms: u64,
    funding_interval_hours: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEngineConfig {
    pub candle_capacity: usize,
    pub active_n: usize,
    pub focus_n: usize,
    pub stale_after_ms: u64,
    pub funding_interval_hours: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDebugSnapshot {
    pub active_symbols: Vec<String>,
    pub focus_symbols: Vec<String>,
    pub stale_symbols: Vec<String>,
    pub packet_count: usize,
    pub full_book_resync_needed: usize,
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
        Self::with_config(
            symbols,
            cache,
            RuntimeEngineConfig {
                candle_capacity,
                active_n,
                focus_n,
                stale_after_ms: 15_000,
                funding_interval_hours: 8,
            },
        )
    }

    pub fn with_config(
        symbols: Vec<String>,
        cache: PacketCache,
        config: RuntimeEngineConfig,
    ) -> Self {
        let active_n = config.active_n;
        let focus_n = config.focus_n;
        let pinned_symbols = canonical_symbols(symbols.clone(), active_n);
        let active_symbols = canonical_symbols(symbols.clone(), active_n);
        let focus_symbols = canonical_symbols(symbols.clone(), focus_n);
        let states = symbols
            .into_iter()
            .map(|symbol| {
                let canonical = symbol.to_ascii_uppercase();
                (
                    canonical.clone(),
                    SymbolState::new(canonical, config.candle_capacity),
                )
            })
            .collect();

        Self {
            states,
            cache,
            active_n,
            focus_n,
            pinned_symbols,
            active_symbols,
            focus_symbols,
            stale_after_ms: config.stale_after_ms,
            funding_interval_hours: config.funding_interval_hours,
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
                    self.ensure_state(&symbol);
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
                    self.ensure_state(&symbol);
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
                self.ensure_state(&symbol);
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
        self.ensure_state(&symbol);
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
        self.ensure_state(&symbol);
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
        self.ensure_state(&symbol);
        let Some(state) = self.states.get_mut(&symbol) else {
            return false;
        };
        let accepted = state.apply_funding_history(&symbol, rates);
        if accepted {
            self.refresh_symbol(&symbol);
        }
        accepted
    }

    pub fn recompute_universe(&mut self) {
        let ranked = rank_u0_universe(
            self.states
                .values()
                .map(|state| UniverseRankingInput {
                    symbol: state.symbol.clone(),
                    quote_volume_24h: state.quote_volume_24h,
                    price_change_percent_24h: state.price_change_percent_24h,
                    funding_rate: state.funding_rate,
                    liquidation_5m_usd: liquidation_total_for_universe(state, 300_000),
                    ret_15m: state.ret_15m(),
                })
                .collect(),
            self.active_n,
        );
        let mut active_symbols = Vec::new();
        for symbol in &self.pinned_symbols {
            if active_symbols.len() >= self.active_n {
                break;
            }
            if !active_symbols.contains(symbol) {
                active_symbols.push(symbol.clone());
            }
        }
        for candidate in &ranked {
            if active_symbols.len() >= self.active_n {
                break;
            }
            if !active_symbols.contains(&candidate.symbol) {
                active_symbols.push(candidate.symbol.clone());
            }
        }
        self.active_symbols = active_symbols;
        self.focus_symbols = ranked
            .into_iter()
            .take(self.focus_n)
            .map(|candidate| candidate.symbol)
            .collect();
        let symbols = self.active_symbols.clone();
        for (idx, symbol) in symbols.iter().enumerate() {
            self.refresh_symbol_with_rank(symbol, idx + 1);
        }
    }

    pub fn age_all(&mut self, now_ms: i64) {
        let symbols = self.states.keys().cloned().collect::<Vec<_>>();
        for symbol in symbols {
            if let Some(state) = self.states.get_mut(&symbol) {
                state.age_quality(now_ms, self.stale_after_ms);
            }
            self.refresh_symbol(&symbol);
        }
    }

    pub fn debug_snapshot(&self) -> RuntimeDebugSnapshot {
        RuntimeDebugSnapshot {
            active_symbols: self.active_symbols.clone(),
            focus_symbols: self.focus_symbols.clone(),
            stale_symbols: self
                .states
                .values()
                .filter(|state| state.quality.stale)
                .map(|state| state.symbol.clone())
                .collect(),
            packet_count: self.cache.len(),
            full_book_resync_needed: self
                .states
                .values()
                .filter(|state| state.quality.book_seq_ok == Some(false))
                .count(),
        }
    }

    fn ensure_state(&mut self, symbol: &str) {
        let symbol = symbol.to_ascii_uppercase();
        self.states
            .entry(symbol.clone())
            .or_insert_with(|| SymbolState::new(symbol, 1500));
    }

    fn refresh_symbol(&self, symbol: &str) {
        let rank = self
            .active_symbols
            .iter()
            .position(|active| active == symbol)
            .map(|idx| idx + 1)
            .unwrap_or(1);
        self.refresh_symbol_with_rank(symbol, rank);
    }

    fn refresh_symbol_with_rank(&self, symbol: &str, rank: usize) {
        if let Some(state) = self.states.get(symbol) {
            self.cache.upsert(build_standard_packet_with_funding_interval(
                state,
                rank,
                self.active_n,
                self.focus_n,
                self.funding_interval_hours,
            ));
        }
    }
}

fn canonical_symbols(symbols: Vec<String>, limit: usize) -> Vec<String> {
    symbols
        .into_iter()
        .map(|symbol| symbol.to_ascii_uppercase())
        .take(limit)
        .collect()
}

fn liquidation_total_for_universe(state: &SymbolState, window_ms: i64) -> Option<f64> {
    let latest = state
        .liquidations
        .iter()
        .map(|event| event.event_time_ms)
        .max()?;
    Some(
        state
            .liquidations
            .iter()
            .filter(|event| latest - event.event_time_ms <= window_ms)
            .map(|event| event.notional_usd)
            .sum(),
    )
}

pub fn start_ingestion_tasks(
    config: &AppConfig,
    cache: PacketCache,
    urls: Vec<Url>,
) -> Vec<JoinHandle<()>> {
    let (tx, mut rx) = mpsc::channel::<String>(4096);
    let symbols = config.universe.always_focus.clone();
    let mut engine = RuntimeEngine::with_config(
        symbols,
        cache,
        RuntimeEngineConfig {
            candle_capacity: 1500,
            active_n: config.universe.active_n,
            focus_n: config.universe.focus_n,
            stale_after_ms: config.packets.standard_interval_ms.saturating_mul(15),
            funding_interval_hours: 8,
        },
    );

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
            engine.recompute_universe();
        }
    });

    let health_handle = tokio::spawn(async move {
        loop {
            tracing::debug!("runtime health tick");
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
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
    handles.push(health_handle);
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
