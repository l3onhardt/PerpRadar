CREATE TABLE IF NOT EXISTS perp_radar.mark_funding_sample
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    mark_price Float64,
    index_price Float64,
    basis_bp Float64,
    funding_rate Float64,
    next_funding_time DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
