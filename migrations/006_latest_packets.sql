CREATE TABLE IF NOT EXISTS perp_radar.latest_packets
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    profile String,
    rank UInt32,
    packet_json String
)
ENGINE = ReplacingMergeTree(ts)
ORDER BY (symbol, profile);
