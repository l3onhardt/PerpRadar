pub mod batcher;
pub mod clickhouse;
pub mod migrations;
pub mod rows;
pub mod sink;
pub mod writer;

pub fn crate_name() -> &'static str {
    "perp-radar-storage"
}
