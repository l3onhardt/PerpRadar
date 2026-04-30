CREATE TABLE IF NOT EXISTS perp_radar.klines_1m
(
    symbol String,
    open_time DateTime64(3, 'UTC'),
    close_time DateTime64(3, 'UTC'),
    open Float64,
    high Float64,
    low Float64,
    close Float64,
    volume_base Float64,
    volume_quote Float64,
    trades UInt64,
    taker_buy_base Float64,
    taker_buy_quote Float64,
    is_closed Bool,
    source String,
    ingest_time DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, open_time);
