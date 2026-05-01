use axum::Json;
use serde::Serialize;

pub fn debug_routes_enabled() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct DebugWsStatus {
    pub enabled: bool,
    pub status: &'static str,
    pub reconnect_policy: &'static str,
    pub session_rollover_hours: u64,
}

#[derive(Debug, Serialize)]
pub struct DebugRateLimits {
    pub enabled: bool,
    pub status: &'static str,
    pub control_messages_per_second: u64,
    pub queue_mode: &'static str,
}

pub async fn ws() -> Json<DebugWsStatus> {
    Json(DebugWsStatus {
        enabled: debug_routes_enabled(),
        status: "runtime_managed",
        reconnect_policy: "bounded_backoff",
        session_rollover_hours: 24,
    })
}

pub async fn rate_limits() -> Json<DebugRateLimits> {
    Json(DebugRateLimits {
        enabled: debug_routes_enabled(),
        status: "configured",
        control_messages_per_second: 10,
        queue_mode: "coalesce_lossy_streams",
    })
}
