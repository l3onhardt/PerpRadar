use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub session_rollover_secs: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 2_000,
            max_delay_ms: 60_000,
            session_rollover_secs: 23 * 60 * 60,
        }
    }
}

impl ReconnectPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let multiplier = 1_u64.checked_shl(attempt.min(16)).unwrap_or(u64::MAX);
        let delay = self
            .initial_delay_ms
            .saturating_mul(multiplier)
            .min(self.max_delay_ms);
        std::time::Duration::from_millis(delay)
    }
}

pub async fn stream_text_messages(url: Url, tx: mpsc::Sender<String>) -> anyhow::Result<()> {
    let (stream, _) = tokio_tungstenite::connect_async(url.as_str()).await?;
    let (_, mut read) = stream.split();

    while let Some(message) = read.next().await {
        if let Message::Text(text) = message? {
            if tx.send(text.to_string()).await.is_err() {
                return Ok(());
            }
        }
    }

    Ok(())
}
