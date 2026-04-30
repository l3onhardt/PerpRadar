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
fn data_contract_docs_name_packet_schema() {
    let docs = std::fs::read_to_string(format!(
        "{}/../../docs/DATA_CONTRACT.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(docs.contains("packet_schema"));
    assert!(docs.contains("quality.reasons"));
}
