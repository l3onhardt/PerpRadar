CREATE TABLE IF NOT EXISTS perp_radar.features_1m
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    price Float64,
    ret_1m Float64,
    ret_5m Float64,
    ret_15m Float64,
    atr_pct Float64,
    rsi14 Float64,
    macd_hist Float64,
    adx14 Float64,
    bb_width Float64,
    funding_z_7d Float64,
    basis_bp Float64,
    spread_bp Float64,
    i1 Float64,
    i5 Float64,
    tcs Float64,
    lri Float64,
    dpi5 Float64,
    csi Float64,
    rpi Float64,
    vov Float64,
    quality_json String
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
