use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsBase {
    Market(String),
    Public(String),
}

impl WsBase {
    fn as_str(&self) -> &str {
        match self {
            WsBase::Market(value) | WsBase::Public(value) => value,
        }
    }
}

pub fn combined_stream_url(base: WsBase, streams: &[&str]) -> anyhow::Result<Url> {
    anyhow::ensure!(!streams.is_empty(), "at least one stream is required");

    let joined = streams.join("/");
    let url = format!(
        "{}/stream?streams={}",
        base.as_str().trim_end_matches('/'),
        joined
    );
    Ok(Url::parse(&url)?)
}
