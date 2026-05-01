#[test]
fn operations_docs_name_required_clickhouse_dependency() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/OPERATIONS.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("ClickHouse is required"));
}

#[test]
fn operations_docs_describe_serving_api_after_startup() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/OPERATIONS.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("serves the API"));
    assert!(docs.contains("confirm the HTTP runtime is serving requests"));
}

#[test]
fn compose_file_wires_app_clickhouse_and_mock_binance() {
    let compose = std::fs::read_to_string(format!(
        "{}/../../docker-compose.yml",
        env!("CARGO_MANIFEST_DIR")
    ))
        .unwrap_or_else(|error| panic!("docker-compose.yml should exist: {error}"));

    assert!(compose.contains("clickhouse"));
    assert!(compose.contains("perp-radar"));
    assert!(compose.contains("mock-binance"));
    assert!(compose.contains("PERP_RADAR__BINANCE__REST_BASE"));
    assert!(compose.contains("PERP_RADAR__STORAGE__CLICKHOUSE_URL"));
}

#[test]
fn data_contract_docs_name_packet_schema() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/DATA_CONTRACT.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("packet_schema"));
    assert!(docs.contains("quality.reasons"));
}

#[test]
fn runbook_docs_describe_cargo_run_serving_api() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/RUNBOOK.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("serves the HTTP API"));
}

#[test]
fn runbook_docs_explain_empty_export_before_ingestion() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/RUNBOOK.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("empty output"));
    assert!(docs.contains("ingested"));
    assert!(docs.contains("cache"));
}
