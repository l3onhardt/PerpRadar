CREATE TABLE IF NOT EXISTS perp_radar.symbols
(
    symbol String,
    pair String,
    contract_type String,
    status String,
    base_asset String,
    quote_asset String,
    margin_asset String,
    tick_size Float64,
    step_size Float64,
    min_notional Float64,
    updated_at DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, updated_at);
