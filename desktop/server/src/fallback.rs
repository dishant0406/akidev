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
//! The sites behind Cloudflare (tagged `solver` in Prowlarr) are searched apart from the others, and
//! at the same time, so a slow one costs its own results and not everyone's.
//!
//! A search runs in a task of its own, so it finishes and is remembered even if the one who asked has
//! gone, and the same search asked twice at once runs once.

use akidev_core::magnet::{info_hash_from_magnet, to_hex_info_hash};
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Searches that may run at once. Prowlarr asks every site it knows, which is slow and heavy.
const AT_ONCE: usize = 2;
/// How long a search waits for its turn before it is given up as busy.
const QUEUE_FOR: Duration = Duration::from_secs(5);
/// The longest Prowlarr is given to search the sites that answer at once.
const FAST_SEARCH: Duration = Duration::from_secs(9);
/// The same for the sites behind Cloudflare, which a browser opens one page at a time: a cold one takes
/// ten seconds or more.
const SLOW_SEARCH: Duration = Duration::from_secs(12);
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
    pub seeders: u32,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// Two searches are already running and none finished in time.
    Busy,
    Failed,
}

/// Prowlarr's indexers, by how quickly they answer.
struct Tiers {
    fast: Vec<u64>,
    slow: Vec<u64>,
}

type Flight = Shared<BoxFuture<'static, Result<Vec<Hit>, Failure>>>;

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
    seeders: Option<i64>,
    indexer: Option<String>,
    protocol: Option<String>,
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
    tiers: Mutex<Option<(Instant, Arc<Tiers>)>>,
    fast_search: Duration,
    slow_search: Duration,
    total: Duration,
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
            tiers: Mutex::default(),
            fast_search: FAST_SEARCH,
            slow_search: SLOW_SEARCH,
            total: TOTAL_TIME,
        })
    }

    /// The answer to a search made lately, which costs nothing to give.
    pub fn cached(
        &self,
        query: &str,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Option<Vec<Hit>> {
        self.remembered(&search_text(query, season, episode).to_lowercase())
    }

    /// What Prowlarr finds for `query`. The same search asked while it is running joins it.
    pub async fn search(
        self: &Arc<Self>,
        query: &str,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Result<Vec<Hit>, Failure> {
        let text = search_text(query, season, episode);
        let key = text.to_lowercase();
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
                    let result = me.find(&text, &key).await;
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

    async fn find(self: &Arc<Self>, text: &str, key: &str) -> Result<Vec<Hit>, Failure> {
        let _turn = tokio::time::timeout(QUEUE_FOR, self.turns.acquire())
            .await
            .map_err(|_| Failure::Busy)?
            .map_err(|_| Failure::Failed)?;
        let (hits, complete) = match self.tiers().await.as_deref() {
            Some(Tiers { fast, slow }) if !fast.is_empty() && !slow.is_empty() => {
                let (first, second) = tokio::join!(
                    self.tier(text, Some(fast), self.fast_search),
                    self.tier(text, Some(slow), self.slow_search)
                );
                match (first, second) {
                    (Err(_), Err(_)) => return Err(Failure::Failed),
                    // One tier failing or running out of time costs its own results only.
                    (Ok((hits, _)), Err(_)) | (Err(_), Ok((hits, _))) => (hits, false),
                    (Ok((mut hits, one)), Ok((more, other))) => {
                        let mut seen: HashSet<String> =
                            hits.iter().map(|hit| hit.hash.clone()).collect();
                        hits.extend(more.into_iter().filter(|hit| seen.insert(hit.hash.clone())));
                        hits.sort_by_key(|hit| std::cmp::Reverse(hit.seeders));
                        hits.truncate(MAX_RESULTS);
                        (hits, one && other)
                    }
                }
            }
            // Nothing to tell apart: one search of everything, given as long as the slow ones need.
            _ => self
                .tier(text, None, self.slow_search)
                .await
                .map_err(|_| Failure::Failed)?,
        };
        tracing::info!("[fallback] {text:?}: {} results", hits.len());
        // An answer that is missing links or a whole tier still being worked on is not kept, so the
        // next search for it finds what finished in the memories below.
        if complete {
            let mut answers = self.answers.lock().unwrap();
            if answers.len() >= MAX_ANSWERS {
                answers.clear();
            }
            answers.insert(key.to_string(), (Instant::now(), hits.clone()));
        }
        Ok(hits)
    }

    /// One search of some of Prowlarr's indexers (all of them for `None`), given `search` to answer,
    /// and then its links followed, all within the total time.
    async fn tier(
        self: &Arc<Self>,
        text: &str,
        indexers: Option<&[u64]>,
        search: Duration,
    ) -> anyhow::Result<(Vec<Hit>, bool)> {
        let deadline = Instant::now() + self.total;
        let rows = self.ask(text, indexers, search).await.map_err(|error| {
            tracing::warn!("[fallback] search for {text:?} failed: {error}");
            error
        })?;
        Ok(self.hits(rows, deadline).await)
    }

    /// Which of Prowlarr's enabled indexers answer at once and which are behind the solver (tagged
    /// `solver`), looked up every ten minutes. `None` when that cannot be read.
    async fn tiers(&self) -> Option<Arc<Tiers>> {
        if let Some((when, tiers)) = &*self.tiers.lock().unwrap()
            && when.elapsed() < ANSWER_TTL
        {
            return Some(tiers.clone());
        }
        let list = |path: &'static str| async move {
            self.client
                .get(format!("{}{path}", self.base))
                .header("X-Api-Key", &self.key)
                .timeout(Duration::from_secs(3))
                .send()
                .await
                .ok()?
                .json::<Vec<serde_json::Value>>()
                .await
                .ok()
        };
        let (indexers, tags) = tokio::join!(list("/api/v1/indexer"), list("/api/v1/tag"));
        let solver = tags?
            .iter()
            .find(|tag| tag["label"] == "solver")
            .and_then(|tag| tag["id"].as_u64());
        let mut tiers = Tiers {
            fast: Vec::new(),
            slow: Vec::new(),
        };
        for indexer in indexers?.iter().filter(|indexer| indexer["enable"] == true) {
            let id = indexer["id"].as_u64()?;
            let behind_solver = solver.is_some_and(|solver| {
                indexer["tags"]
                    .as_array()
                    .is_some_and(|tags| tags.iter().any(|tag| tag.as_u64() == Some(solver)))
            });
            if behind_solver {
                &mut tiers.slow
            } else {
                &mut tiers.fast
            }
            .push(id);
        }
        let tiers = Arc::new(tiers);
        *self.tiers.lock().unwrap() = Some((Instant::now(), tiers.clone()));
        Some(tiers)
    }

    async fn ask(
        &self,
        text: &str,
        indexers: Option<&[u64]>,
        time: Duration,
    ) -> anyhow::Result<Vec<Row>> {
        let mut query = vec![
            ("query".to_string(), text.to_string()),
            ("type".to_string(), "search".to_string()),
            ("limit".to_string(), "100".to_string()),
        ];
        query.extend(
            indexers
                .unwrap_or_default()
                .iter()
                .map(|id| ("indexerIds".to_string(), id.to_string())),
        );
        let response = self
            .client
            .get(format!("{}/api/v1/search", self.base))
            .header("X-Api-Key", &self.key)
            .query(&query)
            .timeout(time)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    /// The torrent results that have a hash, or whose hash was found by `deadline`, best seeded
    /// first, and whether every link that was followed finished in time.
    async fn hits(self: &Arc<Self>, rows: Vec<Row>, deadline: Instant) -> (Vec<Hit>, bool) {
        let mut known = Vec::new();
        let mut without = Vec::new();
        for row in rows {
            if row.protocol.as_deref().is_some_and(|p| p != "torrent") {
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
                seeders: row.seeders.unwrap_or(0).clamp(0, u32::MAX as i64) as u32,
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
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let link = |name: &str| format!("http://nowhere.invalid:9696/1/download?apikey=x&link={name}");
                        Json(json!([
                            { "title": "Has hash", "infoHash": A, "seeders": 5, "size": 10, "indexer": "one", "protocol": "torrent" },
                            { "title": "Magnet only", "magnetUrl": format!("magnet:?xt=urn:btih:{B}&dn=x"), "seeders": 2, "indexer": "two", "protocol": "torrent" },
                            { "title": "Link only", "downloadUrl": link("good"), "seeders": 9, "indexer": "three", "protocol": "torrent" },
                            { "title": "Dead link", "downloadUrl": link("dead"), "seeders": 1, "indexer": "three", "protocol": "torrent" },
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

        let hits = fallback.search("Show", None, None).await.unwrap();

        let by_title: HashMap<&str, &str> = hits
            .iter()
            .map(|hit| (hit.title.as_str(), hit.hash.as_str()))
            .collect();
        assert_eq!(by_title["Has hash"], A);
        assert_eq!(by_title["Magnet only"], B);
        // Its link names another host, and was followed at Prowlarr's own address all the same.
        assert_eq!(by_title["Link only"], C);
        assert!(!by_title.contains_key("Dead link"));
        assert!(!by_title.contains_key("Usenet"));
        // Best seeded first, and nothing handed out carries the key.
        assert_eq!(hits[0].title, "Link only");
        assert!(!serde_json::to_string(&hits).unwrap().contains("apikey"));
        assert_eq!(seen.links.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_same_search_twice_asks_prowlarr_once() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        fallback.search("Show", None, None).await.unwrap();
        assert!(fallback.cached("SHOW", None, None).is_some());
        fallback.search("SHOW", None, None).await.unwrap();
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);

        // A different title is a different search, but the links already followed are remembered.
        fallback.search("Other", None, None).await.unwrap();
        assert_eq!(seen.searches.load(Ordering::SeqCst), 2);
        assert_eq!(seen.links.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_same_search_asked_at_once_runs_once() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        let asks = (0..4).map(|_| fallback.search("Show", None, None));
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
            tokio::spawn(async move { fallback.search("Show", None, None).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        asker.abort();

        // It finishes by itself, and the next one to ask has the answer at once.
        for _ in 0..50 {
            if fallback.cached("Show", None, None).is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(fallback.cached("Show", None, None).is_some());
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_episode_is_searched_the_way_sites_name_it() {
        let (base, seen) = prowlarr().await;
        let fallback = fallback(&base);

        fallback.search("Show", Some(1), Some(5)).await.unwrap();
        assert_eq!(*seen.last_query.lock().unwrap(), "Show S01E05");

        fallback.search("Show", None, Some(5)).await.unwrap();
        assert_eq!(*seen.last_query.lock().unwrap(), "Show 05");
    }

    #[tokio::test]
    async fn a_wrong_key_or_a_dead_prowlarr_is_a_failure_and_never_a_hang() {
        let (base, _) = prowlarr().await;
        let wrong = Arc::new(Fallback::new(&base, "not the key").unwrap());
        assert_eq!(
            wrong.search("Show", None, None).await.unwrap_err(),
            Failure::Failed
        );

        let nobody = fallback("http://127.0.0.1:1");
        assert_eq!(
            nobody.search("Show", None, None).await.unwrap_err(),
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
            .hits(rows, Instant::now() + Duration::from_millis(200))
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

    /// A Prowlarr with a fast indexer (1) and one behind the solver (2, tagged), whose search takes
    /// `slow` to answer. A search of both at once answers with both.
    async fn tiered_prowlarr(slow: Duration) -> String {
        use axum::extract::RawQuery;
        let app = Router::new()
            .route(
                "/api/v1/indexer",
                get(|| async {
                    Json(json!([
                        { "id": 1, "enable": true, "tags": [] },
                        { "id": 2, "enable": true, "tags": [7] },
                        { "id": 3, "enable": false, "tags": [] }
                    ]))
                }),
            )
            .route(
                "/api/v1/tag",
                get(|| async { Json(json!([{ "id": 7, "label": "solver" }])) }),
            )
            .route(
                "/api/v1/search",
                get(move |RawQuery(raw): RawQuery| async move {
                    let raw = raw.unwrap_or_default();
                    let fast = json!({ "title": "Fast one", "infoHash": A, "seeders": 5, "protocol": "torrent" });
                    let slow_row = json!({ "title": "Slow one", "infoHash": C, "seeders": 9, "protocol": "torrent" });
                    if raw.contains("indexerIds=2") {
                        tokio::time::sleep(slow).await;
                        Json(json!([slow_row]))
                    } else if raw.contains("indexerIds=1") {
                        Json(json!([fast]))
                    } else {
                        Json(json!([fast, slow_row]))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    fn quick(base: &str) -> Fallback {
        let mut fallback = Fallback::new(base, KEY).unwrap();
        fallback.fast_search = Duration::from_millis(500);
        fallback.slow_search = Duration::from_millis(500);
        fallback.total = Duration::from_millis(1000);
        fallback
    }

    #[tokio::test]
    async fn the_sites_behind_the_solver_are_searched_apart_so_a_slow_one_costs_only_itself() {
        // The solver's sites take far longer than they are given.
        let base = tiered_prowlarr(Duration::from_secs(5)).await;
        let fallback = Arc::new(quick(&base));

        let started = Instant::now();
        let hits = fallback.search("Show", None, None).await.unwrap();

        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Fast one");
        // What is missing a tier is not kept, so the next search tries the slow one again.
        assert!(fallback.cached("Show", None, None).is_none());
    }

    #[tokio::test]
    async fn both_tiers_are_merged_and_kept_when_both_answer() {
        let base = tiered_prowlarr(Duration::from_millis(50)).await;
        let fallback = Arc::new(quick(&base));

        let hits = fallback.search("Show", None, None).await.unwrap();

        let titles: Vec<&str> = hits.iter().map(|hit| hit.title.as_str()).collect();
        assert_eq!(titles, ["Slow one", "Fast one"], "best seeded first");
        assert!(fallback.cached("Show", None, None).is_some());
    }

    #[tokio::test]
    async fn without_a_list_of_indexers_everything_is_searched_at_once() {
        // The first fake has no list of indexers, so the tiers cannot be told apart.
        let (base, seen) = prowlarr().await;
        let fallback = Arc::new(quick(&base));

        let hits = fallback.search("Show", None, None).await.unwrap();

        assert_eq!(hits.len(), 3);
        assert_eq!(seen.searches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn it_is_off_without_an_address_and_a_key() {
        assert!(Fallback::new("", KEY).is_none());
        assert!(Fallback::new("http://prowlarr:9696", "").is_none());
        assert!(Fallback::new("http://prowlarr:9696/", KEY).is_some());
    }
}
