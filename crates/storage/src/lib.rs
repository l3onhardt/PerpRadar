pub mod batcher;
pub mod clickhouse;
pub mod migrations;

pub fn crate_name() -> &'static str {
    "perp-radar-storage"
}
