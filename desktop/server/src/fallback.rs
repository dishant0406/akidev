//! A search through Prowlarr, for what the indexers in `sources.json` could not find.
//!
//! Prowlarr runs next to this server and knows hundreds of torrent sites. This asks it, and hands back
//! what it found in the shape an indexer definition reads. It is on only when `PROWLARR_URL` and
//! `PROWLARR_API_KEY` are set.
//!
//! Most sites put the info hash in the result. A few (LimeTorrents) only give a download link, which
//! Prowlarr answers with a redirect to a magnet after fetching the site's page: slow, so it is done
//! for the best few results only, and remembered.
//!
//! A search runs in a task of its own, so it finishes and is remembered even if the one who asked has
//! gone, and the same search asked twice at once runs once.

use akidev_core::magnet::{info_hash_from_magnet, to_hex_info_hash};
use akidev_core::model::Kind;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Semaphore;

/// Searches that may run at once. Prowlarr asks every site it knows, which is slow and heavy.
const AT_ONCE: usize = 2;
/// How long a search waits for its turn before it is given up as busy.
const QUEUE_FOR: Duration = Duration::from_secs(5);
/// The longest Prowlarr is given for the search itself.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(9);
/// The longest a search takes in all, links included, a little under the 15 seconds the apps wait.
/// Results whose hash was not found by then are left out.
const TOTAL_TIME: Duration = Duration::from_secs(13);
/// A search is kept this long, so the same one asked again (the apps ask once per title) is free.
const ANSWER_TTL: Duration = Duration::from_secs(10 * 60);
/// A hash found through a download link is kept this long, and so is a link that gave none.
const HASH_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Results with no hash in them that get a download link followed, best seeded first. Prowlarr asks a
/// site for one at a time, so each one followed is a wait.
const LINKS_FOLLOWED: usize = 4;
const MAX_RESULTS: usize = 100;
/// Entries each memory may hold before it is emptied, so it cannot grow for ever.
const MAX_ANSWERS: usize = 300;
const MAX_HASHES: usize = 5_000;

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub title: String,
    pub hash: String,
    pub size: Option<u64>,
    pub seeders: Option<u32>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// Two searches are already running and none finished in time.
    Busy,
    /// Prowlarr asked the client to wait before it asks again.
    RateLimited(Option<u64>),
    Failed,
}

type Flight = Shared<BoxFuture<'static, Result<Vec<Hit>, Failure>>>;

enum AskError {
    Failed(anyhow::Error),
    RateLimited(Option<u64>),
}

/// One result as Prowlarr's search answers it.
#[derive(Deserialize)]
struct Row {
    title: String,
    #[serde(rename = "infoHash")]
    info_hash: Option<String>,
    #[serde(rename = "magnetUrl")]
    magnet_url: Option<String>,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    size: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_seeders")]
    seeders: Option<u32>,
    indexer: Option<String>,
    protocol: Option<String>,
    #[serde(default, deserialize_with = "deserialize_categories")]
    categories: Vec<u32>,
}

pub struct Fallback {
    /// Prowlarr's address, like `http://prowlarr:9696`, without a trailing slash.
    base: String,
    key: String,
    /// Follows no redirects: a download link's answer is the redirect, read and not followed.
    client: reqwest::Client,
    turns: Semaphore,
    answers: Mutex<HashMap<String, (Instant, Vec<Hit>)>>,
    hashes: Mutex<HashMap<String, (Instant, Option<String>)>>,
    running: Mutex<HashMap<String, Flight>>,
    rate_limited_until: Mutex<Option<Instant>>,
}

/// What is searched for: the title, with the season and episode the way sites name them (`S01E05`),
/// or for an anime only the number.
fn search_text(query: &str, season: Option<u32>, episode: Option<u32>) -> String {
    match (season, episode) {
        (Some(season), Some(episode)) => format!("{query} S{season:02}E{episode:02}"),
        (None, Some(episode)) => format!("{query} {episode:02}"),
        _ => query.to_string(),
    }
}

/// Prowlarr categories for the media kind. An empty category list on a result means unknown.
fn categories(kind: Kind) -> &'static [u32] {
    match kind {
        Kind::Anime => &[5070],
        Kind::Series => &[5000],
        _ => &[2000],
    }
}

fn category_allowed(kind: Kind, category: u32) -> bool {
    match kind {
        Kind::Anime => category == 5070,
        Kind::Series => (5000..6000).contains(&category),
        _ => (2000..3000).contains(&category),
    }
}

fn cache_key(text: &str, kind: Kind) -> String {
    format!("{}:{text}", kind.as_str()).to_lowercase()
}

fn retry_after(value: &str) -> Option<u64> {
    if let Ok(seconds) = value.parse() {
        return Some(seconds);
    }
    let date = httpdate::parse_http_date(value).ok()?;
    Some(
        date.duration_since(SystemTime::now())
            .unwrap_or_default()
            .as_secs(),
    )
}

impl Fallback {
    /// `None` unless both are set.
    pub fn new(base: &str, key: &str) -> Option<Fallback> {
        let base = base.trim().trim_end_matches('/');
        if base.is_empty() || key.is_empty() {
            return None;
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(TOTAL_TIME)
            .build()
            .ok()?;
        Some(Fallback {
            base: base.to_string(),
            key: key.to_string(),
            client,
            turns: Semaphore::new(AT_ONCE),
            answers: Mutex::default(),
            hashes: Mutex::default(),
            running: Mutex::default(),
            rate_limited_until: Mutex::new(None),
        })
    }

    /// The answer to a search made lately, which costs nothing to give.
    pub fn cached(
        &self,
        query: &str,
        kind: Kind,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Option<Vec<Hit>> {
        self.remembered(&cache_key(&search_text(query, season, episode), kind))
    }

    /// What Prowlarr finds for `query`. The same search asked while it is running joins it.
    pub async fn search(
        self: &Arc<Self>,
        query: &str,
        kind: Kind,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Result<Vec<Hit>, Failure> {
        let text = search_text(query, season, episode);
        let key = cache_key(&text, kind);
        if let Some(hits) = self.remembered(&key) {
            return Ok(hits);
        }
        let flight = self
            .running
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_insert_with(|| {
                let (me, key) = (self.clone(), key.clone());
                let task = tokio::spawn(async move {
                    let result = me.find(&text, &key, kind).await;
                    me.running.lock().unwrap().remove(&key);
                    result
                });
                async move { task.await.unwrap_or(Err(Failure::Failed)) }
                    .boxed()
                    .shared()
            })
            .clone();
        flight.await
    }

    /// Whether Prowlarr answers and how many of its indexers are on and failing, for the health report.
    pub async fn status(&self) -> serde_json::Value {
        let reachable = self
            .client
            .get(format!("{}/ping", self.base))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success());
        if !reachable {
            return serde_json::json!({ "configured": true, "reachable": false });
        }
        let list = |path: &'static str| async move {
            let response = self
                .client
                .get(format!("{}{path}", self.base))
                .header("X-Api-Key", &self.key)
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .ok()?;
            response.json::<Vec<serde_json::Value>>().await.ok()
        };
        let indexers = list("/api/v1/indexer").await.map(|all| {
            all.iter()
                .filter(|indexer| indexer["enable"] == true)
                .count()
        });
        let failing = list("/api/v1/indexerstatus").await.map(|all| all.len());
        serde_json::json!({
            "configured": true, "reachable": true, "indexers": indexers, "failing": failing,
        })
    }

    fn remembered(&self, key: &str) -> Option<Vec<Hit>> {
        let answers = self.answers.lock().unwrap();
        let (when, hits) = answers.get(key)?;
        (when.elapsed() < ANSWER_TTL).then(|| hits.clone())
    }

    fn rate_limit_wait(&self) -> Option<u64> {
        let until = *self.rate_limited_until.lock().unwrap();
        let wait = until?.checked_duration_since(Instant::now())?;
        Some(wait.as_secs().max(1))
    }

    fn remember_rate_limit(&self, retry: u64) {
        *self.rate_limited_until.lock().unwrap() =
            Some(Instant::now() + Duration::from_secs(retry));
    }

    async fn find(
        self: &Arc<Self>,
        text: &str,
        key: &str,
        kind: Kind,
    ) -> Result<Vec<Hit>, Failure> {
        let _turn = tokio::time::timeout(QUEUE_FOR, self.turns.acquire())
            .await
            .map_err(|_| Failure::Busy)?
            .map_err(|_| Failure::Failed)?;
        if let Some(wait) = self.rate_limit_wait() {
            return Err(Failure::RateLimited(Some(wait)));
        }
        let deadline = Instant::now() + TOTAL_TIME;
        let rows = self.ask(text, kind).await.map_err(|error| match error {
            AskError::RateLimited(retry) => {
                let retry = retry.unwrap_or(30);
                self.remember_rate_limit(retry);
                Failure::RateLimited(Some(retry))
            }
            AskError::Failed(error) => {
                tracing::warn!("[fallback] search for {text:?} failed: {error}");
                Failure::Failed
            }
        })?;
        let (hits, complete) = self.hits(rows, deadline, kind).await;
        tracing::info!("[fallback] {text:?}: {} results", hits.len());
        // An answer that is missing links still being followed is not kept, so the next search for
        // it finds them in the hash memory, which they fill when they finish.
        if complete {
            let mut answers = self.answers.lock().unwrap();
            if answers.len() >= MAX_ANSWERS {
                answers.clear();
            }
            answers.insert(key.to_string(), (Instant::now(), hits.clone()));
        }
        Ok(hits)
    }

    async fn ask(&self, text: &str, kind: Kind) -> Result<Vec<Row>, AskError> {
        let mut request = self
            .client
            .get(format!("{}/api/v1/search", self.base))
            .header("X-Api-Key", &self.key)
            .query(&[("query", text), ("type", "search"), ("limit", "100")]);
        for category in categories(kind) {
            request = request.query(&[("categories", *category)]);
        }
        let response = request
            .timeout(SEARCH_TIMEOUT)
            .send()
            .await
            .map_err(|error| AskError::Failed(error.into()))?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(retry_after);
            return Err(AskError::RateLimited(retry));
        }
        let response = response
            .error_for_status()
            .map_err(|error| AskError::Failed(error.into()))?;
        response
            .json()
            .await
            .map_err(|error| AskError::Failed(error.into()))
    }

    /// The torrent results that have a hash, or whose hash was found by `deadline`, best seeded
    /// first, and whether every link that was followed finished in time.
    async fn hits(
        self: &Arc<Self>,
        rows: Vec<Row>,
        deadline: Instant,
        kind: Kind,
    ) -> (Vec<Hit>, bool) {
        let mut known = Vec::new();
        let mut without = Vec::new();
        for row in rows {
            if row.protocol.as_deref().is_some_and(|p| p != "torrent") {
                continue;
            }
            if !row
                .categories
                .iter()
                .any(|category| category_allowed(kind, *category))
            {
                continue;
            }
            match hash_in(&row) {
                Some(hash) => known.push((row, hash)),
                None if row.download_url.is_some() => without.push(row),
                None => {}
            }
        }
        without.sort_by_key(|row| std::cmp::Reverse(row.seeders));
        without.truncate(LINKS_FOLLOWED);

        // Each link is its own task, so one that is too slow for this search still finishes and is
        // remembered for the next.
        let links: Vec<_> = without
            .iter()
            .map(|row| {
                let (me, link) = (self.clone(), row.download_url.clone().unwrap_or_default());
                tokio::spawn(async move { me.hash_from_link(&link).await })
            })
            .collect();
        let mut complete = true;
        let mut found = Vec::new();
        for link in links {
            match tokio::time::timeout_at(deadline.into(), link).await {
                Ok(Ok(hash)) => found.push(hash),
                _ => {
                    complete = false;
                    found.push(None);
                }
            }
        }
        known.extend(
            without
                .into_iter()
                .zip(found)
                .filter_map(|(row, hash)| Some((row, hash?))),
        );

        let mut seen = HashSet::new();
        let mut hits: Vec<Hit> = known
            .into_iter()
            .filter(|(_, hash)| seen.insert(hash.clone()))
            .map(|(row, hash)| Hit {
                title: row.title,
                hash,
                size: row.size,
                seeders: row.seeders,
                source: row.indexer.unwrap_or_default(),
            })
            .collect();
        hits.sort_by_key(|hit| std::cmp::Reverse(hit.seeders));
        hits.truncate(MAX_RESULTS);
        (hits, complete)
    }

    /// Follows a result's download link, which Prowlarr answers with a redirect to a magnet. A link
    /// that answered without one is remembered as having none; one that could not be reached is not.
    async fn hash_from_link(&self, link: &str) -> Option<String> {
        let link = self.own_address(link)?;
        if let Some((when, hash)) = self.hashes.lock().unwrap().get(&link)
            && when.elapsed() < HASH_TTL
        {
            return hash.clone();
        }
        let response = self.client.get(&link).send().await.ok()?;
        let hash = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|location| info_hash_from_magnet(location.to_str().ok()?));
        let mut hashes = self.hashes.lock().unwrap();
        if hashes.len() >= MAX_HASHES {
            hashes.clear();
        }
        hashes.insert(link, (Instant::now(), hash.clone()));
        hash
    }

    /// A download link with Prowlarr's address as this server knows it. The link in an answer names
    /// whatever host Prowlarr was reached by, and that must not decide where this server connects.
    fn own_address(&self, link: &str) -> Option<String> {
        let link = reqwest::Url::parse(link).ok()?;
        let query = link.query().map(|query| format!("?{query}"));
        Some(format!(
            "{}{}{}",
            self.base,
            link.path(),
            query.unwrap_or_default()
        ))
    }
}

/// Prowlarr returns category objects. Older versions and test doubles can return numbers.
fn deserialize_categories<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items.iter().filter_map(category_id).collect())
}

fn category_id(value: &serde_json::Value) -> Option<u32> {
    match value {
        serde_json::Value::Number(number) => number.as_u64().and_then(|id| u32::try_from(id).ok()),
        serde_json::Value::String(text) => text.parse().ok(),
        serde_json::Value::Object(object) => object
            .get("id")
            .or_else(|| object.get("ID"))
            .and_then(category_id),
        _ => None,
    }
}

/// Missing or malformed provider counts are unknown, never a fabricated zero.
fn deserialize_seeders<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_u64().and_then(|count| u32::try_from(count).ok()))
}

/// The hash a result carries, in its own field or in its magnet link.
fn hash_in(row: &Row) -> Option<String> {
    row.info_hash
        .as_deref()
        .and_then(to_hex_info_hash)
        .or_else(|| info_hash_from_magnet(row.magnet_url.as_deref()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidev_core::model::Kind;
    use axum::extract::{Query, State};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::IntoResponse;
    use axum::{Json, Router, routing::get};
    use futures::future::join_all;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const KEY: &str = "secret";
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccc";

    #[derive(Default)]
    struct Seen {
        searches: AtomicUsize,
        links: AtomicUsize,
        last_query: Mutex<String>,
        last_categories: Mutex<Vec<String>>,
    }

    /// A stand-in for Prowlarr: a search (which takes a moment) with a result that has a hash, one
    /// that only has a magnet, one that only has a link that redirects to a magnet, one whose link
    /// gives nothing, and a usenet result. The links name a host that does not exist.
    async fn prowlarr() -> (String, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        let app = Router::new()
            .route(
                "/api/v1/search",
                get(
                    |State(seen): State<Arc<Seen>>,
                     headers: HeaderMap,
                     Query(query): Query<HashMap<String, String>>| async move {
                        if headers.get("X-Api-Key").and_then(|v| v.to_str().ok()) != Some(KEY) {
                            return StatusCode::UNAUTHORIZED.into_response();
                        }
                        seen.searches.fetch_add(1, Ordering::SeqCst);
                        *seen.last_query.lock().unwrap() = query["query"].clone();
                        *seen.last_categories.lock().unwrap() =
                            query.get("categories").cloned().into_iter().collect();
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let link = |name: &str| format!("http://nowhere.invalid:9696/1/download?apikey=x&link={name}");
                        Json(json!([
                            { "title": "Has hash", "infoHash": A, "seeders": 5, "size": 10, "indexer": "one", "protocol": "torrent", "categories": [{ "id": 5040, "name": "TV" }] },
                            { "title": "Magnet only", "magnetUrl": format!("magnet:?xt=urn:btih:{B}&dn=x"), "seeders": 2, "indexer": "two", "protocol": "torrent", "categories": [{ "ID": "5000" }] },
                            { "title": "Link only", "downloadUrl": link("good"), "seeders": 9, "indexer": "three", "protocol": "torrent", "categories": [{ "id": 5000 }] },
                            { "title": "Dead link", "downloadUrl": link("dead"), "seeders": 1, "indexer": "three", "protocol": "torrent", "categories": [{ "id": 5000 }] },
                            { "title": "Music", "infoHash": A, "categories": [{ "id": 3000, "name": "Audio" }], "indexer": "audio", "protocol": "torrent" },
                            { "title": "Usenet", "infoHash": B, "protocol": "usenet" }
                        ]))
                        .into_response()
                    },
                ),
            )
            .route(
                "/1/download",
                get(
                    |State(seen): State<Arc<Seen>>, Query(query): Query<HashMap<String, String>>| async move {
                        seen.links.fetch_add(1, Ordering::SeqCst);
                        if query["link"] == "slow" {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        if query["link"] == "good" || query["link"] == "slow" {
                            (
                                StatusCode::MOVED_PERMANENTLY,
                                [(header::LOCATION, format!("magnet:?xt=urn:btih:{C}&dn=y"))],
                            )
                                .into_response()
                        } else {
                            StatusCode::NOT_FOUND.into_response()
                        }
                    },
                ),
            )
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), seen)
    }

    fn fallback(base: &str) -> Arc<Fallback> {
        Arc::new(Fallback::new(base, KEY).unwrap())
    }

    #[tokio::test]
    async fn results_come_back_with_hashes_from_every_place_one_can_be_found() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        let hits = fallback
            .search("Show", Kind::Series, None, None)
            .await
            .unwrap();

        let by_title: HashMap<&str, &str> = hits
            .iter()
            .map(|hit| (hit.title.as_str(), hit.hash.as_str()))
            .collect();
        assert_eq!(by_title["Has hash"], A);
        assert_eq!(by_title["Magnet only"], B);
        // Its link names another host, and was followed at Prowlarr's own address all the same.
        assert_eq!(by_title["Link only"], C);
        assert!(!by_title.contains_key("Dead link"));
        assert!(!by_title.contains_key("Music"));
        assert!(!by_title.contains_key("Usenet"));
        // Best seeded first, and nothing handed out carries the key.
        assert_eq!(hits[0].title, "Link only");
        assert!(!serde_json::to_string(&hits).unwrap().contains("apikey"));
        assert_eq!(seen.links.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_media_kind_selects_categories_and_is_part_of_the_cache_key() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        fallback
            .search("Show", Kind::Anime, None, Some(1))
            .await
            .unwrap();
        assert_eq!(*seen.last_categories.lock().unwrap(), ["5070"]);

        fallback
            .search("Show", Kind::Series, Some(1), Some(1))
            .await
            .unwrap();
        assert_eq!(*seen.last_categories.lock().unwrap(), ["5000"]);

        fallback
            .search("Show", Kind::Movie, None, None)
            .await
            .unwrap();
        assert_eq!(*seen.last_categories.lock().unwrap(), ["2000"]);
        assert_eq!(seen.searches.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn retry_after_accepts_an_http_date() {
        let date = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(30));
        let wait = retry_after(&date).unwrap();
        assert!((25..=30).contains(&wait));
    }

    #[tokio::test]
    async fn a_provider_rate_limit_keeps_its_retry_after_value() {
        let searches = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route(
                "/api/v1/search",
                get(|State(searches): State<Arc<AtomicUsize>>| async move {
                    searches.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        [(header::RETRY_AFTER, "7")],
                        Json(json!({ "error": "slow down" })),
                    )
                }),
            )
            .with_state(searches.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let fallback = fallback(&format!("http://{address}"));
        let result = fallback.search("Show", Kind::Movie, None, None).await;
        assert!(matches!(result, Err(Failure::RateLimited(Some(7)))));
        let result = fallback.search("Other", Kind::Movie, None, None).await;
        assert!(matches!(result, Err(Failure::RateLimited(Some(wait))) if wait > 0));
        assert_eq!(searches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_same_search_twice_asks_prowlarr_once() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        fallback
            .search("Show", Kind::Series, None, None)
            .await
            .unwrap();
        assert!(fallback.cached("SHOW", Kind::Series, None, None).is_some());
        fallback
            .search("SHOW", Kind::Series, None, None)
            .await
            .unwrap();
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);

        // A different title is a different search, but the links already followed are remembered.
        fallback
            .search("Other", Kind::Series, None, None)
            .await
            .unwrap();
        assert_eq!(seen.searches.load(Ordering::SeqCst), 2);
        assert_eq!(seen.links.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_same_search_asked_at_once_runs_once() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        let asks = (0..4).map(|_| fallback.search("Show", Kind::Series, None, None));
        let answers = join_all(asks).await;

        assert!(
            answers
                .iter()
                .all(|answer| answer.as_ref().unwrap().len() == 3)
        );
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_search_goes_on_and_is_kept_when_the_one_who_asked_has_gone() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        let asker = {
            let fallback = fallback.clone();
            tokio::spawn(async move { fallback.search("Show", Kind::Series, None, None).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        asker.abort();

        // It finishes by itself, and the next one to ask has the answer at once.
        for _ in 0..50 {
            if fallback.cached("Show", Kind::Series, None, None).is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(fallback.cached("Show", Kind::Series, None, None).is_some());
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_episode_is_searched_the_way_sites_name_it() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        fallback
            .search("Show", Kind::Series, Some(1), Some(5))
            .await
            .unwrap();
        assert_eq!(*seen.last_query.lock().unwrap(), "Show S01E05");

        fallback
            .search("Show", Kind::Anime, None, Some(5))
            .await
            .unwrap();
        assert_eq!(*seen.last_query.lock().unwrap(), "Show 05");
    }

    #[tokio::test]
    async fn a_wrong_key_or_a_dead_prowlarr_is_a_failure_and_never_a_hang() {
        let (base, _) = prowlarr().await;
        let wrong = Arc::new(Fallback::new(&base, "not the key").unwrap());
        assert_eq!(
            wrong
                .search("Show", Kind::Series, None, None)
                .await
                .unwrap_err(),
            Failure::Failed
        );

        let nobody = fallback("http://127.0.0.1:1");
        assert_eq!(
            nobody
                .search("Show", Kind::Series, None, None)
                .await
                .unwrap_err(),
            Failure::Failed
        );
    }

    fn row(title: &str, hash: Option<&str>, base: &str, link: &str) -> Row {
        Row {
            title: title.into(),
            info_hash: hash.map(Into::into),
            magnet_url: None,
            download_url: Some(format!("{base}/1/download?link={link}")),
            size: None,
            seeders: Some(3),
            indexer: None,
            protocol: Some("torrent".into()),
            categories: vec![5000],
        }
    }

    #[tokio::test]
    async fn a_link_that_is_too_slow_is_left_out_but_still_remembered_when_it_finishes() {
        let (base, _) = prowlarr().await;
        let fallback = fallback(&base);
        let rows = vec![
            row("Fast", Some(A), &base, "good"),
            row("Slow", None, &base, "slow"),
        ];

        let started = Instant::now();
        let (hits, complete) = fallback
            .hits(
                rows,
                Instant::now() + Duration::from_millis(200),
                Kind::Series,
            )
            .await;

        assert!(started.elapsed() < Duration::from_millis(900));
        assert!(!complete, "an answer that missed a link must not be kept");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Fast");

        // The link goes on by itself, and the next search finds its hash waiting.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let (hits, complete) = fallback
            .hits(
                vec![row("Slow", None, &base, "slow")],
                Instant::now() + Duration::from_millis(200),
                Kind::Series,
            )
            .await;
        assert!(complete);
        assert_eq!(hits[0].hash, C);
    }

    #[tokio::test]
    async fn a_link_that_could_not_be_reached_is_not_remembered_but_one_with_no_magnet_is() {
        let (base, _) = prowlarr().await;

        let nobody = fallback("http://127.0.0.1:1");
        assert_eq!(
            nobody.hash_from_link("http://x/1/download?link=a").await,
            None
        );
        assert!(nobody.hashes.lock().unwrap().is_empty());

        let fallback = fallback(&base);
        assert_eq!(
            fallback
                .hash_from_link("http://x/1/download?link=dead")
                .await,
            None
        );
        assert_eq!(fallback.hashes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_status_counts_the_indexers_that_are_on_and_failing() {
        use axum::routing::get;
        let app = Router::new()
            .route("/ping", get(|| async { "ok" }))
            .route(
                "/api/v1/indexer",
                get(|| async {
                    Json(json!([{ "enable": true }, { "enable": true }, { "enable": false }]))
                }),
            )
            .route(
                "/api/v1/indexerstatus",
                get(|| async { Json(json!([{ "indexerId": 2 }])) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let status = fallback(&format!("http://{address}")).status().await;
        assert_eq!(
            status,
            json!({ "configured": true, "reachable": true, "indexers": 2, "failing": 1 })
        );
        let down = fallback("http://127.0.0.1:1").status().await;
        assert_eq!(down, json!({ "configured": true, "reachable": false }));
    }

    #[tokio::test]
    async fn missing_and_invalid_seeders_stay_null_while_zero_is_known() {
        let fallback = fallback("http://127.0.0.1:1");
        for seeders in [
            None,
            Some(json!(null)),
            Some(json!(-1)),
            Some(json!(1.5)),
            Some(json!("12")),
            Some(json!(true)),
            Some(json!(u64::MAX)),
            Some(json!(0)),
            Some(json!(u32::MAX)),
        ] {
            let mut body = json!({ "title": "Count", "infoHash": A, "categories": [5000] });
            if let Some(value) = seeders.clone() {
                body["seeders"] = value;
            }
            let row: Row = serde_json::from_value(body).unwrap();
            let expected = seeders
                .as_ref()
                .and_then(|value| value.as_u64())
                .and_then(|count| u32::try_from(count).ok());
            let (hits, complete) = fallback.hits(vec![row], Instant::now(), Kind::Series).await;
            assert!(complete);
            assert_eq!(hits[0].seeders, expected);
            assert_eq!(
                serde_json::to_value(&hits[0]).unwrap()["seeders"],
                json!(expected)
            );
        }
    }

    #[tokio::test]
    async fn anime_results_must_use_the_anime_category() {
        let fallback = fallback("http://127.0.0.1:1");
        let mut tv = row("Show", Some(A), "http://127.0.0.1:1", "unused");
        tv.categories = vec![5040];
        let (hits, _) = fallback.hits(vec![tv], Instant::now(), Kind::Anime).await;
        assert!(hits.is_empty());

        let mut anime = row("Show", Some(A), "http://127.0.0.1:1", "unused");
        anime.categories = vec![5070];
        let (hits, _) = fallback
            .hits(vec![anime], Instant::now(), Kind::Anime)
            .await;
        assert_eq!(hits.len(), 1);

        let mut unknown = row("Show", Some(A), "http://127.0.0.1:1", "unused");
        unknown.categories.clear();
        let (hits, _) = fallback
            .hits(vec![unknown], Instant::now(), Kind::Anime)
            .await;
        assert!(hits.is_empty());
    }

    #[test]
    fn prowlarr_categories_can_be_objects_numbers_strings_or_null() {
        for categories in [
            json!([{ "id": 5040, "name": "TV" }]),
            json!([5040]),
            json!(["5040"]),
            json!(null),
        ] {
            let expected = if categories.is_null() {
                Vec::new()
            } else {
                vec![5040]
            };
            let row: Row = serde_json::from_value(json!({
                "title": "Show",
                "infoHash": A,
                "categories": categories,
            }))
            .unwrap();
            assert_eq!(row.categories, expected);
        }
    }

    #[test]
    fn it_is_off_without_an_address_and_a_key() {
        assert!(Fallback::new("", KEY).is_none());
        assert!(Fallback::new("http://prowlarr:9696", "").is_none());
        assert!(Fallback::new("http://prowlarr:9696/", KEY).is_some());
    }
}
