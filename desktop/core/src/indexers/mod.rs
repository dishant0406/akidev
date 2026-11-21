//! Torrent indexers.
//!
//! What each indexer is and how it is asked is data (`sources::IndexerDef`); this module is
//! the one engine that follows those definitions. A failing indexer reduces the number of
//! results and never fails the request, so `search_all` logs an error and moves on.

mod extract;

use crate::cache::{cached, list_ttl, peek};
use crate::config::Config;
use crate::definition::{Fanout, IndexerDef, Method};
use crate::http::client;
use crate::model::{Kind, Release};
use crate::sources;
use crate::template::Fill;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a non-empty answer is reused. An empty one lives shorter, see `cache::list_ttl`.
const TTL: Duration = Duration::from_secs(10 * 60);

/// The most an indexer's answer may weigh. A real one is well under a megabyte.
const MAX_RESPONSE_BYTES: usize = 8 << 20;

/// A search fans out over every (indexer, term) pair, which is 24 requests for one
/// episode. The shared cap bounds HTTP exchanges across searches and hedged mirrors;
/// permit waits are included in the per-host timeout.
const MAX_CONCURRENT_REQUESTS: usize = 24;
/// Bound both detached background work and each search's plan/results buffer.
const MAX_PLANNED_REQUESTS: usize = 96;
/// Bound retained rows as well as response bytes; snapshots never accumulate events.
const MAX_ROWS_PER_REQUEST: usize = 512;

fn request_slots() -> &'static tokio::sync::Semaphore {
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    SLOTS.get_or_init(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS))
}

fn task_slots() -> &'static Arc<tokio::sync::Semaphore> {
    static SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MAX_PLANNED_REQUESTS)))
}

type FlightResult = Result<QueryAnswer, RequestOutcome>;
type Flight = tokio::sync::watch::Sender<Option<Arc<FlightResult>>>;

/// Only a new producer consumes admission. Cache hits and caller-owned coalesced
/// waits never spawn detached work or compete with the 96 producer slots.
fn flight(key: &str) -> anyhow::Result<(Arc<Flight>, Option<tokio::sync::OwnedSemaphorePermit>)> {
    static FLIGHTS: OnceLock<Mutex<HashMap<String, std::sync::Weak<Flight>>>> = OnceLock::new();
    let mut flights = FLIGHTS.get_or_init(Default::default).lock().unwrap();
    flights.retain(|_, flight| flight.strong_count() > 0);
    if let Some(flight) = flights.get(key).and_then(std::sync::Weak::upgrade) {
        return Ok((flight, None));
    }
    let slot = task_slots()
        .clone()
        .try_acquire_owned()
        .map_err(|_| LocalCongestion)?;
    let (send, _) = tokio::sync::watch::channel(None);
    let flight = Arc::new(send);
    flights.insert(key.to_string(), Arc::downgrade(&flight));
    Ok((flight, Some(slot)))
}

/// How long a search keeps listening once it has enough to play. The first answers are rarely the
/// only good ones, so a short wait lets the quick indexers add theirs. Pending
/// answers fill the cache and remain observable through the caller-owned live tail.
const GRACE: Duration = Duration::from_millis(700);

/// How long a mirror gets before the next one is asked as well. The first to answer is used. A dead
/// mirror then costs this and not its whole timeout.
const HEDGE: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub kind: Kind,
    pub imdb_id: Option<String>,
    /// The id of the title or episode in another service, found through a mapper, for the indexers
    /// that are asked through it.
    pub mapped: Option<Mapped>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// An id found through a mapper (see `definition::MapperDef`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// The mapper that found it: `kitsu`.
    pub mapper: String,
    /// The kind of the title it was found for.
    pub kind: Kind,
    /// `kitsu:7442:1` for an episode, `kitsu:50192` for a film.
    pub id: String,
    /// A film is asked for on the film route and has no episode suffix.
    pub movie: bool,
}

/// The indexers that serve this kind, in the order the definitions list them. A `server_only` one
/// is left out unless this is the server.
fn serving(indexers: &[IndexerDef], kind: Kind, on_server: bool) -> Vec<&IndexerDef> {
    indexers
        .iter()
        .filter(|indexer| indexer.kinds.contains(&kind) && (on_server || !indexer.server_only))
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
    let queries: Vec<&String> = terms
        .iter()
        .filter(|term| {
            !term.is_empty() && term.len() <= 4096 && !term.chars().any(char::is_control)
        })
        .take(MAX_PLANNED_REQUESTS)
        .collect();
    let Some(first) = queries.first() else {
        return Vec::new();
    };

    let mut pairs = Vec::new();
    for &indexer in indexers.iter().take(MAX_PLANNED_REQUESTS) {
        match indexer.fanout {
            Fanout::Once => pairs.push((indexer, (*first).clone())),
            Fanout::PerTitle => {
                let mut seen = HashSet::new();
                for term in &queries {
                    let title = title_of(term);
                    if seen.insert(title.clone()) {
                        pairs.push((indexer, title));
                        if pairs.len() == MAX_PLANNED_REQUESTS {
                            return pairs;
                        }
                    }
                }
            }
            Fanout::PerTerm => pairs.extend(
                queries
                    .iter()
                    .take(MAX_PLANNED_REQUESTS - pairs.len())
                    .map(|term| (indexer, (*term).clone())),
            ),
        }
        if pairs.len() == MAX_PLANNED_REQUESTS {
            break;
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
            mapped: def.mapper_id().map(|mapper| Mapped {
                mapper: mapper.into(),
                kind,
                id: format!("{mapper}:1:1"),
                movie: false,
            }),
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
        let mapper = def.mapper_id();
        let hole = |name: &str| match name {
            "term" => Some(term.to_string()).filter(|term| !term.is_empty()),
            "imdb" => imdb.clone(),
            "imdb_digits" => imdb.as_ref().map(|id| id[2..].to_string()),
            name if mapper == Some(name) => mapped(def, options),
            name if mapper.is_some_and(|mapper| name.strip_suffix("_kind") == Some(mapper)) => {
                mapped_kind(def, options)
            }
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

    /// Identifies the request, so an identical one is answered from the cache. How the answer is read
    /// and the headers are part of it, so a plugin that is updated is not answered from what the old one found.
    fn key(&self, def: &IndexerDef) -> String {
        format!(
            "{}:{:?}:{:?}:{}:{:?}:{:?}:{:?}:{:?}",
            def.id,
            def.hosts,
            self.kind,
            self.path,
            self.query,
            self.json,
            def.request.headers,
            def.response
        )
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

/// A mapped id is valid only for the exact requested episode, so an alias cannot select a different
/// season or episode. A mapped film has no episode suffix. The ids look like `<mapper>:<title>:<episode>`.
fn mapped(def: &IndexerDef, options: &SearchOptions) -> Option<String> {
    let mapper = def.mapper_id()?;
    let found = options
        .mapped
        .as_ref()
        .filter(|found| found.mapper == mapper && found.kind == options.kind)?;
    let value = found.id.to_lowercase();
    let front = regex::escape(mapper);
    if found.movie {
        let id = Regex::new(&format!("^{front}:([1-9][0-9]*)$")).ok()?;
        let title = id.captures(&value)?[1].parse::<u32>().ok()?;
        return (options.episode == Some(1) && title > 0).then_some(value);
    }
    let id = Regex::new(&format!("^{front}:([1-9][0-9]*):([1-9][0-9]*)$")).ok()?;
    let found = id.captures(&value)?;
    (found[1].parse::<u32>().ok()? > 0 && found[2].parse::<u32>().ok()? == options.episode?)
        .then_some(value)
}

/// The route a mapped id is asked on: films have their own.
fn mapped_kind(def: &IndexerDef, options: &SearchOptions) -> Option<String> {
    mapped(def, options)?;
    let movie = options.mapped.as_ref()?.movie;
    Some(if movie { "movie" } else { "series" }.into())
}

/// A host that just failed is not asked again for every search term: a search fans out over
/// several terms, and a blocked mirror would otherwise be hit once per term.
#[derive(Default)]
struct HostHealth {
    down: HashMap<String, Instant>,
    file: Option<PathBuf>,
}

fn host_health() -> &'static Mutex<HostHealth> {
    static HEALTH: OnceLock<Mutex<HostHealth>> = OnceLock::new();
    HEALTH.get_or_init(Default::default)
}

/// Remembers which hosts are resting in `dir`, across runs. Without it every launch starts by asking
/// each dead mirror again and waiting for it, which is the slowest part of a first search.
pub fn remember_host_health_in(dir: &std::path::Path) {
    remember_health(&mut host_health().lock().unwrap(), dir);
}

fn remember_health(health: &mut HostHealth, dir: &std::path::Path) {
    let file = dir.join("indexer-health.json");
    if let Ok(text) = std::fs::read_to_string(&file)
        && let Ok(saved) = serde_json::from_str::<HashMap<String, u64>>(&text)
    {
        let now = unix_now();
        for (host, until) in saved {
            if until > now {
                health
                    .down
                    .insert(host, Instant::now() + Duration::from_secs(until - now));
            }
        }
    }
    health.file = Some(file);
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Rests `host` for `pause`, and saves what is resting.
fn mark_down(host: &str, pause: Duration) {
    rest_host(&mut host_health().lock().unwrap(), host, pause);
}

fn rest_host(health: &mut HostHealth, host: &str, pause: Duration) {
    let now = Instant::now();
    health.down.insert(host.to_string(), now + pause);
    health.down.retain(|_, until| *until > now);
    let Some(file) = &health.file else {
        return;
    };
    let wall = unix_now();
    let saved: HashMap<&String, u64> = health
        .down
        .iter()
        .map(|(host, until)| (host, wall + until.duration_since(now).as_secs()))
        .collect();
    if let Ok(text) = serde_json::to_string(&saved) {
        let _ = std::fs::write(file, text);
    }
}

fn usable(host: &str) -> bool {
    host_health()
        .lock()
        .unwrap()
        .down
        .get(host)
        .is_none_or(|until| *until <= Instant::now())
}

#[derive(Debug)]
struct LocalCongestion;

impl std::fmt::Display for LocalCongestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("provider locally busy")
    }
}

impl std::error::Error for LocalCongestion {}

async fn ask(
    def: &IndexerDef,
    host: &str,
    request: &Request,
    term: &str,
    trackers: &[String],
    limit: Duration,
    sent: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Vec<Release>> {
    let deadline = tokio::time::Instant::now() + limit;
    let _slot = tokio::time::timeout_at(deadline, request_slots().acquire())
        .await
        .map_err(|_| LocalCongestion)?
        .map_err(|_| LocalCongestion)?;
    sent.store(true, std::sync::atomic::Ordering::SeqCst);
    let body = if let Some(export) = &def.code {
        // The plugin's own code makes the request and answers with the JSON its `response` reads.
        tokio::time::timeout_at(deadline, code_body(def, export, host, request))
            .await
            .map_err(|_| RequestOutcome::Timeout)??
    } else {
        let url = format!("{host}{}", request.path);
        anyhow::ensure!(
            crate::http::stays_on(host, &url),
            "the request would leave its host"
        );
        let builder = match def.request.method {
            Method::Get => client().get(url),
            Method::Post => client().post(url),
        };
        let mut builder = builder.query(&request.query);
        for (name, value) in &def.request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.json {
            builder = builder.json(body);
        }
        // The limit covers the whole exchange, so a mirror that hangs costs one limit and the
        // next mirror is still tried.
        tokio::time::timeout_at(deadline, async {
            let response = builder.send().await?.error_for_status()?;
            crate::http::read_capped(response, MAX_RESPONSE_BYTES)
                .await
                .map_err(|error| {
                    if error.downcast_ref::<reqwest::Error>().is_some() {
                        error
                    } else {
                        RequestOutcome::BadResponse.into()
                    }
                })
        })
        .await
        .map_err(|_| RequestOutcome::Timeout)??
    };
    extract::releases(
        def,
        &String::from_utf8_lossy(&body),
        request.kind,
        term,
        trackers,
    )
    .map_err(|_| RequestOutcome::BadResponse.into())
}

/// What the plugin's code answers for this request: the host it is asked on, the filled path, query and
/// body as hints, and the term.
#[cfg(feature = "code-plugins")]
async fn code_body(
    def: &IndexerDef,
    export: &str,
    host: &str,
    request: &Request,
) -> anyhow::Result<Vec<u8>> {
    let module = def
        .module
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("its module is not loaded"))?;
    let hosts = def
        .hosts
        .iter()
        .filter_map(|each| Some(reqwest::Url::parse(each).ok()?.host_str()?.to_string()))
        .collect();
    let input = serde_json::json!({
        "host": host,
        "kind": request.kind.as_str(),
        "path": request.path,
        "query": request.query,
        "json": request.json,
    });
    // The tests run a server on this machine, which the public address rules rightly refuse.
    #[cfg(test)]
    let allowed: fn(&str) -> anyhow::Result<()> = |_| Ok(());
    #[cfg(not(test))]
    let allowed: fn(&str) -> anyhow::Result<()> = crate::plugin::check_public_url;
    let answer = crate::code::run_with(
        module.0.to_vec(),
        export,
        input,
        hosts,
        crate::code::Limits::default(),
        allowed,
    )
    .await?;
    Ok(serde_json::to_vec(&answer)?)
}

#[cfg(not(feature = "code-plugins"))]
async fn code_body(_: &IndexerDef, _: &str, _: &str, _: &Request) -> anyhow::Result<Vec<u8>> {
    anyhow::bail!("this build cannot run a plugin's code")
}

/// One indexer asked about one term. The hosts are tried in order and the first that answers
/// is used, even with no rows: a valid answer of nothing is a real result.
#[cfg(test)]
async fn search_one(
    def: &IndexerDef,
    term: &str,
    options: &SearchOptions,
    trackers: &[String],
    limit: Duration,
) -> anyhow::Result<Vec<Release>> {
    search_one_report(def, term, options, trackers, limit)
        .await
        .map(|answer| answer.rows)
        .map_err(anyhow::Error::new)
}

async fn search_one_report(
    def: &IndexerDef,
    term: &str,
    options: &SearchOptions,
    trackers: &[String],
    limit: Duration,
) -> FlightResult {
    // An anime-specific hole cannot fall through to a default IMDb route.
    let Some(request) = Request::new(def, term, options) else {
        return Ok(QueryAnswer::new(Vec::new(), RequestOutcome::Skipped));
    };
    let key = format!(
        "indexers-v4:{}:{}:{term:?}:{trackers:?}",
        sources::current().version,
        request.key(def)
    );
    if let Some(mut answer) = peek::<QueryAnswer>(&key) {
        answer.cache = CacheEvidence::Hit;
        return Ok(answer);
    }
    let (flight, slot) = flight(&key).map_err(|error| classify_error(&error))?;
    let mut receive = flight.subscribe();
    let coalesced = slot.is_none();
    if let Some(slot) = slot {
        let (def, term, trackers) = (def.clone(), term.to_string(), trackers.to_vec());
        let flight = flight.clone();
        tokio::spawn(async move {
            let _slot = slot;
            let loaded = std::sync::atomic::AtomicBool::new(false);
            let result = cached(
                &key,
                |answer: &QueryAnswer| list_ttl(TTL)(&answer.rows),
                || async {
                    loaded.store(true, std::sync::atomic::Ordering::Relaxed);
                    // Cooldown only guards network misses, never valid cached answers.
                    let hosts: Vec<&String> =
                        def.hosts.iter().filter(|host| usable(host)).collect();
                    if hosts.is_empty() {
                        return Err(RequestOutcome::Cooldown.into());
                    }
                    ask_hosts(&def, hosts, &request, &term, &trackers, limit)
                        .await
                        .map(|rows| {
                            let outcome = if rows.is_empty() {
                                RequestOutcome::Empty
                            } else {
                                RequestOutcome::Answered
                            };
                            QueryAnswer::new(rows, outcome)
                        })
                },
            )
            .await
            .map(|mut answer| {
                if !loaded.load(std::sync::atomic::Ordering::Relaxed) {
                    answer.cache = CacheEvidence::Hit;
                }
                answer
            })
            .map_err(|error| classify_error(&error));
            flight.send_replace(Some(Arc::new(result)));
        });
    }
    loop {
        if let Some(result) = receive.borrow().clone() {
            return result.as_ref().clone().map(|mut answer| {
                if coalesced {
                    answer.cache = CacheEvidence::Coalesced;
                }
                answer
            });
        }
        receive
            .changed()
            .await
            .map_err(|_| RequestOutcome::Unavailable)?;
    }
}

/// `ask`, with the host that answered named, so several can be in flight at once.
async fn ask_named<'a>(
    def: &IndexerDef,
    host: &'a String,
    request: &Request,
    term: &str,
    trackers: &[String],
    limit: Duration,
    sent: Arc<std::sync::atomic::AtomicBool>,
) -> (&'a String, anyhow::Result<Vec<Release>>) {
    (
        host,
        ask(def, host, request, term, trackers, limit, &sent).await,
    )
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
    let mut started: Vec<(&String, Arc<std::sync::atomic::AtomicBool>)> = Vec::new();
    let mut last = None;
    loop {
        if pending.is_empty() && started.len() < hosts.len() {
            let host = hosts[started.len()];
            let sent = Arc::new(std::sync::atomic::AtomicBool::new(false));
            started.push((host, sent.clone()));
            pending.push(ask_named(def, host, request, term, trackers, limit, sent));
        }
        if pending.is_empty() {
            break;
        }
        let more = started.len() < hosts.len();
        tokio::select! {
                            Some((host, result)) = pending.next() => match result {
                                Ok(rows) => {
                                    // The mirrors still out were slower than this one: rest them.
        for (other, sent) in &started {
                                        if *other != host && usable(other)
                                            && sent.load(std::sync::atomic::Ordering::SeqCst)
                                        {
                                            mark_down(other, pause);
                                        }
                                    }
                                    return Ok(rows);
                                }
                                Err(error) => {
        if error.downcast_ref::<LocalCongestion>().is_none() {
                                        mark_down(host, pause);
                                    }
                tracing::debug!(indexer = %def.id, "mirror request failed");
                                    last = Some(error);
                                }
                            },
                            _ = tokio::time::sleep(HEDGE), if more => {
                                let host = hosts[started.len()];
        let sent = Arc::new(std::sync::atomic::AtomicBool::new(false));
                                started.push((host, sent.clone()));
                                pending.push(ask_named(def, host, request, term, trackers, limit, sent));
                            }
                        }
    }
    Err(last.expect("there was at least one host"))
}

/// Bounded, sanitized classification; never retains an upstream error or address.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestOutcome {
    #[default]
    Pending,
    Answered,
    Empty,
    /// Rows arrived but the caller's usability predicate rejected this answer.
    Irrelevant,
    Skipped,
    Timeout,
    Http {
        status: u16,
    },
    BadResponse,
    LocalCongestion,
    Cooldown,
    Unavailable,
}

impl std::fmt::Display for RequestOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("provider request unavailable")
    }
}
impl std::error::Error for RequestOutcome {}

fn classify_error(error: &anyhow::Error) -> RequestOutcome {
    if let Some(outcome) = error.downcast_ref::<RequestOutcome>() {
        return *outcome;
    }
    if error.downcast_ref::<LocalCongestion>().is_some() {
        return RequestOutcome::LocalCongestion;
    }
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        if error.is_timeout() {
            return RequestOutcome::Timeout;
        }
        if let Some(status) = error.status() {
            return RequestOutcome::Http {
                status: status.as_u16(),
            };
        }
        if error.is_decode() || error.is_body() {
            return RequestOutcome::BadResponse;
        }
    }
    RequestOutcome::Unavailable
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEvidence {
    #[default]
    NotObserved,
    Fresh,
    Hit,
    Coalesced,
}

#[derive(Debug, Clone)]
struct QueryAnswer {
    rows: Vec<Release>,
    outcome: RequestOutcome,
    cache: CacheEvidence,
}
impl QueryAnswer {
    fn new(rows: Vec<Release>, outcome: RequestOutcome) -> Self {
        Self {
            rows,
            outcome,
            cache: if outcome == RequestOutcome::Skipped {
                CacheEvidence::NotObserved
            } else {
                CacheEvidence::Fresh
            },
        }
    }
}

/// One entry per planned provider query; ordinal is plan-local, never the query text.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RequestReport {
    pub query_ordinal: usize,
    pub outcome: RequestOutcome,
    pub elapsed_ms: u64,
    pub cache: CacheEvidence,
}

/// One request's answer, as it reaches the search.
struct Arrival {
    index: usize,
    indexer: String,
    took: Duration,
    answered: bool,
    skipped: bool,
    outcome: RequestOutcome,
    cache: CacheEvidence,
    rows: Vec<Release>,
}

/// The releases answered so far, in the order of the requests and not of the answers, so what
/// "first seen wins" keeps does not depend on who was fastest.
fn merged(answers: &[Option<Vec<Release>>]) -> Vec<Release> {
    let mut positions = HashMap::new();
    let mut found: Vec<Release> = Vec::new();
    for release in answers.iter().flatten().flatten() {
        let selection = release.selection_id();
        match positions.get(&selection).copied() {
            Some(at) => combine(&mut found[at], release),
            None => {
                positions.insert(selection, found.len());
                let mut row = release.clone();
                if row.sources.is_empty() && !row.source.is_empty() {
                    row.sources.push(row.source.clone());
                }
                found.push(row);
            }
        }
    }
    found
}

/// Keep only configured public trackers: an upstream magnet cannot add arbitrary
/// destinations, web seeds, credentials or passkeys to the engine's address list.
fn safe_magnet(hash: &str, title: &str, magnets: &[&str], allowed: &[String]) -> String {
    let mut trackers = Vec::new();
    for magnet in magnets {
        if !magnet.starts_with("magnet:?")
            || crate::magnet::info_hash_from_magnet(magnet).as_deref() != Some(hash)
        {
            continue;
        }
        if let Ok(url) = reqwest::Url::parse(magnet) {
            for (name, value) in url.query_pairs() {
                if trackers.len() == 32 {
                    break;
                }
                if name == "tr"
                    && allowed.iter().any(|tracker| tracker == value.as_ref())
                    && !trackers.contains(&value.to_string())
                {
                    trackers.push(value.to_string());
                    if trackers.len() == 32 {
                        break;
                    }
                }
            }
        }
    }
    for tracker in allowed {
        if trackers.len() == 32 {
            break;
        }
        if !trackers.contains(tracker) {
            trackers.push(tracker.clone());
        }
    }
    crate::magnet::build_magnet(hash, Some(title), &trackers)
}

fn extra_name(title: &str) -> bool {
    static EXTRA: OnceLock<Regex> = OnceLock::new();
    EXTRA.get_or_init(|| Regex::new(r"(?i)(?:^|[^a-z])(?:bonus|bouns|extras?|samples?|bloopers|deleted[ ._-]*scenes)(?:$|[^a-z])").unwrap()).is_match(title)
}

/// Combine evidence, never adding seed counts or voting on identity. Stable ties keep
/// the first request's authority. A clean usable name beats an extra-file variant.
fn combine(known: &mut Release, other: &Release) {
    if known.sources.is_empty() && !known.source.is_empty() {
        known.sources.push(known.source.clone());
    }
    for source in std::iter::once(&other.source).chain(other.sources.iter()) {
        if !source.is_empty() && !known.sources.contains(source) {
            known.sources.push(source.clone());
        }
    }
    known.seeders = known.seeders.max(other.seeders);
    if known.size_bytes.is_none_or(|size| size == 0) {
        known.size_bytes = other.size_bytes.filter(|size| *size > 0);
    }
    let usable = !other.title.trim().is_empty()
        && other.title.len() <= 4096
        && !other.title.chars().any(char::is_control);
    if usable
        && ((extra_name(&known.title) && !extra_name(&other.title))
            || (extra_name(&known.title) == extra_name(&other.title)
                && other.title.len() > known.title.len()))
    {
        known.title = other.title.clone();
    }
    let sources = sources::current();
    known.magnet = safe_magnet(
        &known.info_hash,
        &known.title,
        &[&known.magnet, &other.magnet],
        &sources.trackers,
    );
}

/// Counts are per planned provider/term request, including failed or unfinished variants.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SourceReport {
    pub source: String,
    pub requests: usize,
    pub answered: usize,
    pub failed: usize,
    pub skipped: usize,
    pub pending: usize,
    pub rows: usize,
    pub elapsed_ms: u64,
    /// At most MAX_PLANNED_REQUESTS entries, including pending requests.
    #[serde(deserialize_with = "bounded_request_reports")]
    pub outcomes: Vec<RequestReport>,
}

fn bounded_request_reports<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<RequestReport>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<RequestReport>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded list of request reports")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut reports = Vec::new();
            while let Some(report) = seq.next_element()? {
                if reports.len() == MAX_PLANNED_REQUESTS {
                    return Err(serde::de::Error::custom("too many request reports"));
                }
                reports.push(report);
            }
            Ok(reports)
        }
    }
    deserializer.deserialize_seq(Bounded)
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchOutcome {
    pub releases: Vec<Release>,
    pub sources: Vec<SourceReport>,
    /// True when at least one planned request failed or did not finish before return.
    pub partial: bool,
    pub elapsed_ms: u64,
    /// Caller-owned tail; dropping it cancels observation, not admitted cache fills.
    #[serde(skip)]
    pub live: Option<LiveResults>,
}

/// Bounded search tails, retained by a plan rather than detached observer tasks.
#[derive(Clone)]
pub struct LiveResults(Vec<Arc<LiveState>>);

impl std::fmt::Debug for LiveResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LiveResults")
    }
}

struct LiveState {
    receive: tokio::sync::Mutex<FuturesUnordered<futures::future::BoxFuture<'static, Arrival>>>,
    answers: Mutex<Vec<Option<Vec<Release>>>>,
}

impl LiveResults {
    pub fn snapshot(&self) -> Vec<Release> {
        let answers: Vec<_> = self
            .0
            .iter()
            .map(|state| {
                Some(merged(
                    &state
                        .answers
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()),
                ))
            })
            .collect();
        merged(&answers)
    }

    /// Cancellation-safe: only consume an arrival when its evidence is recorded.
    pub async fn changed(&self) -> Option<Vec<Release>> {
        let waits = self.0.iter().map(|state| {
            async move {
                let mut receive = state.receive.lock().await;
                let arrival = receive.next().await.ok_or(())?;
                state
                    .answers
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())[arrival.index] = Some(arrival.rows);
                Ok::<_, ()>(())
            }
            .boxed()
        });
        futures::future::select_ok(waits).await.ok()?;
        Some(self.snapshot())
    }

    pub(crate) fn join(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }
}

/// Asks every indexer and returns what has come back when one of three things happens: all have
/// answered; `enough` says the answers so far hold something to play and `GRACE` has passed since;
/// or `search_ceiling_ms` has run out. The requests still out are not cancelled: they finish in the
/// background and fill the cache. The report's live tail also lets playback observe
/// their rows under its existing deadline, without restarting the search.
///
/// `enough` is given the answers so far, deduplicated, and says whether they hold a release worth
/// opening. The caller knows what the ranking would keep, which is not the same as having rows.
pub async fn search_all(
    config: &Config,
    terms: &[String],
    options: &SearchOptions,
    enough: impl Fn(&[Release]) -> bool,
) -> Vec<Release> {
    search_all_report(config, terms, options, enough)
        .await
        .releases
}

/// Search evidence without upstream addresses or internal errors. Empty successful
/// answers are distinguishable from unavailable providers through the report counts.
pub async fn search_all_report(
    config: &Config,
    terms: &[String],
    options: &SearchOptions,
    enough: impl Fn(&[Release]) -> bool,
) -> SearchOutcome {
    let sources = sources::current();
    let chosen = serving(&sources.indexers, options.kind, config.on_server);
    search_in(&sources, &chosen, config, terms, options, enough).await
}

/// `search_all` over these indexers: the ones that are not fallbacks first, then the fallbacks if
/// what came back is not enough.
pub(crate) async fn search_in(
    sources: &Arc<sources::Sources>,
    chosen: &[&IndexerDef],
    config: &Config,
    terms: &[String],
    options: &SearchOptions,
    enough: impl Fn(&[Release]) -> bool,
) -> SearchOutcome {
    let started = Instant::now();
    let (later, first): (Vec<&IndexerDef>, Vec<&IndexerDef>) =
        chosen.iter().partition(|indexer| indexer.fallback);
    // A single slow indexer must not hold the whole search open. Every normal request is
    // well under a second; the limit only bounds the tail, per host.
    let quick = Duration::from_millis(config.indexer_timeout_ms);
    let ceiling = Duration::from_millis(config.search_ceiling_ms);
    let mut found = search_set(sources, &first, terms, options, &enough, quick, ceiling).await;
    if !later.is_empty() && !enough(&found.releases) {
        let slow = Duration::from_millis(config.fallback_timeout_ms);
        let more = search_set(sources, &later, terms, options, &enough, slow, slow).await;
        found.releases = merged(&[Some(found.releases), Some(more.releases)]);
        found.sources.extend(more.sources);
        found.partial |= more.partial;
        found.live = match (found.live.take(), more.live) {
            (Some(first), Some(second)) => Some(first.join(second)),
            (first, second) => first.or(second),
        };
    }
    found.elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    found
}

/// Asks these indexers and returns what has come back: see `search_all`. `limit` bounds one request
/// and `ceiling` the whole of it.
async fn search_set(
    sources: &Arc<sources::Sources>,
    chosen: &[&IndexerDef],
    terms: &[String],
    options: &SearchOptions,
    enough: &impl Fn(&[Release]) -> bool,
    limit: Duration,
    ceiling: Duration,
) -> SearchOutcome {
    let started = Instant::now();
    let ceiling = started + ceiling;
    let pairs = plan(chosen, terms);
    let total = pairs.len();
    let mut reports: Vec<SourceReport> = Vec::new();
    for (ordinal, (def, _)) in pairs.iter().enumerate() {
        let position = reports
            .iter()
            .position(|report| report.source == def.id)
            .unwrap_or_else(|| {
                reports.push(SourceReport {
                    source: def.id.clone(),
                    ..Default::default()
                });
                reports.len() - 1
            });
        reports[position].requests += 1;
        reports[position].pending += 1;
        reports[position].outcomes.push(RequestReport {
            query_ordinal: ordinal,
            ..Default::default()
        });
    }

    let mut receive = FuturesUnordered::new();
    for (index, (indexer, term)) in pairs.into_iter().enumerate() {
        let (def, term, options) = (indexer.clone(), term, options.clone());
        // Caller-owned futures are bounded by the plan and retained in the live tail;
        // only search_one's admitted producers survive if that tail is dropped.
        let sources = sources.clone();
        receive.push(
            async move {
                let began = Instant::now();
                let result =
                    search_one_report(&def, &term, &options, &sources.trackers, limit).await;
                if let Err(outcome) = &result
                    && *outcome != RequestOutcome::Skipped
                {
                    tracing::warn!(indexer = %def.id, "provider request failed");
                }
                let answered = result.is_ok();
                let skipped = result
                    .as_ref()
                    .is_ok_and(|answer| answer.outcome == RequestOutcome::Skipped);
                let mut answer = result.unwrap_or_else(|outcome| QueryAnswer {
                    rows: Vec::new(),
                    outcome,
                    cache: CacheEvidence::NotObserved,
                });
                answer.rows.truncate(MAX_ROWS_PER_REQUEST);
                Arrival {
                    index,
                    indexer: def.id.clone(),
                    took: began.elapsed(),
                    answered,
                    skipped,
                    outcome: answer.outcome,
                    cache: answer.cache,
                    rows: answer.rows,
                }
            }
            .boxed(),
        );
    }

    let mut answers: Vec<Option<Vec<Release>>> = vec![None; total];
    let mut arrived = 0;
    let mut first_enough: Option<Duration> = None;
    let mut grace_ends: Option<Instant> = None;
    // Per indexer: requests answered, requests failed, rows, slowest answer.
    let mut each: Vec<(String, usize, usize, usize, Duration)> = Vec::new();
    let stopped = loop {
        let until = grace_ends.map_or(ceiling, |grace| grace.min(ceiling));
        match tokio::time::timeout_at(until.into(), receive.next()).await {
            Ok(Some(arrival)) => {
                arrived += 1;
                if let Some(report) = reports
                    .iter_mut()
                    .find(|report| report.source == arrival.indexer)
                {
                    report.pending -= 1;
                    if let Some(entry) = report
                        .outcomes
                        .iter_mut()
                        .find(|entry| entry.query_ordinal == arrival.index)
                    {
                        entry.outcome = if arrival.outcome == RequestOutcome::Answered
                            && !enough(&arrival.rows)
                        {
                            RequestOutcome::Irrelevant
                        } else {
                            arrival.outcome
                        };
                        entry.cache = arrival.cache;
                        entry.elapsed_ms = arrival.took.as_millis().min(u64::MAX as u128) as u64;
                    }
                    report.skipped += usize::from(arrival.skipped);
                    report.answered += usize::from(arrival.answered && !arrival.skipped);
                    report.failed += usize::from(!arrival.answered);
                    report.rows += arrival.rows.len();
                    report.elapsed_ms = report
                        .elapsed_ms
                        .max(arrival.took.as_millis().min(u64::MAX as u128) as u64);
                }
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
    for report in &mut reports {
        for entry in &mut report.outcomes {
            if entry.outcome == RequestOutcome::Pending {
                entry.elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            }
        }
    }
    let found = merged(&answers);
    each.sort_by_key(|row| std::cmp::Reverse(row.4));
    tracing::info!(
        "[search] {}: {arrived} of {total} requests answered in {:?}, stopped on {stopped}; \
         first playable after {}; {} releases; per indexer (requests, failed, rows, slowest): {}",
        options.kind.as_str(),
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
    SearchOutcome {
        releases: found,
        partial: reports
            .iter()
            .any(|report| report.failed > 0 || report.pending > 0),
        sources: reports,
        elapsed_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        live: (!receive.is_empty()).then(|| {
            LiveResults(vec![Arc::new(LiveState {
                receive: tokio::sync::Mutex::new(receive),
                answers: Mutex::new(answers),
            })])
        }),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    // Saturation tests deliberately own every global permit. Serialize network tests
    // against those cases, while the rest of the core suite still runs in parallel.
    pub(crate) static NETWORK_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn options(kind: Kind) -> SearchOptions {
        SearchOptions {
            kind,
            imdb_id: Some("tt0903747".into()),
            mapped: None,
            season: Some(1),
            episode: Some(5),
        }
    }

    fn definitions() -> std::sync::Arc<sources::Sources> {
        sources::current()
    }

    #[test]
    fn a_server_only_indexer_is_asked_by_the_server_and_by_no_desktop_app() {
        let sources = definitions();
        let ids = |on_server: bool| -> Vec<String> {
            serving(&sources.indexers, Kind::Movie, on_server)
                .iter()
                .map(|indexer| indexer.id.clone())
                .collect()
        };
        assert!(ids(true).contains(&"prowlarr".to_string()));
        assert!(!ids(false).contains(&"prowlarr".to_string()));
        // Everything else is asked by both.
        assert_eq!(ids(true).len(), ids(false).len() + 1);
    }

    /// The ids the plan would ask, and the term each is asked.
    fn planned(kind: Kind, terms: &[&str], only: &[&str]) -> Vec<(String, String)> {
        let sources = definitions();
        // As the server sees them: it asks every indexer, `server_only` ones included.
        let chosen: Vec<&IndexerDef> = serving(&sources.indexers, kind, true)
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

    /// An indexer that reads `{"items": [{"title", "hash"}]}` from `path` on a local server.
    fn local_indexer(id: &str, base: &str, path: &str, fallback: bool) -> IndexerDef {
        serde_json::from_value(json!({
            "id": id, "name": id, "kinds": ["series"], "hosts": [base], "fallback": fallback,
            "request": { "path": path, "headers": if fallback { json!({ "x-marker": "1" }) } else { json!({}) } },
            "response": { "format": "json", "rows": "items",
                          "fields": { "title": "title", "hash": "hash" } }
        }))
        .unwrap()
    }

    /// A plugin that brings code, built from `plugins/code-indexer` (see its README): it signs its request
    /// and reads an HTML page into rows. Here it runs against a page served from this machine.
    #[cfg(feature = "code-plugins")]
    #[tokio::test]
    async fn an_indexer_whose_requests_are_made_by_a_plugins_code_finds_releases() {
        use axum::{Router, extract::Query, http::HeaderMap, routing::get};
        use sha2::Digest;
        let app = Router::new().route(
            "/list",
            get(
                |headers: HeaderMap,
                 Query(query): Query<std::collections::HashMap<String, String>>| async move {
                    let term = query.get("q").cloned().unwrap_or_default();
                    let wanted =
                        sources::encode_hex(&sha2::Sha256::digest(format!("secret:{term}")));
                    // Only a request signed the way the service wants is answered.
                    if headers.get("x-sign").and_then(|value| value.to_str().ok()) != Some(&wanted)
                    {
                        return "<p>forbidden</p>".to_string();
                    }
                    format!(
                        "<ul><li data-hash=\"{}\" data-seeders=\"7\">{term} S01E05 1080p</li>\
                         <li data-hash=\"{}\" data-seeders=\"2\">{term} S01E05 720p</li></ul>",
                        "a".repeat(40),
                        "b".repeat(40)
                    )
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let mut def: IndexerDef = serde_json::from_value(json!({
            "id": "signed", "name": "signed", "kinds": ["series"], "hosts": [base], "code": "search",
            "request": { "path": "/list", "query": { "q": "{term}" } },
            "response": { "format": "json", "rows": "items",
                          "fields": { "title": "title", "hash": "hash", "seeders": "seeders" } }
        }))
        .unwrap();
        def.module = Some(crate::definition::Module(Arc::new(
            include_bytes!("../../tests/fixtures/code-indexer.wasm").to_vec(),
        )));

        let found = search_one(
            &def,
            "Show",
            &options(Kind::Series),
            &[],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .any(|release| release.title == "Show S01E05 1080p")
        );
        assert!(found.iter().any(|release| release.seeders == Some(7)));

        // The same indexer without the module cannot be asked.
        def.module = None;
        assert!(
            search_one(
                &def,
                "Other",
                &options(Kind::Series),
                &[],
                Duration::from_secs(5)
            )
            .await
            .is_err()
        );
    }

    /// A server with `/empty`, `/one` (a release) and `/broken`, and how many times `/one` was asked.
    async fn local_server() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use axum::{Router, http::StatusCode, routing::get};
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = asked.clone();
        let app = Router::new()
            .route(
                "/empty",
                get(|| async { json!({ "items": [] }).to_string() }),
            )
            .route(
                "/one",
                get(move || async move {
                    counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    json!({ "items": [{ "title": "Show S01E01", "hash": "a".repeat(40) }] })
                        .to_string()
                }),
            )
            .route(
                "/broken",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .route(
                "/marked",
                get(|headers: axum::http::HeaderMap| async move {
                    if headers.contains_key("x-marker") {
                        Ok(json!({ "items": [{ "title": "Show S01E01", "hash": "b".repeat(40) }] })
                            .to_string())
                    } else {
                        Err(StatusCode::BAD_REQUEST)
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), asked)
    }

    async fn run_search(indexers: &[IndexerDef]) -> Vec<Release> {
        let chosen: Vec<&IndexerDef> = indexers.iter().collect();
        search_in(
            &definitions(),
            &chosen,
            &Config::default(),
            &strings(&["Show S01E01"]),
            &options(Kind::Series),
            |found| !found.is_empty(),
        )
        .await
        .releases
    }

    #[tokio::test]
    async fn reports_distinguish_irrelevant_rows_and_network_timeout() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;
        let def = local_indexer("irrelevant-report", &base, "/one", false);
        let sources = definitions();
        let opts = options(Kind::Series);
        let terms = strings(&["Show"]);
        let limit = Duration::from_millis(20);
        let report = search_set(
            &sources,
            &[&def],
            &terms,
            &opts,
            &|_| false,
            limit,
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            report.sources[0].outcomes[0].outcome,
            RequestOutcome::Irrelevant
        );
        assert_eq!(report.sources[0].outcomes[0].cache, CacheEvidence::Fresh);
        let base = stub().await;
        let def = local_indexer("timeout-report", &base, "/hang", false);
        let report = search_set(
            &sources,
            &[&def],
            &terms,
            &opts,
            &|_| false,
            limit,
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            report.sources[0].outcomes[0].outcome,
            RequestOutcome::Timeout
        );
        assert_eq!(report.sources[0].failed, 1);
    }

    #[test]
    fn request_reports_are_bounded_and_old_reports_default() {
        let old: SourceReport =
            serde_json::from_value(json!({"source":"old", "answered":1})).unwrap();
        assert!(old.outcomes.is_empty());
        let report = json!({"outcomes": vec![json!({}); MAX_PLANNED_REQUESTS + 1]});
        assert!(serde_json::from_value::<SourceReport>(report).is_err());
        assert_eq!(
            classify_error(&anyhow::Error::new(RequestOutcome::Timeout)),
            RequestOutcome::Timeout
        );
    }

    #[tokio::test]
    async fn reports_distinguish_http_cooldown_and_local_admission() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;
        let def = local_indexer("typed-http", &base, "/broken", false);
        let sources = definitions();
        let opts = options(Kind::Series);
        let limit = Duration::from_secs(2);
        // Keep the chosen slice alive across each await.
        let chosen = [&def];
        let terms = strings(&["A"]);
        let first = search_set(&sources, &chosen, &terms, &opts, &|_| false, limit, limit).await;
        assert_eq!(
            first.sources[0].outcomes[0].outcome,
            RequestOutcome::Http { status: 500 }
        );
        let terms = strings(&["B"]);
        let second = search_set(&sources, &chosen, &terms, &opts, &|_| false, limit, limit).await;
        assert_eq!(
            second.sources[0].outcomes[0].outcome,
            RequestOutcome::Cooldown
        );
        let permits: Vec<_> = (0..MAX_PLANNED_REQUESTS)
            .map(|_| task_slots().clone().try_acquire_owned().unwrap())
            .collect();
        let terms = strings(&["C"]);
        let third = search_set(&sources, &chosen, &terms, &opts, &|_| false, limit, limit).await;
        assert_eq!(
            third.sources[0].outcomes[0].outcome,
            RequestOutcome::LocalCongestion
        );
        assert_eq!(
            third.sources[0].outcomes[0].cache,
            CacheEvidence::NotObserved
        );
        let serialized = serde_json::to_string(&third.sources).unwrap();
        assert!(!serialized.contains(&base));
        assert!(!serialized.contains("provider locally busy"));
        drop(permits);
    }

    #[test]
    fn plans_and_global_admission_are_bounded() {
        let sources = definitions();
        let chosen: Vec<_> = sources.indexers.iter().collect();
        let terms: Vec<_> = (0..1000).map(|n| format!("Show {n}")).collect();
        assert_eq!(plan(&chosen, &terms).len(), MAX_PLANNED_REQUESTS);
        assert!(plan(&chosen, &["x".repeat(4097)]).is_empty());
        // A separate instance exercises the exact non-queuing admission primitive.
        let slots = Arc::new(tokio::sync::Semaphore::new(MAX_PLANNED_REQUESTS));
        let permits: Vec<_> = (0..MAX_PLANNED_REQUESTS)
            .map(|_| slots.clone().try_acquire_owned().unwrap())
            .collect();
        assert!(slots.clone().try_acquire_owned().is_err());
        drop(permits);
        assert_eq!(slots.available_permits(), MAX_PLANNED_REQUESTS);
        assert!(Arc::ptr_eq(task_slots(), task_slots()));
        assert!(std::ptr::eq(request_slots(), request_slots()));
    }

    #[tokio::test]
    async fn concurrent_requests_share_the_global_network_limit() {
        let _guard = NETWORK_TEST.lock().await;
        use axum::{Router, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (counted, measured) = (active.clone(), peak.clone());
        let app = Router::new().route(
            "/search",
            get(move || {
                let (active, peak) = (counted.clone(), measured.clone());
                async move {
                    let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(running, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    json!({"items":[]}).to_string()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let def = local_indexer("global-limit", &base, "/search", false);
        let opts = options(Kind::Series);
        let terms: Vec<_> = (0..48).map(|n| format!("Show-{n}")).collect();
        let mut searches = FuturesUnordered::new();
        for term in &terms {
            searches.push(search_one(&def, term, &opts, &[], Duration::from_secs(2)));
        }
        while let Some(result) = searches.next().await {
            assert!(result.unwrap().is_empty());
        }
        assert!(peak.load(Ordering::SeqCst) > 1);
        assert!(peak.load(Ordering::SeqCst) <= MAX_CONCURRENT_REQUESTS);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        server.abort();
    }

    #[tokio::test]
    async fn saturated_admission_still_serves_cached_evidence() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, calls) = local_server().await;
        let def = local_indexer("saturated-cache", &base, "/one", false);
        let sources = definitions();
        let opts = options(Kind::Series);
        let terms = strings(&["Show"]);
        let limit = Duration::from_secs(2);
        assert_eq!(
            search_one(&def, "Show", &opts, &sources.trackers, limit)
                .await
                .unwrap()
                .len(),
            1
        );
        let permits: Vec<_> = (0..MAX_PLANNED_REQUESTS)
            .map(|_| task_slots().clone().try_acquire_owned().unwrap())
            .collect();
        let result = search_set(&sources, &[&def], &terms, &opts, &|_| false, limit, limit).await;
        assert_eq!(result.releases.len(), 1);
        assert_eq!(
            (result.sources[0].answered, result.sources[0].failed),
            (1, 0)
        );
        assert!(!result.partial);
        assert_eq!(result.sources[0].outcomes[0].cache, CacheEvidence::Hit);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop(permits);
    }

    #[tokio::test]
    async fn many_same_flights_share_one_slot_even_when_admission_is_full() {
        let _guard = NETWORK_TEST.lock().await;
        use axum::{Router, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let gate = Arc::new(tokio::sync::Notify::new());
        let held = gate.clone();
        let app = Router::new().route(
            "/one",
            get(move || {
                let (calls, gate) = (counted.clone(), held.clone());
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    gate.notified().await;
                    json!({"items":[{"title":"Show", "hash":"a".repeat(40)}]}).to_string()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let def = local_indexer("many-flights", &base, "/one", false);
        let sources = definitions();
        let opts = options(Kind::Series);
        let terms = strings(&["Show"]);
        let limit = Duration::from_secs(5);
        let first = search_one(&def, "Show", &opts, &sources.trackers, limit);
        tokio::pin!(first);
        tokio::select! {
            result = &mut first => panic!("producer finished before gate: {result:?}"),
            _ = async {
                while calls.load(Ordering::SeqCst) == 0 { tokio::task::yield_now().await; }
            } => {}
        }
        assert_eq!(task_slots().available_permits(), MAX_PLANNED_REQUESTS - 1);
        let permits: Vec<_> = (0..MAX_PLANNED_REQUESTS - 1)
            .map(|_| task_slots().clone().try_acquire_owned().unwrap())
            .collect();
        let chosen = [&def];
        let mut callers = FuturesUnordered::new();
        for _ in 0..MAX_PLANNED_REQUESTS * 2 {
            let mut caller = Box::pin(search_set(
                &sources,
                &chosen,
                &terms,
                &opts,
                &|_| false,
                limit,
                limit,
            ));
            // Every waiter must already be coalesced, not merely an unpolled cache hit.
            assert!(futures::poll!(caller.as_mut()).is_pending());
            callers.push(caller);
        }
        assert_eq!(task_slots().available_permits(), 0);
        gate.notify_one();
        while let Some(result) = callers.next().await {
            assert_eq!(result.releases.len(), 1);
            assert_eq!(
                (result.sources[0].answered, result.sources[0].failed),
                (1, 0)
            );
            assert!(!result.partial);
            assert_eq!(
                result.sources[0].outcomes[0].cache,
                CacheEvidence::Coalesced
            );
        }
        assert_eq!(first.await.unwrap().len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(permits);
        server.abort();
    }

    #[tokio::test]
    async fn http_permit_timeout_does_not_poison_host_health_or_cache() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, calls) = local_server().await;
        let def = local_indexer("local-congestion", &base, "/one", false);
        let opts = options(Kind::Series);
        let permits = request_slots()
            .acquire_many(MAX_CONCURRENT_REQUESTS as u32)
            .await
            .unwrap();
        let result = search_one(&def, "Show", &opts, &[], Duration::from_millis(20)).await;
        assert_eq!(
            classify_error(&result.unwrap_err()),
            RequestOutcome::LocalCongestion
        );
        assert!(
            usable(&base),
            "an unsent request cannot establish host failure"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        drop(permits);
        assert_eq!(
            search_one(&def, "Show", &opts, &[], Duration::from_secs(2))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn error_envelopes_fail_reports_and_are_not_cached() {
        let _guard = NETWORK_TEST.lock().await;
        use axum::{Router, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};
        for bad in [
            json!({"error":"rate limited"}),
            json!({}),
            json!({"items":null}),
            json!({"items":[], "status":"error"}),
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let counted = calls.clone();
            let app = Router::new().route(
                "/search",
                get(move || {
                    let counted = counted.clone();
                    let bad = bad.clone();
                    async move {
                        if counted.fetch_add(1, Ordering::SeqCst) == 0 {
                            bad.to_string()
                        } else {
                            json!({"items":[]}).to_string()
                        }
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let mut def = local_indexer("envelope", &base, "/search", false);
            def.cooldown_secs = 0;
            // Keep owned inputs outside the future, avoiding references to temporaries.
            let sources = definitions();
            let config = Config::default();
            let terms = strings(&["Show"]);
            let opts = options(Kind::Series);
            let chosen = [&def];
            let first = search_in(&sources, &chosen, &config, &terms, &opts, |_| false).await;
            assert_eq!((first.sources[0].answered, first.sources[0].failed), (0, 1));
            assert!(first.partial);
            assert_eq!(
                first.sources[0].outcomes[0].outcome,
                RequestOutcome::BadResponse
            );
            assert_eq!(
                first.sources[0].outcomes[0].cache,
                CacheEvidence::NotObserved
            );
            let second = search_in(&sources, &chosen, &config, &terms, &opts, |_| false).await;
            assert_eq!(
                (second.sources[0].answered, second.sources[0].failed),
                (1, 0)
            );
            assert!(!second.partial);
            assert_eq!(second.sources[0].outcomes[0].outcome, RequestOutcome::Empty);
            assert_eq!(second.sources[0].outcomes[0].cache, CacheEvidence::Fresh);
            let third = search_in(&sources, &chosen, &config, &terms, &opts, |_| false).await;
            assert_eq!(third.sources[0].outcomes[0].outcome, RequestOutcome::Empty);
            assert_eq!(third.sources[0].outcomes[0].cache, CacheEvidence::Hit);
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            server.abort();
        }
    }

    #[tokio::test]
    async fn cached_a_survives_b_failure_and_identical_misses_share_a_flight() {
        let _guard = NETWORK_TEST.lock().await;
        use axum::{Router, extract::Query, routing::get};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let app = Router::new().route(
            "/search",
            get(move |Query(query): Query<HashMap<String, String>>| {
                let counted = counted.clone();
                async move {
                    counted.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    if query.get("q").map(String::as_str) == Some("B") {
                        json!({"error":"rate limited"}).to_string()
                    } else {
                        json!({"items":[{"title":"Show", "hash":"a".repeat(40)}]}).to_string()
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut def = local_indexer("cache-flight", &base, "/search", false);
        def.request.query.insert("q".into(), json!("{term}"));
        let opts = options(Kind::Series);
        let limit = Duration::from_secs(1);
        let (a, same) = tokio::join!(
            search_one(&def, "A", &opts, &[], limit),
            search_one(&def, "A", &opts, &[], limit)
        );
        assert_eq!(a.unwrap().len(), 1);
        assert_eq!(same.unwrap().len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(search_one(&def, "B", &opts, &[], limit).await.is_err());
        assert!(!usable(&base));
        assert_eq!(
            search_one(&def, "A", &opts, &[], limit)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn a_fallback_is_asked_only_when_the_others_found_nothing() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, asked) = local_server().await;

        let found = run_search(&[
            local_indexer("first", &base, "/empty", false),
            local_indexer("later", &base, "/one", true),
        ])
        .await;
        assert_eq!(found.len(), 1);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);

        // With something to play already, the fallback is not asked at all.
        asked.store(0, std::sync::atomic::Ordering::SeqCst);
        let found = run_search(&[
            local_indexer("first", &base, "/one", false),
            local_indexer("later", &base, "/one?again", true),
        ])
        .await;
        assert_eq!(found.len(), 1);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_request_carries_the_headers_its_definition_names() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;

        // The request with the header is answered; the same address asked without it is refused.
        let found = run_search(&[local_indexer("later", &base, "/marked", true)]).await;
        assert_eq!(found.len(), 1);
        let found = run_search(&[local_indexer("plain", &base, "/marked", false)]).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn an_indexer_from_a_plugin_is_chosen_and_asked_like_a_built_in_one() {
        let _definitions = sources::definitions_change_lock().lock().await;
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;
        // Built straight from JSON, which skips the https rule a stand-in on this machine could not meet.
        let plugin: crate::plugin::Plugin = serde_json::from_value(json!({
            "plugin": 1, "id": "stand-in", "name": "Stand-in", "version": 1,
            "indexers": [{
                "id": "plugin-torrents", "name": "Plugin", "kinds": ["series"], "hosts": [base],
                "request": { "path": "/one" },
                "response": { "format": "json", "rows": "items",
                              "fields": { "title": "title", "hash": "hash" } }
            }]
        }))
        .unwrap();
        assert!(sources::use_plugins(&[plugin]).iter().all(Result::is_ok));

        let current = sources::current();
        let chosen = serving(&current.indexers, Kind::Series, false);
        let ours: Vec<&IndexerDef> = chosen
            .iter()
            .copied()
            .filter(|def| def.id == "plugin-torrents")
            .collect();
        let found = search_in(
            &current,
            &ours,
            &Config::default(),
            &strings(&["Show S01E01"]),
            &options(Kind::Series),
            |found| !found.is_empty(),
        )
        .await
        .releases;
        sources::use_plugins(&[]);

        assert_eq!(ours.len(), 1, "the search plan does not include the plugin");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, "plugin-torrents");
    }

    #[tokio::test]
    async fn a_failing_fallback_does_not_fail_the_search() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;
        let found = run_search(&[
            local_indexer("first", &base, "/empty", false),
            local_indexer("later", &base, "/broken", true),
        ])
        .await;
        assert!(found.is_empty());
    }

    #[test]
    fn an_indexer_that_reads_its_answer_differently_does_not_share_cached_answers() {
        let opts = options(Kind::Series);
        let old = local_indexer("plugin", "https://torrents.example.com", "/search", false);
        let mut new = old.clone();
        new.response.fields.title = serde_json::from_value(json!("name")).unwrap();
        let mut keyed = old.clone();
        keyed
            .request
            .headers
            .insert("x-key".into(), "secret".into());
        let key = |def: &IndexerDef| Request::new(def, "Show", &opts).unwrap().key(def);
        assert_ne!(key(&old), key(&new));
        assert_ne!(key(&old), key(&keyed));
        assert_eq!(key(&old), key(&old.clone()));
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
                "torrentio",
                "apibay",
                "zilean-anime",
                "nekobt",
                "bangumi-moe",
                "prowlarr"
            ]
        );
        assert_eq!(
            names(Kind::Series),
            [
                "knaben",
                "torrents-csv",
                "torrentio",
                "eztv",
                "apibay",
                "zilean",
                "torrentsdb",
                "prowlarr"
            ]
        );
        assert_eq!(
            names(Kind::Movie),
            [
                "knaben",
                "torrents-csv",
                "torrentio",
                "yts",
                "apibay",
                "zilean",
                "torrentsdb",
                "prowlarr"
            ]
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

        // The fallback is told the season and episode where the kind has them, and empty where it has none.
        let query = |kind| request("prowlarr", "Show", &options(kind)).unwrap().query;
        let pair = |name: &str, value: &str| (name.to_string(), value.to_string());
        assert!(query(Kind::Series).contains(&pair("season", "1")));
        assert!(query(Kind::Series).contains(&pair("episode", "5")));
        assert!(query(Kind::Anime).contains(&pair("season", "")));
        assert!(query(Kind::Anime).contains(&pair("episode", "5")));
        assert!(query(Kind::Movie).contains(&pair("episode", "")));

        // Zilean takes the season and episode as query parameters, which a film must not send.
        let zilean = request("zilean", "", &options(Kind::Series)).unwrap();
        assert_eq!(
            zilean.path,
            "/dmm/filtered?ImdbId=tt0903747&Season=1&Episode=5"
        );
        let zilean = request("zilean", "", &options(Kind::Movie)).unwrap();
        assert_eq!(zilean.path, "/dmm/filtered?ImdbId=tt0903747");

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

    fn kitsu_id(id: &str, movie: bool) -> Option<Mapped> {
        Some(Mapped {
            mapper: "kitsu".into(),
            kind: Kind::Anime,
            id: id.into(),
            movie,
        })
    }

    #[test]
    fn a_mapped_anime_kitsu_episode_is_asked_by_its_exact_id() {
        let torrentio = definitions().indexer("torrentio").unwrap().clone();
        let mut opts = options(Kind::Anime);
        opts.mapped = kitsu_id("kitsu:7442:1", false);
        opts.episode = Some(1);
        let built = request("torrentio", "", &opts).unwrap();
        assert_eq!(built.path, "/stream/series/kitsu:7442:1.json");
        assert_eq!(mapped(&torrentio, &opts).as_deref(), Some("kitsu:7442:1"));
        for bad in [
            "kitsu:7442:2",
            "kitsu:7442:1:2",
            " kitsu:7442:1 ",
            "kitsu:07442:1",
            "kitsu:7442:01",
            "kitsu:4294967296:1",
            "kitsu:7442:4294967296",
            "k1:p",
            "kitsu:7442",
            "kitsu:nope:1",
            "https://localhost",
        ] {
            opts.mapped = kitsu_id(bad, false);
            assert!(request("torrentio", "", &opts).is_none(), "{bad} was sent");
        }
        opts.mapped = None;
        assert!(request("torrentio", "", &opts).is_none());
        // Found for another kind, or by another mapper, is no mapping for this search.
        opts.mapped = Some(Mapped {
            kind: Kind::Series,
            ..kitsu_id("kitsu:7442:1", false).unwrap()
        });
        assert!(request("torrentio", "", &opts).is_none());
        opts.mapped = Some(Mapped {
            mapper: "other".into(),
            ..kitsu_id("kitsu:7442:1", false).unwrap()
        });
        assert!(request("torrentio", "", &opts).is_none());
    }

    #[test]
    fn a_mapped_anime_movie_uses_the_movie_route_without_an_episode_suffix() {
        let torrentio = definitions().indexer("torrentio").unwrap().clone();
        let mut opts = options(Kind::Anime);
        opts.mapped = kitsu_id("kitsu:50192", true);
        opts.episode = Some(1);
        let built = request("torrentio", "", &opts).unwrap();
        assert_eq!(built.path, "/stream/movie/kitsu:50192.json");
        assert_eq!(mapped_kind(&torrentio, &opts), Some("movie".into()));
        for bad in [
            "kitsu:50192:1",
            "kitsu:50192:1:2",
            " kitsu:50192 ",
            "kitsu:050192",
            "kitsu:4294967296",
            "k1:p",
            "kitsu:nope",
            "https://localhost",
        ] {
            opts.mapped = kitsu_id(bad, true);
            assert!(request("torrentio", "", &opts).is_none(), "{bad} was sent");
        }
        opts.mapped = kitsu_id("kitsu:50192", true);
        opts.episode = Some(2);
        assert!(request("torrentio", "", &opts).is_none());
    }

    #[test]
    fn an_indexer_asked_through_a_mapper_has_its_hole_in_the_path_the_query_or_the_body() {
        let mut def = definitions().indexer("torrentio").unwrap().clone();
        assert!(def.request_uses("kitsu"));
        assert!(!def.request_uses("nothing"));
        def.request.path = json!("/search");
        assert!(!def.request_uses("kitsu"));
        def.request.query.insert("id".into(), json!("{kitsu}"));
        assert!(def.request_uses("kitsu"));
        def.request.query.clear();
        def.request.json = Some(json!({ "id": "{kitsu}" }));
        assert!(def.request_uses("kitsu"));
    }

    #[tokio::test]
    async fn an_unmapped_kitsu_episode_never_requests_the_addon() {
        let _guard = NETWORK_TEST.lock().await;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = calls.clone();
        let app = axum::Router::new().fallback(axum::routing::get(move || {
            let counted = counted.clone();
            async move {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({"streams":[]}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut def = definitions().indexer("torrentio").unwrap().clone();
        def.id = "kitsu-no-mapping-test".into();
        def.hosts = vec![base];
        let mut missing = options(Kind::Anime);
        missing.episode = Some(1);
        let answer = search_one_report(&def, "Show", &missing, &[], Duration::from_millis(100))
            .await
            .unwrap();
        assert_eq!(answer.outcome, RequestOutcome::Skipped);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        server.abort();
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
        let _guard = NETWORK_TEST.lock().await;
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
        assert_eq!(found[0].seeders, Some(3));
        assert!(!usable(&hosts[0]), "a host that failed rests");
        assert!(
            !usable(&hosts[1]),
            "a block page is a failure, not an empty result"
        );
        assert!(usable(&hosts[2]));
    }

    #[tokio::test]
    async fn every_mirror_failing_is_an_error_and_resting_hosts_are_not_asked() {
        let _guard = NETWORK_TEST.lock().await;
        let base = stub().await;
        let hosts = [format!("{base}/down"), format!("{base}/page")];
        let def = indexer_at("all-down-test", &hosts);
        let error = search_one(&def, "show", &options(Kind::Anime), &[], LIMIT).await;
        assert!(error.is_err());
        // Both now rest, so the next term makes no request and caches nothing.
        let again = search_one(&def, "show two", &options(Kind::Anime), &[], LIMIT).await;
        assert!(
            again.is_err(),
            "cooldown is unavailability, not a successful empty answer"
        );
        let request = Request::new(&def, "show", &options(Kind::Anime)).unwrap();
        let key = format!(
            "indexers-v3:{}:{}:{:?}:{:?}",
            sources::current().version,
            request.key(&def),
            "show",
            Vec::<String>::new()
        );
        assert!(
            crate::cache::peek::<Vec<Release>>(&key).is_none(),
            "a failed answer must never be cached as empty"
        );
    }

    #[tokio::test]
    async fn a_mirror_that_hangs_does_not_stop_the_next_one_being_tried() {
        let _guard = NETWORK_TEST.lock().await;
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
        let _guard = NETWORK_TEST.lock().await;
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
        assert!(
            !usable(&hosts[0]),
            "the slow mirror rests for the next search"
        );
        assert!(usable(&hosts[1]));
    }

    #[test]
    fn answers_are_merged_in_request_order_without_repeats() {
        let release = |hash: &str, title: &str| Release {
            title: title.into(),
            info_hash: hash.into(),
            file_idx: None,
            magnet: String::new(),
            size_bytes: None,
            seeders: Some(1),
            source: "t".into(),
            sources: vec!["t".into()],
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
        let mut file = release("a", "same swarm, selected file");
        file.file_idx = Some(0);
        assert_eq!(
            super::merged(&[Some(vec![release("a", "legacy"), file])]).len(),
            2
        );
    }

    #[test]
    fn duplicate_evidence_keeps_provenance_max_known_seeders_and_clean_richer_title() {
        let row = |title: &str, source: &str, seeders, size_bytes| Release {
            title: title.into(),
            info_hash: "a".repeat(40),
            file_idx: None,
            magnet: format!(
                "magnet:?xt=urn:btih:{}&tr=http://127.0.0.1/secret&ws=http://127.0.0.1/x",
                "a".repeat(40)
            ),
            size_bytes,
            seeders,
            source: source.into(),
            sources: Vec::new(),
        };
        let found = merged(&[
            Some(vec![row("Show Bonus EP01.mkv", "first", None, None)]),
            Some(vec![row("Show EP01.mkv", "second", Some(0), Some(100))]),
            Some(vec![row(
                "Show EP01 1080p.mkv",
                "third",
                Some(24),
                Some(200),
            )]),
            Some(vec![row(
                "Show EP01 very long deleted scenes.mkv",
                "fourth",
                None,
                None,
            )]),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Show EP01 1080p.mkv");
        assert_eq!(found[0].source, "first");
        assert_eq!(found[0].sources, ["first", "second", "third", "fourth"]);
        assert_eq!(found[0].seeders, Some(24));
        assert_eq!(found[0].size_bytes, Some(100));
        assert!(!found[0].magnet.contains("secret"));
        assert!(!found[0].magnet.contains("ws="));
    }

    #[test]
    fn public_tracker_union_is_allowlisted_deduplicated_and_bounded() {
        let allowed: Vec<String> = (0..40)
            .map(|i| format!("udp://tracker{i}.example:1337/announce"))
            .collect();
        let hash = "a".repeat(40);
        let first =
            crate::magnet::build_magnet(&hash, None, &[allowed[1].clone(), allowed[0].clone()]);
        let second = crate::magnet::build_magnet(
            &hash,
            None,
            &[allowed[0].clone(), "http://127.0.0.1/secret".into()],
        );
        let magnet = safe_magnet(&hash, "Show", &[&first, &second], &allowed);
        let url = reqwest::Url::parse(&magnet).unwrap();
        let trackers: Vec<String> = url
            .query_pairs()
            .filter(|(name, _)| name == "tr")
            .map(|(_, value)| value.into_owned())
            .collect();
        assert_eq!(trackers.len(), 32);
        assert_eq!(&trackers[..2], &[allowed[1].clone(), allowed[0].clone()]);
        assert!(!magnet.contains("secret"));
    }

    #[test]
    fn legacy_cached_releases_deserialize_with_optional_evidence() {
        let old = json!({ "title": "Show", "info_hash": "a".repeat(40), "magnet": "", "size_bytes": null, "seeders": 0, "source": "legacy" });
        let row: Release = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(row.seeders, Some(0));
        assert!(row.sources.is_empty());
        let mut missing = old;
        missing.as_object_mut().unwrap().remove("seeders");
        let row: Release = serde_json::from_value(missing).unwrap();
        assert_eq!(row.seeders, None);
    }

    #[tokio::test]
    async fn reports_distinguish_empty_failed_skipped_and_pending_answers() {
        let _guard = NETWORK_TEST.lock().await;
        let (base, _) = local_server().await;
        let (broken_base, _) = local_server().await;
        let defs = [
            local_indexer("empty-report", &base, "/empty", false),
            local_indexer("failed-report", &broken_base, "/broken", false),
        ];
        let chosen = defs.iter().collect::<Vec<_>>();
        let report = search_in(
            &definitions(),
            &chosen,
            &Config::default(),
            &strings(&["Show"]),
            &options(Kind::Series),
            |_| false,
        )
        .await;
        assert!(report.releases.is_empty());
        assert!(report.partial);
        assert_eq!(report.sources[0].answered, 1);
        assert_eq!(report.sources[0].failed, 0);
        assert_eq!(report.sources[1].answered, 0);
        assert_eq!(report.sources[1].failed, 1);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains(&base));
        assert_eq!(report.sources[0].outcomes[0].outcome, RequestOutcome::Empty);
        assert_eq!(
            report.sources[1].outcomes[0].outcome,
            RequestOutcome::Http { status: 500 }
        );
        assert!(!serialized.contains("Internal Server Error"));

        let hanging = stub().await;
        let def = local_indexer("pending-report", &hanging, "/hang", false);
        let report = search_set(
            &definitions(),
            &[&def],
            &strings(&["Show"]),
            &options(Kind::Series),
            &|_| false,
            LIMIT,
            Duration::ZERO,
        )
        .await;
        assert!(report.partial);
        assert_eq!(report.sources[0].pending, 1);
        assert_eq!(
            report.sources[0].outcomes[0].outcome,
            RequestOutcome::Pending
        );
        assert_eq!(
            report.sources[0].outcomes[0].cache,
            CacheEvidence::NotObserved
        );

        let mut def = def;
        def.request.path = json!("/needs/{imdb}");
        let mut no_id = options(Kind::Series);
        no_id.imdb_id = None;
        let report = search_set(
            &definitions(),
            &[&def],
            &strings(&["Show"]),
            &no_id,
            &|_| false,
            LIMIT,
            LIMIT,
        )
        .await;
        assert!(!report.partial);
        assert_eq!(report.sources[0].skipped, 1);
        assert_eq!(report.sources[0].answered, 0);
    }

    #[test]
    fn cache_identity_separates_hosts_and_kind() {
        let mut def = local_indexer("scope", "https://one.example", "/empty", false);
        let request = Request::new(&def, "Show", &options(Kind::Series)).unwrap();
        let first = request.key(&def);
        def.hosts = strings(&["https://two.example"]);
        assert_ne!(first, request.key(&def));
        def.hosts = strings(&["https://one.example"]);
        let film = Request::new(&def, "Show", &options(Kind::Movie)).unwrap();
        assert_ne!(first, film.key(&def));
    }

    #[test]
    fn resting_hosts_are_remembered_across_runs() {
        let dir = std::env::temp_dir().join(format!("akidev-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut health = HostHealth::default();
        remember_health(&mut health, &dir);
        rest_host(
            &mut health,
            "http://remembered.test",
            Duration::from_secs(120),
        );
        let saved = std::fs::read_to_string(dir.join("indexer-health.json")).unwrap();
        assert!(saved.contains("remembered.test"));

        // A new run: nothing is resting in memory, and the file says what was.
        health.down.remove("http://remembered.test");
        assert!(!health.down.contains_key("http://remembered.test"));
        let mut restarted = HostHealth::default();
        remember_health(&mut restarted, &dir);
        assert!(restarted.down["http://remembered.test"] > Instant::now());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
