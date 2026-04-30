CREATE TABLE IF NOT EXISTS perp_radar.depth_features_1s
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    mode String,
    spread_bp Float64,
    mid Float64,
    i1 Float64,
    i5 Float64,
    i10 Float64,
    microprice_bp Float64,
    bid_top20_usd Float64,
    ask_top20_usd Float64,
    liq_5bp_usd Float64,
    liq_10bp_usd Float64,
    slip_10k_buy_bp Float64,
    slip_10k_sell_bp Float64,
    coverage_bid_bp Float64,
    coverage_ask_bp Float64,
    seq_ok Bool
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
