//! The manga engine's HTTP client. It is its own (rustls, TLS 1.2 and 1.3) because the workspace client uses the
//! system's native TLS, which on Apple platforms cannot speak TLS 1.3, and sites such as MangaDex now require it.

use std::sync::OnceLock;
use std::time::Duration;

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("AkiManga/1.0")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client builds")
    })
}

/// Reads a response body, failing once it grows past `max` bytes.
pub async fn read_capped(mut response: reqwest::Response, max: usize) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(body.len() + chunk.len() <= max, "the response is too large");
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
