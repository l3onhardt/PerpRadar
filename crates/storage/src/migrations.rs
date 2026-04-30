const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_symbols.sql",
        include_str!("../../../migrations/001_symbols.sql"),
    ),
    (
        "002_klines_1m.sql",
        include_str!("../../../migrations/002_klines_1m.sql"),
    ),
    (
        "003_mark_funding_sample.sql",
        include_str!("../../../migrations/003_mark_funding_sample.sql"),
    ),
    (
        "004_depth_features_1s.sql",
        include_str!("../../../migrations/004_depth_features_1s.sql"),
    ),
    (
        "005_features_1m.sql",
        include_str!("../../../migrations/005_features_1m.sql"),
    ),
    (
        "006_latest_packets.sql",
        include_str!("../../../migrations/006_latest_packets.sql"),
    ),
];

pub fn migration_names() -> Vec<&'static str> {
    MIGRATIONS.iter().map(|(name, _)| *name).collect()
}

pub fn migration_sql(name: &str) -> Option<&'static str> {
    MIGRATIONS
        .iter()
        .find_map(|(migration_name, sql)| (*migration_name == name).then_some(*sql))
}

pub fn migration_sql_for_database(name: &str, database: &str) -> Option<String> {
    migration_sql(name).map(|sql| render_sql_for_database(sql, database))
}

pub fn all_ordered_sql() -> Vec<(&'static str, &'static str)> {
    MIGRATIONS.to_vec()
}

pub fn all_ordered_sql_for_database(database: &str) -> Vec<(&'static str, String)> {
    MIGRATIONS
        .iter()
        .map(|(name, sql)| (*name, render_sql_for_database(sql, database)))
        .collect()
}

fn render_sql_for_database(sql: &str, database: &str) -> String {
    let escaped_database = clickhouse::Client::default()
        .query("?")
        .bind(clickhouse::sql::Identifier(database))
        .sql_display()
        .to_string();

    sql.replace("perp_radar.", &format!("{escaped_database}."))
}
