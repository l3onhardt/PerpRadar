use perp_radar_binance::rate_limiter::TokenBucket;
use perp_radar_binance::rest_client::RestClient;

#[test]
fn rest_client_builds_exchange_info_url() {
    let client = RestClient::new("https://fapi.binance.com");
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
