//! Torrent indexers.
//!
//! What each indexer is and how it is asked is data (`sources::IndexerDef`); this module is
//! the one engine that follows those definitions. A failing indexer reduces the number of
//! results and never fails the request, so `search_all` logs an error and moves on.

mod extract;

use crate::cache::{cached, list_ttl};
use crate::config::Config;
use crate::definition::{Fanout, IndexerDef, Method};
use crate::http::client;
use crate::model::{Kind, Release};
use crate::sources;
use crate::template::Fill;
use regex::Regex;
use serde_json::Value;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a non-empty answer is reused. An empty one lives shorter, see `cache::list_ttl`.
const TTL: Duration = Duration::from_secs(10 * 60);

/// The most an indexer's answer may weigh. A real one is well under a megabyte.
const MAX_RESPONSE_BYTES: usize = 8 << 20;

/// A search fans out over every (indexer, term) pair, which is 24 requests for one
/// episode. The cap is deliberately the full fan-out: measured, a low cap costs real
/// time because a 45ms request ends up queued behind a 1.5s one, and the per-indexer
/// timeout is what actually bounds the tail. The cap stays as a guard against a
/// definition set with many more indexers or terms than the default.
const MAX_CONCURRENT_REQUESTS: usize = 24;

/// How long a search keeps listening once it has enough to play. The first answers are rarely the
/// only good ones, so a short wait lets the quick indexers add theirs, and no longer: what is still
/// out is added to the cache for the next search, not waited for.
const GRACE: Duration = Duration::from_millis(700);

/// How long a mirror gets before the next one is asked as well. The first to answer is used. A dead
/// mirror then costs this and not its whole timeout.
const HEDGE: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub kind: Kind,
    pub imdb_id: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// The indexers that serve this kind, in the order the definitions list them.
fn serving(indexers: &[IndexerDef], kind: Kind) -> Vec<&IndexerDef> {
    indexers
        .iter()
        .filter(|indexer| indexer.kinds.contains(&kind))
        .collect()
}

/// The show title without the episode or season token: "Show 001" -> "Show",
/// "Show S01" -> "Show".
///
/// Only a token that actually looks like an episode number or a season is stripped.
/// Removing the last word unconditionally ate a real word from any term that had no
/// episode number on the end, so "Attack on Titan" was searched as "Attack on".
pub fn title_of(term: &str) -> String {
    static TOKEN: OnceLock<Regex> = OnceLock::new();
    let token = TOKEN.get_or_init(|| {
        Regex::new(r"\s+(?:S[0-9]{1,2}E[0-9]{1,4}|S[0-9]{1,2}|[0-9]{1,4})$").unwrap()
    });
    let title = token.replace(term, "").trim().to_string();
    if title.is_empty() {
        term.to_string()
    } else {
        title
    }
}

/// Which (indexer, term) requests a search makes.
fn plan<'a>(indexers: &[&'a IndexerDef], terms: &[String]) -> Vec<(&'a IndexerDef, String)> {
    let queries: Vec<&String> = terms.iter().filter(|term| !term.is_empty()).collect();
    let Some(first) = queries.first() else {
        return Vec::new();
    };

    let mut pairs = Vec::new();
    for &indexer in indexers {
        match indexer.fanout {
            Fanout::Once => pairs.push((indexer, (*first).clone())),
            Fanout::PerTitle => {
                let mut seen = HashSet::new();
                for term in &queries {
                    let title = title_of(term);
                    if seen.insert(title.clone()) {
                        pairs.push((indexer, title));
                    }
                }
            }
            Fanout::PerTerm => pairs.extend(queries.iter().map(|term| (indexer, (*term).clone()))),
        }
    }
    pairs
}

/// Whether a definition's request can be filled in for every kind it serves. Catches a
/// misspelt hole before the definition is used, where it would only look like an indexer
/// that never finds anything.
pub fn check(def: &IndexerDef) -> anyhow::Result<()> {
    for &kind in &def.kinds {
        let sample = SearchOptions {
            kind,
            imdb_id: Some("tt1234567".into()),
            season: Some(1),
            episode: Some(1),
        };
        anyhow::ensure!(
            Request::new(def, "sample", &sample).is_some(),
            "its request cannot be filled in for {}",
            kind.as_str()
        );
    }
    Ok(())
}

/// What a request is made of, with the search filled in.
struct Request {
    kind: Kind,
    path: String,
    query: Vec<(String, String)>,
    json: Option<Value>,
}

impl Request {
    /// The request for one search, or `None` when the search lacks something the request
    /// needs (an indexer that works by IMDb id has nothing to ask without one).
    fn new(def: &IndexerDef, term: &str, options: &SearchOptions) -> Option<Request> {
        let imdb = imdb(options);
        let hole = |name: &str| match name {
            "term" => Some(term.to_string()).filter(|term| !term.is_empty()),
            "imdb" => imdb.clone(),
            "imdb_digits" => imdb.as_ref().map(|id| id[2..].to_string()),
            // For an indexer that can look a title up by its id and otherwise by its name.
            "imdb_or_term" => imdb
                .clone()
                .or_else(|| Some(term.to_string()).filter(|term| !term.is_empty())),
            "season" => Some(options.season.unwrap_or(1).to_string()),
            "episode" => Some(options.episode.unwrap_or(1).to_string()),
            _ => None,
        };
        let fill = Fill {
            kind: options.kind,
            sort: None,
            hole: &hole,
        };
        let path = match fill.value(&def.request.path)? {
            Value::String(path) => path,
            _ => return None,
        };
        let query = def
            .request
            .query
            .iter()
            .map(|(name, value)| match fill.value(value)? {
                Value::String(text) => Some((name.clone(), text)),
                other => Some((name.clone(), other.to_string())),
            })
            .collect::<Option<_>>()?;
        let json = match &def.request.json {
            Some(body) => Some(fill.value(body)?),
            None => None,
        };
        Some(Request {
            kind: options.kind,
            path,
            query,
            json,
        })
    }

    /// Identifies the request, so an identical one is answered from the cache.
    fn key(&self, def: &IndexerDef) -> String {
        format!("{}:{}:{:?}:{:?}", def.id, self.path, self.query, self.json)
    }
}

/// The IMDb id (`{imdb}`: "tt0903747"), lowercase, or `None` for anything that is not one. A
/// malformed id is not sent: some indexers answer it with their whole index.
fn imdb(options: &SearchOptions) -> Option<String> {
    static ID: OnceLock<Regex> = OnceLock::new();
    let id = ID.get_or_init(|| Regex::new(r"^tt[0-9]{5,}$").unwrap());
    let value = options.imdb_id.as_deref()?.trim().to_lowercase();
    id.is_match(&value).then_some(value)
}

/// A host that just failed is not asked again for every search term: a search fans out over
/// several terms, and a blocked mirror would otherwise be hit once per term.
fn down_until() -> &'static Mutex<HashMap<String, Instant>> {
    static DOWN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    DOWN.get_or_init(Default::default)
}

fn health_file() -> &'static Mutex<Option<PathBuf>> {
    static FILE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    FILE.get_or_init(Default::default)
}

/// Remembers which hosts are resting in `dir`, across runs. Without it every launch starts by asking
/// each dead mirror again and waiting for it, which is the slowest part of a first search.
pub fn remember_host_health_in(dir: &std::path::Path) {
    let file = dir.join("indexer-health.json");
    if let Ok(text) = std::fs::read_to_string(&file)
        && let Ok(saved) = serde_json::from_str::<HashMap<String, u64>>(&text)
    {
        let now = unix_now();
        let mut down = down_until().lock().unwrap();
        for (host, until) in saved {
            if until > now {
                down.insert(host, Instant::now() + Duration::from_secs(until - now));
            }
        }
    }
    *health_file().lock().unwrap() = Some(file);
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Rests `host` for `pause`, and saves what is resting.
fn mark_down(host: &str, pause: Duration) {
    let now = Instant::now();
    let mut down = down_until().lock().unwrap();
    down.insert(host.to_string(), now + pause);
    down.retain(|_, until| *until > now);
    let Some(file) = health_file().lock().unwrap().clone() else {
        return;
    };
    let wall = unix_now();
    let saved: HashMap<&String, u64> = down
        .iter()
        .map(|(host, until)| (host, wall + until.duration_since(now).as_secs()))
        .collect();
    if let Ok(text) = serde_json::to_string(&saved) {
        let _ = std::fs::write(file, text);
    }
}

fn usable(host: &str) -> bool {
    down_until()
        .lock()
        .unwrap()
        .get(host)
        .is_none_or(|until| *until <= Instant::now())
}

async fn ask(
    def: &IndexerDef,
    host: &str,
    request: &Request,
    term: &str,
    trackers: &[String],
    limit: Duration,
) -> anyhow::Result<Vec<Release>> {
    let url = format!("{host}{}", request.path);
    let builder = match def.request.method {
        Method::Get => client().get(url),
        Method::Post => client().post(url),
    };
    let mut builder = builder.query(&request.query);
    if let Some(body) = &request.json {
        builder = builder.json(body);
    }
    // The limit covers the whole exchange, so a mirror that hangs costs one limit and the
    // next mirror is still tried.
    let body = tokio::time::timeout(limit, async {
        let response = builder.send().await?.error_for_status()?;
        crate::http::read_capped(response, MAX_RESPONSE_BYTES).await
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out after {}ms", limit.as_millis()))??;
    extract::releases(
        def,
        &String::from_utf8_lossy(&body),
        request.kind,
        term,
        trackers,
    )
}

/// One indexer asked about one term. The hosts are tried in order and the first that answers
/// is used, even with no rows: a valid answer of nothing is a real result.
async fn search_one(
    def: &IndexerDef,
    term: &str,
    options: &SearchOptions,
    trackers: &[String],
    limit: Duration,
) -> anyhow::Result<Vec<Release>> {
    let Some(request) = Request::new(def, term, options) else {
        return Ok(Vec::new());
    };
    let hosts: Vec<&String> = def.hosts.iter().filter(|host| usable(host)).collect();
    // With every host cooling down there is nothing to try. The empty answer is not cached:
    // that would blackhole this term for the full TTL although the host may recover in seconds.
    if hosts.is_empty() {
        return Ok(Vec::new());
    }

    // The definitions' version is part of the key, so an update is not answered from rows
    // read with the old one.
    let key = format!("v{}:{}", sources::current().version, request.key(def));
    cached(&key, list_ttl(TTL), || {
        ask_hosts(def, hosts, &request, term, trackers, limit)
    })
    .await
}

/// `ask`, with the host that answered named, so several can be in flight at once.
async fn ask_named<'a>(
    def: &IndexerDef,
    host: &'a String,
    request: &Request,
    term: &str,
    trackers: &[String],
    limit: Duration,
) -> (&'a String, anyhow::Result<Vec<Release>>) {
    (host, ask(def, host, request, term, trackers, limit).await)
}

/// Asks the hosts in order, the next one as soon as the one before fails and also when it has
/// not answered within `HEDGE`. The first answer wins and the others are dropped. A host that
/// failed rests for the definition's cooldown; one that lost to a later mirror rests as well, so
/// the next search does not wait on it again.
async fn ask_hosts(
    def: &IndexerDef,
    hosts: Vec<&String>,
    request: &Request,
    term: &str,
    trackers: &[String],
    limit: Duration,
) -> anyhow::Result<Vec<Release>> {
    let pause = Duration::from_secs(def.cooldown_secs);
    let mut pending = FuturesUnordered::new();
    let mut started: Vec<&String> = Vec::new();
    let mut last = None;
    loop {
        if pending.is_empty() && started.len() < hosts.len() {
            let host = hosts[started.len()];
            started.push(host);
            pending.push(ask_named(def, host, request, term, trackers, limit));
        }
        if pending.is_empty() {
            break;
        }
        let more = started.len() < hosts.len();
        tokio::select! {
            Some((host, result)) = pending.next() => match result {
                Ok(rows) => {
                    // The mirrors still out were slower than this one: rest them.
                    for other in started
                        .iter()
                        .filter(|other| ***other != *host && usable(other))
                    {
                        mark_down(other, pause);
                    }
                    return Ok(rows);
                }
                Err(error) => {
                    mark_down(host, pause);
                    tracing::debug!("[indexer:{}] {host}: {error:#}", def.id);
                    last = Some(error);
                }
            },
            _ = tokio::time::sleep(HEDGE), if more => {
                let host = hosts[started.len()];
                started.push(host);
                pending.push(ask_named(def, host, request, term, trackers, limit));
            }
        }
    }
    Err(last.expect("there was at least one host"))
}

/// One request's answer, as it reaches the search.
struct Arrival {
    index: usize,
    indexer: String,
    took: Duration,
    answered: bool,
    rows: Vec<Release>,
}

/// The releases answered so far, in the order of the requests and not of the answers, so what
/// "first seen wins" keeps does not depend on who was fastest.
fn merged(answers: &[Option<Vec<Release>>]) -> Vec<Release> {
    let mut seen = HashSet::new();
    answers
        .iter()
        .flatten()
        .flatten()
        .filter(|release| seen.insert(release.info_hash.clone()))
        .cloned()
        .collect()
}

/// Asks every indexer and returns what has come back when one of three things happens: all have
/// answered; `enough` says the answers so far hold something to play and `GRACE` has passed since;
/// or `search_ceiling_ms` has run out. The requests still out are not cancelled: they finish in the
/// background, fill the cache, and give the next search their rows.
///
/// `enough` is given the answers so far, deduplicated, and says whether they hold a release worth
/// opening. The caller knows what the ranking would keep, which is not the same as having rows.
pub async fn search_all(
    config: &Config,
    terms: &[String],
    options: &SearchOptions,
    enough: impl Fn(&[Release]) -> bool,
) -> Vec<Release> {
    let started = Instant::now();
    let sources = sources::current();
    let chosen = serving(&sources.indexers, options.kind);
    let pairs = plan(&chosen, terms);
    let total = pairs.len();
    // A single slow indexer must not hold the whole search open. Every normal request is
    // well under a second; the limit only bounds the tail, per host.
    let limit = Duration::from_millis(config.indexer_timeout_ms);
    let ceiling = started + Duration::from_millis(config.search_ceiling_ms);

    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let (send, mut receive) = tokio::sync::mpsc::unbounded_channel::<Arrival>();
    for (index, (indexer, term)) in pairs.into_iter().enumerate() {
        let (def, term, options) = (indexer.clone(), term, options.clone());
        let (sources, slots, send) = (sources.clone(), slots.clone(), send.clone());
        tokio::spawn(async move {
            let Ok(_slot) = slots.acquire_owned().await else {
                return;
            };
            let began = Instant::now();
            let result = search_one(&def, &term, &options, &sources.trackers, limit).await;
            if let Err(error) = &result {
                tracing::warn!("[indexer:{}] {error:#}", def.id);
            }
            let answered = result.is_ok();
            let _ = send.send(Arrival {
                index,
                indexer: def.id.clone(),
                took: began.elapsed(),
                answered,
                rows: result.unwrap_or_default(),
            });
        });
    }
    drop(send);

    let mut answers: Vec<Option<Vec<Release>>> = vec![None; total];
    let mut arrived = 0;
    let mut first_enough: Option<Duration> = None;
    let mut grace_ends: Option<Instant> = None;
    // Per indexer: requests answered, requests failed, rows, slowest answer.
    let mut each: Vec<(String, usize, usize, usize, Duration)> = Vec::new();
    let stopped = loop {
        let until = grace_ends.map_or(ceiling, |grace| grace.min(ceiling));
        match tokio::time::timeout_at(until.into(), receive.recv()).await {
            Ok(Some(arrival)) => {
                arrived += 1;
                let position = match each.iter().position(|row| row.0 == arrival.indexer) {
                    Some(position) => position,
                    None => {
                        each.push((arrival.indexer.clone(), 0, 0, 0, Duration::ZERO));
                        each.len() - 1
                    }
                };
                let row = &mut each[position];
                if arrival.answered {
                    row.1 += 1;
                } else {
                    row.2 += 1;
                }
                row.3 += arrival.rows.len();
                row.4 = row.4.max(arrival.took);
                answers[arrival.index] = Some(arrival.rows);
                if grace_ends.is_none() && enough(&merged(&answers)) {
                    first_enough = Some(started.elapsed());
                    grace_ends = Some(Instant::now() + GRACE);
                }
            }
            Ok(None) => break "every request answered",
            Err(_) if grace_ends.is_some_and(|grace| grace <= ceiling) => {
                break "enough, after the grace";
            }
            Err(_) => break "the ceiling",
        }
    };
    let found = merged(&answers);
    each.sort_by_key(|row| std::cmp::Reverse(row.4));
    tracing::info!(
        "[search] {} for {:?}: {arrived} of {total} requests answered in {:?}, stopped on {stopped}; \
         first playable after {}; {} releases; per indexer (requests, failed, rows, slowest): {}",
        options.kind.as_str(),
        terms.first().map(String::as_str).unwrap_or(""),
        started.elapsed(),
        first_enough.map_or("never".to_string(), |took| format!("{took:?}")),
        found.len(),
        each.iter()
            .map(|(id, ok, failed, rows, slowest)| format!(
                "{id} ({}, {failed}, {rows}, {slowest:?})",
                ok + failed
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn options(kind: Kind) -> SearchOptions {
        SearchOptions {
            kind,
            imdb_id: Some("tt0903747".into()),
            season: Some(1),
            episode: Some(5),
        }
    }

    fn definitions() -> std::sync::Arc<sources::Sources> {
        sources::current()
    }

    /// The ids the plan would ask, and the term each is asked.
    fn planned(kind: Kind, terms: &[&str], only: &[&str]) -> Vec<(String, String)> {
        let sources = definitions();
        let chosen: Vec<&IndexerDef> = serving(&sources.indexers, kind)
            .into_iter()
            .filter(|indexer| only.is_empty() || only.contains(&indexer.id.as_str()))
            .collect();
        plan(&chosen, &strings(terms))
            .into_iter()
            .map(|(indexer, term)| (indexer.id.clone(), term))
            .collect()
    }

    fn pair(id: &str, term: &str) -> (String, String) {
        (id.to_string(), term.to_string())
    }

    #[test]
    fn title_of_strips_only_an_episode_number_token() {
        // Removing the last word unconditionally ate a real word from any term with no
        // episode number on the end: "Attack on Titan" was searched as "Attack on".
        assert_eq!(title_of("BOCCHI THE ROCK 1"), "BOCCHI THE ROCK");
        assert_eq!(title_of("BOCCHI THE ROCK 001"), "BOCCHI THE ROCK");
        assert_eq!(title_of("BOCCHI THE ROCK S01E01"), "BOCCHI THE ROCK");
        assert_eq!(title_of("BOCCHI THE ROCK S01"), "BOCCHI THE ROCK");
        assert_eq!(title_of("ONE PIECE 001"), "ONE PIECE");
        assert_eq!(title_of("Attack on Titan"), "Attack on Titan");
        assert_eq!(title_of("Shingeki no Kyojin"), "Shingeki no Kyojin");
        assert_eq!(title_of("進撃の巨人"), "進撃の巨人");
        // A term that is only a number keeps itself.
        assert_eq!(title_of("1917"), "1917");
    }

    #[test]
    fn a_title_only_indexer_is_asked_once_per_distinct_title() {
        let terms = [
            "Attack on Titan 1",
            "Attack on Titan 01",
            "Shingeki no Kyojin 1",
        ];
        assert_eq!(
            planned(Kind::Anime, &terms, &["subsplease"]),
            [
                pair("subsplease", "Attack on Titan"),
                pair("subsplease", "Shingeki no Kyojin")
            ]
        );
    }

    #[test]
    fn an_id_indexer_is_asked_once_and_the_rest_once_per_term() {
        let terms = ["Breaking Bad S01E05", "Breaking Bad 105"];
        assert_eq!(
            planned(Kind::Series, &terms, &["torrentio", "knaben", "eztv"]),
            [
                pair("knaben", "Breaking Bad S01E05"),
                pair("knaben", "Breaking Bad 105"),
                pair("torrentio", "Breaking Bad S01E05"),
                pair("eztv", "Breaking Bad"),
            ]
        );
    }

    #[test]
    fn only_indexers_that_serve_the_kind_are_asked() {
        let names = |kind| -> Vec<String> {
            let mut ids: Vec<String> = Vec::new();
            for (id, _) in planned(kind, &["Dhamaal 4 2024"], &[]) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            ids
        };
        assert_eq!(
            names(Kind::Anime),
            [
                "subsplease",
                "nyaa",
                "animetosho",
                "tokyotosho",
                "knaben",
                "torrents-csv",
                "apibay"
            ]
        );
        assert_eq!(
            names(Kind::Series),
            ["knaben", "torrents-csv", "torrentio", "eztv", "apibay"]
        );
        assert_eq!(
            names(Kind::Movie),
            ["knaben", "torrents-csv", "torrentio", "yts", "apibay"]
        );
    }

    #[test]
    fn nothing_is_asked_for_empty_terms() {
        assert!(planned(Kind::Anime, &["", ""], &[]).is_empty());
        assert!(planned(Kind::Anime, &[], &[]).is_empty());
    }

    fn request(id: &str, term: &str, options: &SearchOptions) -> Option<Request> {
        Request::new(definitions().indexer(id).unwrap(), term, options)
    }

    #[test]
    fn a_request_is_filled_in_from_the_search() {
        let eztv = request("eztv", "ignored", &options(Kind::Series)).unwrap();
        // EZTV wants the bare number: with "tt" it answers with its whole index.
        assert_eq!(eztv.path, "/get-torrents");
        assert!(
            eztv.query
                .contains(&("imdb_id".to_string(), "0903747".to_string()))
        );

        let series = request("torrentio", "", &options(Kind::Series)).unwrap();
        assert_eq!(series.path, "/stream/series/tt0903747:1:5.json");
        let film = request("torrentio", "", &options(Kind::Movie)).unwrap();
        assert_eq!(film.path, "/stream/movie/tt0903747.json");

        let knaben = |kind| {
            request("knaben", "bad", &options(kind))
                .unwrap()
                .json
                .unwrap()
        };
        assert_eq!(knaben(Kind::Anime)["categories"], json!([6_000_000]));
        assert_eq!(
            knaben(Kind::Movie)["categories"],
            json!([3_000_000, 2_000_000])
        );
        assert_eq!(knaben(Kind::Series)["query"], "bad");
    }

    #[test]
    fn a_film_is_looked_up_by_its_id_when_there_is_one_and_by_name_otherwise() {
        let query = |imdb: Option<&str>| {
            let mut asked = options(Kind::Movie);
            asked.imdb_id = imdb.map(str::to_string);
            request("yts", "Inception 2010", &asked)
                .unwrap()
                .query
                .into_iter()
                .find(|(name, _)| name == "query_term")
                .unwrap()
                .1
        };
        assert_eq!(query(Some("tt1375666")), "tt1375666");
        assert_eq!(query(None), "Inception 2010");
        // An id that is not one is not sent in place of the name.
        assert_eq!(query(Some("nonsense")), "Inception 2010");
    }

    #[test]
    fn a_request_that_lacks_what_it_needs_is_not_made() {
        let mut no_id = options(Kind::Series);
        no_id.imdb_id = None;
        assert!(request("eztv", "x", &no_id).is_none());
        assert!(request("torrentio", "x", &no_id).is_none());
        // A malformed id is not sent: some indexers answer it with everything they have.
        no_id.imdb_id = Some("0903747".into());
        assert!(request("eztv", "x", &no_id).is_none());
        assert!(request("nyaa", "", &options(Kind::Anime)).is_none());
        assert!(request("nyaa", "one piece", &options(Kind::Anime)).is_some());
    }

    #[test]
    fn a_missing_season_and_episode_ask_for_the_first() {
        let mut unknown = options(Kind::Series);
        unknown.season = None;
        unknown.episode = None;
        assert_eq!(
            request("torrentio", "", &unknown).unwrap().path,
            "/stream/series/tt0903747:1:1.json"
        );
    }

    #[test]
    fn a_definition_with_a_misspelt_hole_is_refused() {
        let mut broken = definitions().indexer("nyaa").unwrap().clone();
        broken.request.query.insert("q".into(), json!("{trem}"));
        assert!(check(&broken).is_err());
        assert!(check(definitions().indexer("nyaa").unwrap()).is_ok());
    }

    /// A local server standing in for indexers: one host that is blocked, one that is a
    /// block page, one that answers.
    async fn stub() -> String {
        use axum::{Router, http::StatusCode, routing::get};
        let app = Router::new()
            .route("/down", get(|| async { StatusCode::SERVICE_UNAVAILABLE }))
            .route("/page", get(|| async { "<html>blocked</html>" }))
            .route(
                "/hang",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    ""
                }),
            )
            .route(
                "/rows",
                get(|| async {
                    r#"[{"name":"Show S01E01","info_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","seeders":"3"}]"#
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    const LIMIT: Duration = Duration::from_secs(5);

    fn indexer_at(id: &str, hosts: &[String]) -> IndexerDef {
        serde_json::from_value(json!({
            "id": id, "name": id, "kinds": ["anime"], "hosts": hosts,
            "request": { "query": { "q": "{term}" } },
            "response": { "format": "json",
                "fields": { "title": "name", "hash": "info_hash", "seeders": "seeders" } }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn the_next_mirror_answers_when_one_is_blocked_and_the_blocked_one_rests() {
        let base = stub().await;
        let hosts = [
            format!("{base}/down"),
            format!("{base}/page"),
            format!("{base}/rows"),
        ];
        let def = indexer_at("mirrors-test", &hosts);
        let found = search_one(&def, "show", &options(Kind::Anime), &[], LIMIT)
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, 3);
        assert!(!usable(&hosts[0]), "a host that failed rests");
        assert!(
            !usable(&hosts[1]),
            "a block page is a failure, not an empty result"
        );
        assert!(usable(&hosts[2]));
    }

    #[tokio::test]
    async fn every_mirror_failing_is_an_error_and_resting_hosts_are_not_asked() {
        let base = stub().await;
        let hosts = [format!("{base}/down"), format!("{base}/page")];
        let def = indexer_at("all-down-test", &hosts);
        let error = search_one(&def, "show", &options(Kind::Anime), &[], LIMIT).await;
        assert!(error.is_err());
        // Both now rest, so the next term makes no request and caches nothing.
        let again = search_one(&def, "show two", &options(Kind::Anime), &[], LIMIT).await;
        assert_eq!(again.unwrap(), Vec::new());
    }

    #[tokio::test]
    async fn a_mirror_that_hangs_does_not_stop_the_next_one_being_tried() {
        let base = stub().await;
        let hosts = [format!("{base}/hang"), format!("{base}/rows")];
        let def = indexer_at("hang-test", &hosts);
        let quick = Duration::from_millis(300);
        let found = search_one(&def, "show", &options(Kind::Anime), &[], quick).await;
        assert_eq!(found.unwrap().len(), 1);
        assert!(!usable(&hosts[0]), "the hung mirror rests");
    }

    /// A dead mirror used to cost its whole timeout before the next was asked.
    #[tokio::test]
    async fn a_silent_mirror_costs_one_hedge_and_not_its_timeout() {
        let base = stub().await;
        let hosts = [format!("{base}/hang"), format!("{base}/rows")];
        let def = indexer_at("hedge-test", &hosts);
        let started = Instant::now();
        let found = search_one(&def, "show", &options(Kind::Anime), &[], LIMIT).await;
        let took = started.elapsed();
        assert_eq!(found.unwrap().len(), 1);
        assert!(
            took < Duration::from_millis(2500),
            "waited {took:?} on a mirror that never answers"
        );
        assert!(!usable(&hosts[0]), "the slow mirror rests for the next search");
        assert!(usable(&hosts[1]));
    }

    #[test]
    fn answers_are_merged_in_request_order_without_repeats() {
        let release = |hash: &str, title: &str| Release {
            title: title.into(),
            info_hash: hash.into(),
            magnet: String::new(),
            size_bytes: None,
            seeders: 1,
            source: "t".into(),
        };
        // The second request answered first; its rows still come after the first's.
        let answers = vec![
            Some(vec![release("a", "from first"), release("b", "from first")]),
            None,
            Some(vec![release("b", "from third"), release("c", "from third")]),
        ];
        let merged = merged(&answers);
        let titles: Vec<&str> = merged.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["from first", "from first", "from third"]);
    }

    #[test]
    fn resting_hosts_are_remembered_across_runs() {
        let dir = std::env::temp_dir().join(format!("akidev-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        remember_host_health_in(&dir);
        mark_down("http://remembered.test", Duration::from_secs(120));
        let saved = std::fs::read_to_string(dir.join("indexer-health.json")).unwrap();
        assert!(saved.contains("remembered.test"));

        // A new run: nothing is resting in memory, and the file says what was.
        down_until().lock().unwrap().remove("http://remembered.test");
        assert!(usable("http://remembered.test"));
        remember_host_health_in(&dir);
        assert!(!usable("http://remembered.test"));
        let _ = std::fs::remove_dir_all(&dir);
        *health_file().lock().unwrap() = None;
    }
}
