use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

pub async fn stream_text_messages(url: Url, tx: mpsc::Sender<String>) -> anyhow::Result<()> {
    let (stream, _) = tokio_tungstenite::connect_async(url.as_str()).await?;
    let (_, mut read) = stream.split();

    while let Some(message) = read.next().await {
        if let Message::Text(text) = message? {
            tx.send(text.to_string()).await?;
        }
    }

    Ok(())
}
