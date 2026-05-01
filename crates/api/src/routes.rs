use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cache::PacketCache;
use crate::debug;
use crate::export::packet_to_text;

pub const DEFAULT_LIMIT: usize = 20;
pub const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

impl LimitQuery {
    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

pub fn router(cache: PacketCache) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema", get(schema))
        .route("/v1/universe", get(universe))
        .route("/v1/symbols", get(symbols))
        .route("/v1/packet/:symbol", get(packet))
        .route("/v1/packets/top", get(top_packets))
        .route("/v1/export/top.txt", get(export_top_txt))
        .route("/v1/export/top.jsonl", get(export_top_jsonl))
        .route("/v1/debug/ws", get(debug::ws))
        .route("/v1/debug/rate_limits", get(debug::rate_limits))
        .with_state(cache)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn schema() -> Json<serde_json::Value> {
    Json(json!({
        "packet_schema": "2.0",
        "routes": [
            "/v1/health",
            "/v1/schema",
            "/v1/universe",
            "/v1/symbols",
            "/v1/packet/:symbol",
            "/v1/packets/top",
            "/v1/export/top.txt",
            "/v1/export/top.jsonl",
            "/v1/debug/ws",
            "/v1/debug/rate_limits"
        ]
    }))
}

async fn universe(State(cache): State<PacketCache>) -> Json<serde_json::Value> {
    let packets = cache.top(usize::MAX);
    let active_symbols = packets
        .iter()
        .map(|packet| &packet.symbol)
        .collect::<Vec<_>>();
    let focus_symbols = packets
        .iter()
        .filter(|packet| packet.quality.book_mode == "full")
        .map(|packet| &packet.symbol)
        .collect::<Vec<_>>();
    Json(json!({
        "active_n": packets.len(),
        "focus_n": focus_symbols.len(),
        "symbols": active_symbols,
        "active_symbols": active_symbols,
        "focus_symbols": focus_symbols
    }))
}

async fn symbols(State(cache): State<PacketCache>) -> Json<Vec<String>> {
    Json(
        cache
            .top(usize::MAX)
            .into_iter()
            .map(|packet| packet.symbol)
            .collect(),
    )
}

async fn packet(
    State(cache): State<PacketCache>,
    Path(symbol): Path<String>,
) -> Result<Json<perp_radar_core::packet::StandardPacket>, StatusCode> {
    cache.get(&symbol).map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn top_packets(
    State(cache): State<PacketCache>,
    Query(query): Query<LimitQuery>,
) -> Json<Vec<perp_radar_core::packet::StandardPacket>> {
    Json(cache.top(query.limit()))
}

async fn export_top_txt(
    State(cache): State<PacketCache>,
    Query(query): Query<LimitQuery>,
) -> Response {
    let body = cache
        .top(query.limit())
        .iter()
        .map(packet_to_text)
        .collect::<Vec<_>>()
        .join("\n");

    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

async fn export_top_jsonl(
    State(cache): State<PacketCache>,
    Query(query): Query<LimitQuery>,
) -> Result<Response, StatusCode> {
    let lines = cache
        .top(query.limit())
        .into_iter()
        .map(|packet| serde_json::to_string(&packet))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let body = lines.join("\n");

    Ok((
        [(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")],
        body,
    )
        .into_response())
}
