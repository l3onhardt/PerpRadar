use std::net::SocketAddr;

use anyhow::Result;
use perp_radar_api::{cache::PacketCache, routes};
use perp_radar_binance::streams::{combined_stream_url, WsBase};
use tokio::net::TcpListener;
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

pub fn build_u2_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth@500ms", symbol.to_ascii_lowercase()))
        .collect()
}

pub fn build_ws_urls(config: &AppConfig) -> Result<Vec<Url>> {
    let global_streams = build_global_market_streams();
    let u2_streams = build_u2_streams(&config.universe.always_focus);
    let u2_stream_refs = u2_streams.iter().map(String::as_str).collect::<Vec<_>>();

    let global_url = combined_stream_url(
        WsBase::Market(config.binance.market_ws_base.clone()),
        &global_streams,
    )?;
    let u2_url = combined_stream_url(
        WsBase::Public(config.binance.public_ws_base.clone()),
        &u2_stream_refs,
    )?;

    Ok(vec![global_url, u2_url])
}

pub async fn serve_api(config: &AppConfig, cache: PacketCache) -> Result<()> {
    let address = config.api.bind.parse::<SocketAddr>()?;
    let listener = TcpListener::bind(address).await?;

    axum::serve(listener, routes::router(cache)).await?;
    Ok(())
}
