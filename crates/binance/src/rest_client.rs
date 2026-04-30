use url::Url;

#[derive(Debug, Clone)]
pub struct RestClient {
    base: Url,
    client: reqwest::Client,
}

impl RestClient {
    pub fn new(base: &str) -> Self {
        Self {
            base: Url::parse(base).expect("valid Binance REST base URL"),
            client: reqwest::Client::new(),
        }
    }

    pub fn exchange_info_url(&self) -> Url {
        self.base
            .join("/fapi/v1/exchangeInfo")
            .expect("valid exchange info URL")
    }

    pub async fn exchange_info_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(self
            .client
            .get(self.exchange_info_url())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}
