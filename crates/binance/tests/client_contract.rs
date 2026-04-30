use perp_radar_binance::rate_limiter::TokenBucket;
use perp_radar_binance::rest_client::RestClient;
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
