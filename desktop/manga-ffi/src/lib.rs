//! The Swift-facing facade of the manga engine. Spike: one async call.

use std::sync::OnceLock;

uniffi::setup_scaffolding!();

#[derive(Debug, uniffi::Record)]
pub struct Card {
    pub id: String,
    pub title: String,
}

#[derive(Debug, uniffi::Error)]
pub enum MangaError {
    Failed { message: String },
}

impl std::fmt::Display for MangaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MangaError::Failed { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MangaError {}

fn failed(error: impl std::fmt::Display) -> MangaError {
    MangaError::Failed { message: error.to_string() }
}

/// MangaDex asks for a User-Agent that names the client and is not spoofed.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("akidev-manga/0.0-spike")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("http client builds")
    })
}

/// The most followed titles on MangaDex.
#[uniffi::export(async_runtime = "tokio")]
pub async fn popular(limit: u32) -> Result<Vec<Card>, MangaError> {
    let base = std::env::var("AKI_MANGADEX_BASE").unwrap_or_else(|_| "https://api.mangadex.org".into());
    let url = format!(
        "{base}/manga?limit={limit}&order[followedCount]=desc&contentRating[]=safe"
    );
    let body: serde_json::Value = client().get(url).send().await.map_err(failed)?.json().await.map_err(failed)?;
    let rows = body["data"].as_array().ok_or_else(|| failed("no data"))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let title = row["attributes"]["title"].as_object()?.values().next()?.as_str()?;
            Some(Card { id: row["id"].as_str()?.to_string(), title: title.to_string() })
        })
        .collect())
}
