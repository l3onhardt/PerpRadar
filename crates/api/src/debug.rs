use axum::Json;
use serde::Serialize;

pub fn debug_routes_enabled() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct DebugWsStatus {
    pub enabled: bool,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DebugRateLimits {
    pub enabled: bool,
    pub status: &'static str,
}

pub async fn ws() -> Json<DebugWsStatus> {
    Json(DebugWsStatus {
        enabled: debug_routes_enabled(),
        status: "placeholder",
    })
}

pub async fn rate_limits() -> Json<DebugRateLimits> {
    Json(DebugRateLimits {
        enabled: debug_routes_enabled(),
        status: "placeholder",
    })
}
