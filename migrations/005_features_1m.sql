CREATE TABLE IF NOT EXISTS perp_radar.features_1m
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    price Nullable(Float64),
    ret_1m Nullable(Float64),
    ret_5m Nullable(Float64),
    ret_15m Nullable(Float64),
    atr_pct Nullable(Float64),
    rsi14 Nullable(Float64),
    macd_hist Nullable(Float64),
    adx14 Nullable(Float64),
    bb_width Nullable(Float64),
    funding_z_7d Nullable(Float64),
    basis_bp Nullable(Float64),
    spread_bp Nullable(Float64),
    i1 Nullable(Float64),
    i5 Nullable(Float64),
    tcs Nullable(Float64),
    lri Nullable(Float64),
    dpi5 Nullable(Float64),
    csi Nullable(Float64),
    rpi Nullable(Float64),
    vov Nullable(Float64),
    quality_json String
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
