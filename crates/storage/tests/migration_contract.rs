use std::future::Future;

use perp_radar_storage::{
    batcher::BatchConfig,
    clickhouse::run_migrations,
    migrations::{migration_names, migration_sql, migration_sql_for_database},
};

#[test]
fn migrations_are_ordered_and_named() {
    assert_eq!(
        migration_names(),
        vec![
            "001_symbols.sql",
            "002_klines_1m.sql",
            "003_mark_funding_sample.sql",
            "004_depth_features_1s.sql",
            "005_features_1m.sql",
            "006_latest_packets.sql",
        ]
    );
}

#[test]
fn latest_packets_migration_contains_packet_json() {
    let sql = migration_sql("006_latest_packets.sql").unwrap();
    assert!(sql.contains("latest_packets"));
    assert!(sql.contains("packet_json String"));
}

#[test]
fn depth_features_allow_missing_derived_values() {
    let sql = migration_sql("004_depth_features_1s.sql").unwrap();
    assert!(sql.contains("spread_bp Nullable(Float64)"));
    assert!(sql.contains("mid Nullable(Float64)"));
    assert!(sql.contains("microprice_bp Nullable(Float64)"));
    assert!(sql.contains("coverage_ask_bp Nullable(Float64)"));
    assert!(sql.contains("seq_ok Nullable(Bool)"));
}

#[test]
fn features_allow_missing_indicator_values() {
    let sql = migration_sql("005_features_1m.sql").unwrap();
    assert!(sql.contains("price Nullable(Float64)"));
    assert!(sql.contains("ret_15m Nullable(Float64)"));
    assert!(sql.contains("funding_z_7d Nullable(Float64)"));
    assert!(sql.contains("spread_bp Nullable(Float64)"));
    assert!(sql.contains("vov Nullable(Float64)"));
}

#[test]
fn run_migrations_returns_migrated_client() {
    fn assert_return_type<T>(_future: T)
    where
        T: Future<Output = anyhow::Result<clickhouse::Client>>,
    {
    }

    assert_return_type(run_migrations("http://localhost:8123", "perp_radar"));
}

#[test]
fn migrations_can_be_rendered_for_configured_database() {
    let sql = migration_sql_for_database("001_symbols.sql", r"custom-db`x").unwrap();

    assert!(sql.contains(r"CREATE TABLE IF NOT EXISTS `custom-db\`x`.symbols"));
    assert!(!sql.contains("perp_radar.symbols"));
}

#[test]
fn batch_config_only_flushes_non_empty_batches_at_threshold() {
    assert!(!BatchConfig::new(0, 1000).should_flush(0));
    assert!(!BatchConfig::new(0, 1000).should_flush(1));
    assert!(!BatchConfig::new(3, 1000).should_flush(2));
    assert!(BatchConfig::new(3, 1000).should_flush(3));
}
