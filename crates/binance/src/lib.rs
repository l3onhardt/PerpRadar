pub mod parser;
pub mod rate_limiter;
pub mod rest_client;
pub mod streams;
pub mod ws_client;

pub fn crate_name() -> &'static str {
    "perp-radar-binance"
}
