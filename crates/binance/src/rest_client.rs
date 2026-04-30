use anyhow::Context;
use url::Url;

#[derive(Debug, Clone)]
pub struct RestClient {
    base: Url,
    client: reqwest::Client,
}

impl RestClient {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        Ok(Self {
            base: Url::parse(base)
                .with_context(|| format!("invalid Binance REST base URL: {base}"))?,
            client: reqwest::Client::new(),
        })
    }

    pub fn exchange_info_url(&self) -> Url {
        self.base
            .join("/fapi/v1/exchangeInfo")
            .expect("valid exchange info URL")
    }

    pub async fn exchange_info_json(&self) -> anyhow::Result<serde_json::Value> {
        let url = self.exchange_info_url();
        Ok(self
            .client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("send exchangeInfo request to {url}"))?
            .error_for_status()
            .with_context(|| format!("exchangeInfo request returned error status from {url}"))?
            .json()
            .await
            .with_context(|| format!("decode exchangeInfo JSON from {url}"))?)
    }
}
