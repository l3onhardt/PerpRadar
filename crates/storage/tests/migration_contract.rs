use perp_radar_storage::migrations::{migration_names, migration_sql};

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
