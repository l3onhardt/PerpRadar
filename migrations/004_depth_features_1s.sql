CREATE TABLE IF NOT EXISTS perp_radar.depth_features_1s
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    mode String,
    spread_bp Nullable(Float64),
    mid Nullable(Float64),
    i1 Nullable(Float64),
    i5 Nullable(Float64),
    i10 Nullable(Float64),
    microprice_bp Nullable(Float64),
    bid_top20_usd Nullable(Float64),
    ask_top20_usd Nullable(Float64),
    liq_5bp_usd Nullable(Float64),
    liq_10bp_usd Nullable(Float64),
    slip_10k_buy_bp Nullable(Float64),
    slip_10k_sell_bp Nullable(Float64),
    coverage_bid_bp Nullable(Float64),
    coverage_ask_bp Nullable(Float64),
    seq_ok Nullable(Bool)
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
