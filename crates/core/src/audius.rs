//! Audius: a free catalogue of full tracks that needs no key. It stands in when YouTube Music
//! cannot answer, so its track ids carry a prefix and never clash with a YouTube video id.

use crate::http::client;
use crate::music::{Credit, Resolved, Track};
use anyhow::Context;
use serde::Deserialize;

const API: &str = "https://api.audius.co/v1";
const APP_NAME: &str = "akidev";
const PREFIX: &str = "audius:";

#[derive(Deserialize)]
struct Reply<T> {
    data: T,
}

#[derive(Deserialize)]
struct Found {
    id: String,
    title: String,
    duration: Option<u64>,
    #[serde(default)]
    is_streamable: bool,
    user: User,
    artwork: Option<Artwork>,
}

#[derive(Deserialize)]
struct User {
    name: String,
}

#[derive(Deserialize)]
struct Artwork {
    #[serde(rename = "480x480")]
    medium: Option<String>,
}

/// `audius:` and then the letters and digits Audius gives a track.
pub fn is_id(text: &str) -> bool {
    text.strip_prefix(PREFIX).is_some_and(|id| {
        (1..=16).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

pub async fn search(term: &str) -> anyhow::Result<Vec<Track>> {
    let reply: Reply<Vec<Found>> = client()
        .get(format!("{API}/tracks/search"))
        .query(&[("query", term), ("app_name", APP_NAME), ("limit", "20")])
        .send()
        .await
        .context("Audius did not answer")?
        .error_for_status()?
        .json()
        .await
        .context("Audius sent something unexpected")?;
    Ok(reply
        .data
        .into_iter()
        .filter(|found| found.is_streamable)
        .map(|found| Track {
            id: format!("{PREFIX}{}", found.id),
            title: found.title,
            artists: vec![Credit {
                id: None,
                name: found.user.name,
            }],
            album: None,
            duration: found.duration.map(|s| format!("{}:{:02}", s / 60, s % 60)),
            explicit: false,
            cover: found.artwork.and_then(|a| a.medium),
        })
        .collect())
}

/// The stream address redirects to a storage node, which the server and the player both follow, so
/// it is the same for everyone and can be kept as it is.
pub fn resolve(id: &str) -> Resolved {
    let id = id.strip_prefix(PREFIX).unwrap_or(id);
    Resolved {
        url: format!("{API}/tracks/{id}/stream?app_name={APP_NAME}"),
        content_type: "audio/mpeg",
        headers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_prefixed_ids_pass() {
        assert!(is_id("audius:vZJJz"));
        assert!(!is_id("vZJJz"));
        assert!(!is_id("audius:"));
        assert!(!is_id("audius:../x"));
        assert!(!is_id("audius:abcdefghijklmnopq"));
    }

    #[test]
    fn resolve_points_at_the_stream() {
        let found = resolve("audius:vZJJz");
        assert_eq!(
            found.url,
            "https://api.audius.co/v1/tracks/vZJJz/stream?app_name=akidev"
        );
        assert_eq!(found.content_type, "audio/mpeg");
    }

    // Live: needs the network. `cargo test -p akidev-core audius -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_search_finds_playable_tracks() {
        let found = search("daft punk").await.unwrap();
        assert!(!found.is_empty());
        assert!(found.iter().all(|t| is_id(&t.id)));
    }
}
