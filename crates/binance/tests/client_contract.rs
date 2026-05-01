use perp_radar_binance::rate_limiter::TokenBucket;
use perp_radar_binance::rest_client::{
    parse_depth_snapshot_json, parse_funding_rates_json, parse_klines_json, RestClient,
};
use perp_radar_binance::ws_client::stream_text_messages;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

#[test]
fn rest_client_builds_exchange_info_url() {
    let client = RestClient::new("https://fapi.binance.com").expect("valid REST base");
    assert_eq!(
        client.exchange_info_url().as_str(),
        "https://fapi.binance.com/fapi/v1/exchangeInfo"
    );
}

#[test]
fn rest_client_rejects_invalid_base_url() {
    assert!(RestClient::new("not a url").is_err());
}

#[test]
fn rest_client_trims_trailing_slash_for_exchange_info_url() {
    let client = RestClient::new("https://fapi.binance.com/").expect("valid REST base");
    assert_eq!(
        client.exchange_info_url().as_str(),
        "https://fapi.binance.com/fapi/v1/exchangeInfo"
    );
}

#[test]
fn rest_client_builds_klines_and_depth_urls() {
    let client = RestClient::new("https://fapi.binance.com").expect("valid REST base");

    assert_eq!(
        client.klines_url("BTCUSDT", "1m", 1500).as_str(),
        "https://fapi.binance.com/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=1500"
    );
    assert_eq!(
        client.depth_url("ETHUSDT", 1000).as_str(),
        "https://fapi.binance.com/fapi/v1/depth?symbol=ETHUSDT&limit=1000"
    );
    assert_eq!(
        client.funding_rate_url("SOLUSDT", 100).as_str(),
        "https://fapi.binance.com/fapi/v1/fundingRate?symbol=SOLUSDT&limit=100"
    );
}

#[test]
fn parse_klines_json_converts_closed_rest_rows_to_candles() {
    let json = serde_json::json!([[
        1714521600000_i64,
        "64000.0",
        "64200.0",
        "63950.0",
        "64100.0",
        "12.5",
        1714521659999_i64,
        "801250.0",
        120_u64,
        "6.0",
        "384600.0",
        "0"
    ]]);

    let candles = parse_klines_json("BTCUSDT", json).unwrap();

    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].symbol, "BTCUSDT");
    assert_eq!(candles[0].close, 64100.0);
    assert!(candles[0].is_closed);
    assert_eq!(candles[0].source, "rest");
}

#[test]
fn parse_depth_snapshot_json_converts_snapshot_levels() {
    let json = serde_json::json!({
        "lastUpdateId": 12345_u64,
        "bids": [["100.0", "2.0"], ["99.9", "1.0"]],
        "asks": [["100.1", "3.0"], ["100.2", "4.0"]]
    });

    let snapshot = parse_depth_snapshot_json("BTCUSDT", json).unwrap();

    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.last_update_id, 12345);
    assert_eq!(snapshot.bids[0].price, 100.0);
    assert_eq!(snapshot.asks[1].qty, 4.0);
}

#[test]
fn parse_funding_rates_json_extracts_funding_rates() {
    let json = serde_json::json!([
        {"symbol":"BTCUSDT","fundingRate":"0.0001","fundingTime":1714521600000_i64},
        {"symbol":"BTCUSDT","fundingRate":"0.0002","fundingTime":1714550400000_i64}
    ]);

    let rates = parse_funding_rates_json(json).unwrap();

    assert_eq!(rates, vec![0.0001, 0.0002]);
}

#[tokio::test]
async fn token_bucket_denies_when_empty() {
    let bucket = TokenBucket::new(1);
    assert!(bucket.try_take(1));
    assert!(!bucket.try_take(1));
}

#[tokio::test]
async fn token_bucket_try_take_zero_does_not_consume_tokens() {
    let bucket = TokenBucket::new(1);
    assert!(bucket.try_take(0));
    assert!(bucket.try_take(1));
    assert!(!bucket.try_take(1));
}

#[tokio::test]
async fn token_bucket_refills_up_to_capacity() {
    let bucket = TokenBucket::new(2);
    assert!(bucket.try_take(2));
    assert!(!bucket.try_take(1));

    bucket.refill(1);
    assert!(bucket.try_take(1));
    assert!(!bucket.try_take(1));

    bucket.refill(10);
    assert!(bucket.try_take(2));
    assert!(!bucket.try_take(1));
}

#[tokio::test]
async fn token_bucket_concurrent_takes_cannot_exceed_capacity() {
    let bucket = Arc::new(TokenBucket::new(4));
    let mut handles = Vec::new();

    for _ in 0..16 {
        let bucket = Arc::clone(&bucket);
        handles.push(tokio::spawn(async move { bucket.try_take(1) }));
    }

    let mut successful_takes = 0;
    for handle in handles {
        if handle.await.expect("task joined") {
            successful_takes += 1;
        }
    }

    assert_eq!(successful_takes, 4);
    assert!(!bucket.try_take(1));
}

#[tokio::test]
async fn stream_text_messages_returns_ok_when_receiver_is_closed() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local websocket listener");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept websocket client");
        let mut websocket = tokio_tungstenite::accept_async(socket)
            .await
            .expect("complete websocket handshake");
        futures_util::SinkExt::send(&mut websocket, Message::Text("closed".into()))
            .await
            .expect("send websocket text");
    });

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    drop(rx);

    let url = Url::parse(&format!("ws://{address}")).expect("websocket URL");
    let result = stream_text_messages(url, tx).await;
    server.await.expect("server joined");

    assert!(result.is_ok());
}
