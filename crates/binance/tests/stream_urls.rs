use perp_radar_binance::streams::{combined_stream_url, WsBase};

#[test]
fn market_combined_stream_uses_market_base() {
    let url = combined_stream_url(
        WsBase::Market("wss://fstream.binance.com/market".to_string()),
        &["!markPrice@arr", "!ticker@arr"],
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "wss://fstream.binance.com/market/stream?streams=!markPrice@arr/!ticker@arr"
    );
}

#[test]
fn public_combined_stream_uses_public_base() {
    let url = combined_stream_url(
        WsBase::Public("wss://fstream.binance.com/public".to_string()),
        &["btcusdt@depth20@500ms"],
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "wss://fstream.binance.com/public/stream?streams=btcusdt@depth20@500ms"
    );
}
