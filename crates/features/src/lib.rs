pub mod funding;
pub mod liquidity;
pub mod packet_builder;
pub mod ranking;
pub mod scores;
pub mod ta;

pub fn crate_name() -> &'static str {
    "perp-radar-features"
}
