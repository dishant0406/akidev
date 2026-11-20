//! Turns a media id into a URL a browser can play.
//!
//!   media id -> title -> indexer search -> rank releases
//!     -> each configured source in turn: debrid, the torrent engine, the Internet Archive
//!     -> read the file's header, pick the lightest delivery (see `media`)
//!     -> a token, and the URL it unlocks
//!
//! It never fails: when nothing can be played the answer says why, in words a viewer can read.

use crate::config::Settings;
use crate::deliver::{Delivery, Registry};
use crate::dto::Ready;
use crate::hls::{Hls, StartError};
use crate::media::{self, Delivery as Plan};
use crate::origin::Origin;
use crate::sources::{archive, debrid::Debrid};
use akidev_core::model::Release;
use akidev_core::stream::{Choice, FailureScope, Streamer};
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Finding sources at once. More than this would only queue behind the indexers.
const CONCURRENT_RESOLVES: usize = 4;
/// How long a request waits for one of those, before it is told the server is busy.
const QUEUE_FOR: Duration = Duration::from_secs(3);
/// How many torrents are tried in turn when the one that opened cannot be played.
const TORRENT_TRIES: usize = 3;
/// How long a search is kept after the last request waiting for it has gone.
const ABANDON_AFTER: Duration = Duration::from_secs(20);
/// How long past its own deadline one search may run before it is given up, so a step that never
/// answers cannot hold a search slot for good.
const RESOLVE_GRACE: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub enum Outcome {
    Ready(Ready),
    Unavailable(String),
    Busy,
}

/// Searches in flight, one per episode, shared by everyone who asks for that episode while it runs.
///
/// Each search is a task of its own, so it is not tied to the request that started it. When its last
/// waiter leaves (a browser tab closed, a request timed out) it is given `abandon_after` for someone
/// to come back, and then cancelled, which gives back the search slot it held. Without this a search
/// nobody waited for either ran on for minutes or, when kept as a bare shared future, stopped where
/// it was and held its slot until the server restarted; four of those made every visitor see "busy".
struct Flights<T> {
    running: Mutex<HashMap<String, Flight<T>>>,
}

#[derive(Clone)]
struct Flight<T> {
    shared: Shared<BoxFuture<'static, T>>,
    waiters: Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::AbortHandle,
}

impl<T> Flights<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn new() -> Arc<Flights<T>> {
        Arc::new(Flights {
            running: Mutex::new(HashMap::new()),
        })
    }

    /// Joins the search for `key`, starting `work` if none is running. `lost` answers the waiters if
    /// the task dies.
    async fn run<F>(
        self: &Arc<Self>,
        key: String,
        lost: T,
        abandon_after: Duration,
        work: impl FnOnce() -> F + Send + 'static,
    ) -> T
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        let flight = {
            let mut running = self.running.lock().unwrap();
            if let Some(flight) = running.get(&key) {
                flight.clone()
            } else {
                let (flights, forget) = (self.clone(), key.clone());
                let task = tokio::spawn(async move {
                    let outcome = work().await;
                    flights.running.lock().unwrap().remove(&forget);
                    outcome
                });
                let flight = Flight {
                    task: task.abort_handle(),
                    shared: async move { task.await.unwrap_or(lost) }.boxed().shared(),
                    waiters: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                };
                running.insert(key.clone(), flight.clone());
                flight
            }
        };
        flight
            .waiters
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _waiter = Waiter {
            flights: self.clone(),
            key,
            flight: flight.clone(),
            abandon_after,
        };
        flight.shared.await
    }
}

/// One request waiting on a search. When the last one leaves, the search is cancelled after a grace.
struct Waiter<T: Clone + Send + Sync + 'static> {
    flights: Arc<Flights<T>>,
    key: String,
    flight: Flight<T>,
    abandon_after: Duration,
}

impl<T: Clone + Send + Sync + 'static> Drop for Waiter<T> {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering::SeqCst;
        if self.flight.waiters.fetch_sub(1, SeqCst) != 1 {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (flights, key, flight, wait) = (
            self.flights.clone(),
            self.key.clone(),
            self.flight.clone(),
            self.abandon_after,
        );
        runtime.spawn(async move {
            tokio::time::sleep(wait).await;
            if flight.waiters.load(SeqCst) != 0 {
                return;
            }
            let mut running = flights.running.lock().unwrap();
            if running
                .get(&key)
                .is_some_and(|each| Arc::ptr_eq(&each.waiters, &flight.waiters))
            {
                running.remove(&key);
                drop(running);
                flight.task.abort();
                tracing::info!("[play] {key}: nobody is waiting any more, search cancelled");
            }
        });
    }
}

/// Process-wide numeric totals only; never labels from a request or media identity.
#[derive(Default)]
pub struct Metrics {
    attempts: std::sync::atomic::AtomicU64,
    successes: std::sync::atomic::AtomicU64,
    source_errors: std::sync::atomic::AtomicU64,
    delivery_errors: std::sync::atomic::AtomicU64,
    deadlines: std::sync::atomic::AtomicU64,
}
impl Metrics {
    fn increment(counter: &std::sync::atomic::AtomicU64) {
        let _ = counter.fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| Some(n.saturating_add(1)),
        );
    }
    fn snapshot(&self) -> serde_json::Value {
        use std::sync::atomic::Ordering::Relaxed;
        serde_json::json!({ "source_attempts": self.attempts.load(Relaxed), "source_successes": self.successes.load(Relaxed),
            "source_errors": self.source_errors.load(Relaxed), "delivery_errors": self.delivery_errors.load(Relaxed), "deadline_exceeded": self.deadlines.load(Relaxed) })
    }
}

pub struct Player {
    settings: Arc<Settings>,
    /// Absent in tests, which stand a file in for a swarm and must not bind the DHT port.
    streamer: Option<Arc<Streamer>>,
    debrid: Option<Arc<Debrid>>,
    hls: Arc<Hls>,
    registry: Arc<Registry>,
    slots: Arc<Semaphore>,
    /// Requests for the same episode made together are answered by one search.
    flights: Arc<Flights<Outcome>>,
    metrics: Metrics,
}

/// Why one source did not give a playable file.
pub(crate) enum Miss {
    Reason(String),
    Busy,
}

impl Player {
    pub fn new(
        settings: Arc<Settings>,
        streamer: Option<Arc<Streamer>>,
        debrid: Option<Arc<Debrid>>,
        hls: Arc<Hls>,
        registry: Arc<Registry>,
    ) -> Arc<Player> {
        Arc::new(Player {
            settings,
            streamer,
            debrid,
            hls,
            registry,
            slots: Arc::new(Semaphore::new(CONCURRENT_RESOLVES)),
            flights: Flights::new(),
            metrics: Metrics::default(),
        })
    }

    /// Every release that fits an episode or a film, for a viewer to pick from, and which of them the
    /// debrid service already holds (those start at once). `None` for that when there is no such
    /// service or it could not be asked.
    pub async fn releases(
        &self,
        id: &str,
    ) -> Result<(Vec<Choice>, Option<HashSet<String>>), String> {
        let Some(streamer) = &self.streamer else {
            return Err("This server has no torrent engine.".into());
        };
        let mut plan = streamer.plan(id).await?;
        // The initial search intentionally returns early. Include valid rows from its
        // already-running tail once, under an eight-second server budget, not UI polling.
        let late =
            Duration::from_millis(self.settings.indexer_timeout_ms).min(Duration::from_secs(8));
        plan = plan
            .latest_until(
                Instant::now()
                    .checked_add(late)
                    .unwrap_or_else(Instant::now),
            )
            .await;
        if plan.ranked.is_empty()
            && plan.extra.is_empty()
            && plan
                .source_reports
                .iter()
                .any(|report| report.failed > 0 || report.pending > 0)
        {
            return Err(plan.no_sources_reason().into());
        }
        let held = match &self.debrid {
            Some(debrid) => {
                let all: Vec<Release> = plan.ranked.iter().chain(&plan.extra).cloned().collect();
                debrid.holds(&all).await
            }
            None => None,
        };
        Ok((plan.choices(), held))
    }

    /// How many searches are running now, out of `CONCURRENT_RESOLVES`.
    pub fn resolving(&self) -> usize {
        CONCURRENT_RESOLVES - self.slots.available_permits().min(CONCURRENT_RESOLVES)
    }

    pub fn metrics(&self) -> serde_json::Value {
        self.metrics.snapshot()
    }

    pub fn sources(&self) -> Vec<String> {
        self.settings.stream_sources.clone()
    }

    pub fn debrid(&self) -> Option<&Arc<Debrid>> {
        self.debrid.as_ref()
    }

    /// Finds a playable source. `hevc` says the browser can play HEVC, which lets that video be
    /// kept as it is rather than converted.
    pub async fn play(
        self: &Arc<Self>,
        id: &str,
        hevc: bool,
        start: Option<f64>,
        release: Option<String>,
        skip: Vec<String>,
    ) -> Outcome {
        // A resume is part of the request, but only to the minute: it decides how far a conversion
        // may run, not what is found. The release and what is skipped do decide it.
        let key = format!(
            "{id}|{hevc}|{}|{}|{}",
            start.map_or(0, |seconds| (seconds / 60.) as u64),
            release.as_deref().unwrap_or(""),
            skip.join(",")
        );
        let (player, id, release) = (self.clone(), id.to_string(), release);
        let limit = Duration::from_millis(self.settings.resolve_deadline_ms) + RESOLVE_GRACE;
        self.flights
            .run(
                key,
                Outcome::Unavailable("The search was interrupted. Try again.".into()),
                ABANDON_AFTER,
                move || async move {
                    match tokio::time::timeout(
                        limit,
                        player.resolve(&id, hevc, start, release.as_deref(), &skip),
                    )
                    .await
                    {
                        Ok(outcome) => outcome,
                        Err(_) => {
                            Metrics::increment(&player.metrics.deadlines);
                            Outcome::Unavailable("Timed out looking for a playable source.".into())
                        }
                    }
                },
            )
            .await
    }

    async fn resolve(
        self: &Arc<Self>,
        id: &str,
        hevc: bool,
        start: Option<f64>,
        release: Option<&str>,
        skip: &[String],
    ) -> Outcome {
        let Ok(Ok(_slot)) =
            tokio::time::timeout(QUEUE_FOR, self.slots.clone().acquire_owned()).await
        else {
            return Outcome::Busy;
        };

        let Some(streamer) = &self.streamer else {
            return Outcome::Unavailable("This server has no torrent engine.".into());
        };
        let mut plan = match streamer.plan(id).await {
            Ok(plan) => plan.without(skip),
            Err(reason) => return Outcome::Unavailable(reason),
        };
        let started = Instant::now();
        let deadline = started + Duration::from_millis(self.settings.resolve_deadline_ms);
        // A viewer who picked a release gets that one and no other: it is what they asked for, and
        // a different one played in its place would be the opposite of choosing.
        if let Some(selection) = release {
            let Some((hash, file_idx)) = Release::parse_selection_id(selection) else {
                return Outcome::Unavailable(
                    "That release is no longer listed. Pick another.".into(),
                );
            };
            loop {
                plan = plan.latest();
                if let Some(only) = plan.only_selected(&hash, file_idx) {
                    plan = only;
                    break;
                }
                match tokio::time::timeout_at(deadline.into(), plan.wait_for_late()).await {
                    Ok(Some(next)) => plan = next,
                    _ => {
                        return Outcome::Unavailable(
                            "That release is no longer listed. Pick another.".into(),
                        );
                    }
                }
            }
        }
        // What costs this server least is tried first: a release a browser can play without a
        // re-encode (10-bit H.264 is the common one it cannot), and one light enough to download
        // faster than it plays (a REMUX is not). The order is otherwise the library's own.
        let movie = plan.parsed.kind == akidev_core::model::Kind::Movie;
        plan.ranked
            .sort_by_key(|release| (needs_encode_by_name(&release.title), heavy(release, movie)));
        let failure_scope = FailureScope {
            file_index: None,
            capability: Some(if hevc { "web-hevc" } else { "web-h264" }.into()),
        };
        plan = plan.demote_recent_failures_scoped(streamer.failure_memory(), false, &failure_scope);

        let episode = plan.parsed.episode;
        let season = plan.parsed.season;
        let mut reasons: Vec<String> = Vec::new();
        let expected_seconds = plan
            .detail
            .media
            .runtime_minutes
            .filter(|minutes| *minutes > 0)
            .map(|minutes| f64::from(minutes) * 60.0);
        let sources: Vec<&String> = self
            .settings
            .stream_sources
            .iter()
            .filter(|source| release.is_none() || source.as_str() != "archive-org")
            .collect();

        for (index, source) in sources.iter().enumerate() {
            if Instant::now() >= deadline {
                Metrics::increment(&self.metrics.deadlines);
                reasons.push("Timed out looking for a playable source.".into());
                break;
            }
            let remaining_sources = sources.len() - index;
            let source_deadline = Instant::now()
                + deadline.saturating_duration_since(Instant::now()) / remaining_sources as u32;
            Metrics::increment(&self.metrics.attempts);
            let attempt = match source.as_str() {
                "debrid" => match &self.debrid {
                    None => Err(Miss::Reason("The debrid source is not configured.".into())),
                    Some(debrid) => {
                        let mut candidates = plan.latest().ranked;
                        let mut checked = HashSet::new();
                        let mut attempt =
                            Err(Miss::Reason("No cached release could be played.".into()));
                        let mut prepared = 0;
                        loop {
                            candidates
                                .retain(|candidate| !checked.contains(&candidate.selection_id()));
                            if Instant::now() >= source_deadline
                                || prepared >= TORRENT_TRIES
                                || checked.len() >= akidev_core::stream::OFFERED
                            {
                                break;
                            }
                            for candidate in &candidates {
                                checked.insert(candidate.selection_id());
                            }
                            match debrid
                                .find_for_identity(
                                    &candidates,
                                    episode,
                                    season,
                                    &plan.detail.media.title,
                                    &plan.detail.media.title_variants,
                                    plan.detail
                                        .media
                                        .year
                                        .and_then(|year| u32::try_from(year).ok()),
                                    self.settings.max_candidates,
                                    source_deadline,
                                )
                                .await
                            {
                                Ok(found) => {
                                    prepared += 1;
                                    let hash = found.info_hash;
                                    let selection = found.selection_id;
                                    attempt = self
                                        .deliver_checked(
                                            found.origin,
                                            found.release,
                                            Some(selection.clone()),
                                            "debrid",
                                            hevc,
                                            start,
                                            expected_seconds,
                                            source_deadline,
                                        )
                                        .await;
                                    match &attempt {
                                        Err(Miss::Reason(_)) => {
                                            streamer.failure_memory().record_failure_scoped(
                                                id,
                                                &hash,
                                                &failure_scope,
                                            );
                                            plan = plan.without(std::slice::from_ref(&selection));
                                        }
                                        _ => break,
                                    }
                                }
                                Err(reason) => {
                                    attempt = Err(Miss::Reason(reason));
                                }
                            }
                            let latest = plan.latest();
                            candidates = latest
                                .ranked
                                .iter()
                                .filter(|candidate| !checked.contains(&candidate.selection_id()))
                                .cloned()
                                .collect();
                            plan = latest;
                            if candidates.is_empty() {
                                match tokio::time::timeout_at(
                                    source_deadline.into(),
                                    plan.wait_for_late(),
                                )
                                .await
                                {
                                    Ok(Some(next)) => {
                                        candidates = next.ranked.clone();
                                        plan = next;
                                    }
                                    _ => break,
                                }
                            }
                        }
                        attempt
                    }
                },
                "torrent" => {
                    // A release whose file turns out to be unreadable or unplayable is dropped and
                    // the next is tried, a few times, instead of giving up on the source.
                    let mut attempt = Err(Miss::Reason("No torrent could be played.".into()));
                    for _ in 0..TORRENT_TRIES {
                        if Instant::now() >= source_deadline {
                            break;
                        }
                        match streamer
                            .open_best_until(&plan, false, source_deadline)
                            .await
                        {
                            Ok((release, playable)) => {
                                let selection = release.selection_id();
                                let origin = Origin::Torrent {
                                    engine: streamer.engine(),
                                    info_hash: playable.info_hash.clone(),
                                    file_index: playable.file_index,
                                    length: playable.length,
                                    name: playable.file_name,
                                };
                                attempt = self
                                    .deliver_checked(
                                        origin,
                                        release.title,
                                        Some(selection.clone()),
                                        "torrent",
                                        hevc,
                                        start,
                                        expected_seconds,
                                        source_deadline,
                                    )
                                    .await;
                                match &attempt {
                                    Err(Miss::Reason(_)) => {
                                        streamer.failure_memory().record_failure_scoped(
                                            id,
                                            &playable.info_hash,
                                            &failure_scope,
                                        );
                                        plan = plan.without(std::slice::from_ref(&selection));
                                    }
                                    _ => break,
                                }
                            }
                            Err(reason) => {
                                attempt = Err(Miss::Reason(reason));
                                break;
                            }
                        }
                    }
                    attempt
                }
                "archive-org" => {
                    let titles = if plan.detail.media.title_variants.is_empty() {
                        vec![plan.detail.media.title.clone()]
                    } else {
                        plan.detail.media.title_variants.clone()
                    };
                    match archive::find_with_identity(
                        &self.settings.archive_org_base,
                        &titles,
                        &plan.terms,
                        episode,
                        season,
                        expected_seconds,
                        if movie {
                            plan.detail
                                .media
                                .year
                                .and_then(|year| u32::try_from(year).ok())
                        } else {
                            None
                        },
                        source_deadline,
                    )
                    .await
                    {
                        Ok(found) => Ok(Ready {
                            available: true,
                            url: found.url,
                            stream_type: "file",
                            release: found.title,
                            release_id: None,
                            source: "archive-org",
                            duration: None,
                            subtitles: Vec::new(),
                        }),
                        Err(reason) => Err(Miss::Reason(reason)),
                    }
                }
                other => Err(Miss::Reason(format!("Unknown stream source \"{other}\"."))),
            };
            match attempt {
                Ok(ready) => {
                    Metrics::increment(&self.metrics.successes);
                    if let Some(selection) = &ready.release_id
                        && let Some((hash, _)) = Release::parse_selection_id(selection)
                    {
                        streamer
                            .failure_memory()
                            .clear_failure_scoped(id, &hash, &failure_scope);
                    }
                    tracing::info!(
                        "[play] {id}: ready from {source} after {:?} ({} releases found)",
                        started.elapsed(),
                        plan.ranked.len()
                    );
                    return Outcome::Ready(ready);
                }
                Err(Miss::Busy) => return Outcome::Busy,
                Err(Miss::Reason(reason)) => {
                    Metrics::increment(&self.metrics.source_errors);
                    if Instant::now() >= source_deadline {
                        Metrics::increment(&self.metrics.deadlines);
                    }
                    tracing::info!("[play] {id}: {source}: {reason}");
                    reasons.push(reason);
                }
            }
        }

        // When no indexer found anything at all, say so: the sources' own "nothing here" reasons
        // are true but describe the fallbacks, not the actual problem. The reason names the first
        // term, because that is what a person would search for by hand to check.
        if plan.ranked.is_empty() {
            return Outcome::Unavailable(plan.no_sources_reason().into());
        }
        // The last reason is from the source that got furthest: a source is only asked when the
        // ones before it declined.
        Outcome::Unavailable(
            reasons
                .pop()
                .unwrap_or_else(|| "No stream source could serve this episode.".into()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    async fn deliver_checked(
        &self,
        origin: Origin,
        release: String,
        release_id: Option<String>,
        source: &'static str,
        hevc: bool,
        start: Option<f64>,
        expected_seconds: Option<f64>,
        deadline: Instant,
    ) -> Result<Ready, Miss> {
        let result = tokio::time::timeout_at(
            deadline.into(),
            self.deliver_media(
                origin,
                release,
                release_id,
                source,
                hevc,
                start,
                expected_seconds,
            ),
        )
        .await
        .unwrap_or_else(|_| Err(Miss::Reason("Timed out preparing this source.".into())));
        if matches!(result, Err(Miss::Reason(_))) {
            Metrics::increment(&self.metrics.delivery_errors);
        }
        result
    }

    /// Reads the file's header, picks the lightest delivery and hands out a token.
    #[cfg(test)]
    pub(crate) async fn deliver(
        &self,
        origin: Origin,
        release: String,
        release_id: Option<String>,
        source: &'static str,
        hevc: bool,
        start: Option<f64>,
    ) -> Result<Ready, Miss> {
        self.deliver_media(origin, release, release_id, source, hevc, start, None)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn deliver_media(
        &self,
        origin: Origin,
        release: String,
        release_id: Option<String>,
        source: &'static str,
        hevc: bool,
        start: Option<f64>,
        expected_seconds: Option<f64>,
    ) -> Result<Ready, Miss> {
        let origin = Arc::new(origin);
        let probe = media::probe(&origin).await.map_err(|error| {
            tracing::warn!("[play] {release}: {error:#}");
            Miss::Reason("The file could not be read.".into())
        })?;
        if expected_seconds.is_some_and(|expected| {
            probe
                .duration
                .is_some_and(|actual| actual < expected * 0.65 || actual > expected * 1.5)
        }) {
            return Err(Miss::Reason(
                "The file does not match the expected running time.".into(),
            ));
        }
        let plan = media::decide(&probe, hevc);
        tracing::info!(
            "[play] {release}: {:?} {:?} -> {plan:?}",
            probe.container,
            probe.video
        );

        let public = &self.settings.stream_url;
        match plan {
            Plan::Unsupported(reason) => Err(Miss::Reason(reason)),
            Plan::Direct => {
                let token = self.registry.register(Delivery::Direct { origin });
                Ok(Ready {
                    available: true,
                    url: format!("{public}/s/{token}"),
                    stream_type: "file",
                    release,
                    release_id,
                    source,
                    duration: probe.duration,
                    subtitles: Vec::new(),
                })
            }
            Plan::Hls { video, audio } => {
                let session = match self
                    .hls
                    .start(origin, video, audio, probe.duration, start)
                    .await
                {
                    Ok(session) => session,
                    Err(StartError::Busy) => return Err(Miss::Busy),
                    Err(StartError::Failed(reason)) => {
                        tracing::warn!("[play] {release}: {reason}");
                        return Err(Miss::Reason(
                            "This release could not be prepared for playback.".into(),
                        ));
                    }
                };
                if !session.wait_ready(Hls::start_timeout(video)).await {
                    let reason = session.failure().unwrap_or_default();
                    tracing::warn!("[play] {release}: no playlist in time ({reason})");
                    self.hls.destroy(&session);
                    return Err(Miss::Reason(
                        "This release could not be prepared for playback.".into(),
                    ));
                }
                let duration = session.duration().or(probe.duration);
                let token = self.registry.register(Delivery::Hls { session });
                Ok(Ready {
                    available: true,
                    url: format!("{public}/s/{token}/index.m3u8"),
                    stream_type: "hls",
                    release,
                    release_id,
                    source,
                    duration,
                    subtitles: Vec::new(),
                })
            }
        }
    }
}

/// Whether a release is too big to download faster than it plays: a REMUX, or a single episode or
/// film of a size that only a fast swarm can keep up with.
pub fn heavy(release: &Release, movie: bool) -> bool {
    static REMUX: OnceLock<Regex> = OnceLock::new();
    let remux = REMUX
        .get_or_init(|| Regex::new(r"(?i)(?:^|[^a-z0-9])(?:bd)?remux(?:$|[^a-z0-9])").unwrap())
        .is_match(&release.title);
    let limit: u64 = if movie { 9 } else { 3 } * 1024 * 1024 * 1024;
    remux || release.size_bytes.is_some_and(|size| size > limit)
}

/// Whether a release's name says it carries 10-bit video.
pub fn needs_encode_by_name(title: &str) -> bool {
    static TEN_BIT: OnceLock<Regex> = OnceLock::new();
    TEN_BIT
        .get_or_init(|| {
            Regex::new(r"(?i)(?:^|[^a-z0-9])(?:hi10p?|10[- ]?bits?|x265|hevc|av1)(?:$|[^a-z0-9])")
                .unwrap()
        })
        .is_match(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A search that holds a slot until `done` is told, and says 7.
    fn holding(
        slots: &Arc<Semaphore>,
        done: &Arc<tokio::sync::Notify>,
    ) -> impl FnOnce() -> BoxFuture<'static, u32> + Send + 'static {
        let (slots, done) = (slots.clone(), done.clone());
        move || {
            async move {
                let _slot = slots.acquire_owned().await.unwrap();
                done.notified().await;
                7
            }
            .boxed()
        }
    }

    /// The bug this guards against: a search whose every waiter had gone away kept its slot, and four
    /// such searches made the server answer "busy" until it was restarted.
    #[tokio::test]
    async fn a_search_nobody_waits_for_is_cancelled_and_gives_back_its_slot() {
        let flights: Arc<Flights<u32>> = Flights::new();
        let slots = Arc::new(Semaphore::new(1));
        let done = Arc::new(tokio::sync::Notify::new());

        // A waiter that gives up, as a browser does when the tab closes.
        let waiter = {
            let (flights, work) = (flights.clone(), holding(&slots, &done));
            tokio::spawn(async move {
                flights
                    .run("a".into(), 0, Duration::from_millis(50), work)
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            slots.available_permits(),
            0,
            "the search holds the slot while it runs"
        );
        waiter.abort();
        let _ = waiter.await;

        let freed = tokio::time::timeout(Duration::from_secs(2), slots.acquire()).await;
        assert!(freed.is_ok(), "the slot was never given back");
        assert!(flights.running.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_waiter_who_comes_back_in_time_gets_the_search_that_was_running() {
        let flights: Arc<Flights<u32>> = Flights::new();
        let slots = Arc::new(Semaphore::new(1));
        let done = Arc::new(tokio::sync::Notify::new());

        let first = {
            let (flights, work) = (flights.clone(), holding(&slots, &done));
            tokio::spawn(async move {
                flights
                    .run("a".into(), 0, Duration::from_secs(5), work)
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        first.abort();
        let _ = first.await;

        // Back within the grace: joins the same search (a second one would find the slot taken).
        let back = {
            let (flights, work) = (flights.clone(), holding(&slots, &done));
            tokio::spawn(async move {
                flights
                    .run("a".into(), 0, Duration::from_secs(5), work)
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        done.notify_one();
        assert_eq!(back.await.unwrap(), 7);
    }

    #[tokio::test]
    async fn waiters_on_one_search_get_the_same_answer() {
        let flights: Arc<Flights<u32>> = Flights::new();
        let (a, b) = tokio::join!(
            flights.run("k".into(), 0, Duration::from_secs(1), || async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                42
            }),
            flights.run("k".into(), 0, Duration::from_secs(1), || async { 99 }),
        );
        assert_eq!((a, b), (42, 42));
    }

    #[test]
    fn ten_bit_releases_are_the_ones_tried_last() {
        for title in [
            "[Group] Show - 05 (BD 1080p Hi10P FLAC)",
            "[Group] Show - 05 [1080p][10bit]",
            "[LX] Show - 01 [v2][1080p x264 10bits EAC3]",
            "Show.S01E05.1080p.10-bit.x264",
            "Show S01E05 1080p HEVC",
        ] {
            assert!(needs_encode_by_name(title), "{title}");
        }
        for title in [
            "[SubsPlease] Show - 05 (1080p) [ABCD1234].mkv",
            "Show.S01E05.1080p.WEB-DL.x264-GRP",
            "Show 05 720p h264 AAC",
        ] {
            assert!(!needs_encode_by_name(title), "{title}");
        }
    }

    fn release(title: &str, size: Option<u64>) -> Release {
        Release {
            title: title.into(),
            info_hash: "a".repeat(40),
            file_idx: None,
            magnet: String::new(),
            size_bytes: size,
            seeders: Some(1),
            source: "t".into(),
            sources: vec!["t".into()],
        }
    }

    #[test]
    fn a_remux_or_a_huge_file_is_heavy() {
        let gib = 1024 * 1024 * 1024;
        assert!(heavy(
            &release("Show S04E01 1080p FLAC AVC REMUX-FraMeSToR", None),
            false
        ));
        assert!(heavy(&release("Show 05 BDRemux", None), false));
        assert!(heavy(&release("Show 05 1080p", Some(4 * gib)), false));
        assert!(!heavy(&release("Show 05 1080p", Some(4 * gib)), true));
        assert!(heavy(&release("Film 1080p", Some(12 * gib)), true));
        assert!(!heavy(
            &release("[SubsPlease] Show - 05 (1080p)", Some(gib)),
            false
        ));
        assert!(!heavy(&release("Show 05 1080p", None), false));
    }

    #[test]
    fn releases_that_need_no_encode_sort_first_and_keep_their_order() {
        let mut titles = vec!["a 10bit", "b 1080p", "c Hi10P", "d 720p"];
        titles.sort_by_key(|title| needs_encode_by_name(title));
        assert_eq!(titles, ["b 1080p", "d 720p", "a 10bit", "c Hi10P"]);
    }

    /// A real browser-compatible file, with no torrent engine or external requests.
    struct DeliveryFixture {
        player: Arc<Player>,
        path: std::path::PathBuf,
        dir: std::path::PathBuf,
    }

    impl DeliveryFixture {
        async fn mp4() -> Option<Self> {
            if !media::ffmpeg_available().await {
                eprintln!("ffmpeg or ffprobe is missing: skipping local delivery test");
                return None;
            }
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "akidev-play-delivery-{}-{sequence}",
                std::process::id()
            ));
            // Do not overwrite an existing directory or fixture.
            std::fs::create_dir(&dir).expect("create isolated fixture directory");
            let mut settings = Settings::from_env();
            settings.cache_dir = dir.clone();
            settings.stream_url = "http://api.test".into();
            settings.stream_sources = vec!["torrent".into()];
            let settings = Arc::new(settings);
            let hls = Hls::new(settings.clone(), vec![]);
            let fixture = Self {
                player: Player::new(settings, None, None, hls, Arc::new(Registry::default())),
                path: dir.join("clip.mp4"),
                dir,
            };
            let output = tokio::process::Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-n",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=black:size=160x90:rate=24",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=48000",
                    "-t",
                    "2",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-movflags",
                    "+faststart",
                ])
                .arg(&fixture.path)
                .output()
                .await
                .expect("run installed ffmpeg");
            assert!(
                output.status.success(),
                "could not generate MP4: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Some(fixture)
        }

        async fn deliver(&self, expected: f64, deadline: Instant) -> Result<Ready, Miss> {
            let origin = Origin::File {
                path: self.path.clone(),
                length: std::fs::metadata(&self.path).unwrap().len(),
                name: "clip.mp4".into(),
            };
            self.player
                .deliver_checked(
                    origin,
                    "local release".into(),
                    Some("a".repeat(40)),
                    "torrent",
                    false,
                    None,
                    Some(expected),
                    deadline,
                )
                .await
        }
    }

    impl Drop for DeliveryFixture {
        fn drop(&mut self) {
            self.player.hls.shutdown();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn checked_mp4_delivery_registers_the_original_playable_bytes() {
        use tokio::io::AsyncReadExt;

        let Some(fixture) = DeliveryFixture::mp4().await else {
            return;
        };
        let ready = match fixture
            .deliver(2.0, Instant::now() + Duration::from_secs(10))
            .await
        {
            Ok(ready) => ready,
            Err(Miss::Reason(reason)) => panic!("valid MP4 rejected: {reason}"),
            Err(Miss::Busy) => panic!("direct delivery should not need a conversion slot"),
        };
        assert!(ready.available);
        assert_eq!(ready.stream_type, "file");
        assert_eq!(ready.source, "torrent");
        assert_eq!(ready.release, "local release");
        assert_eq!(ready.release_id.as_deref(), Some("a".repeat(40).as_str()));
        assert!((ready.duration.expect("probed duration") - 2.0).abs() < 0.1);
        let token = ready.url.strip_prefix("http://api.test/s/").unwrap();
        let Some(Delivery::Direct { origin }) = fixture.player.registry.get(token) else {
            panic!("delivery URL must unlock the original MP4, not an HLS conversion");
        };
        let mut bytes = Vec::new();
        origin
            .open_at(0)
            .await
            .unwrap()
            .read_to_end(&mut bytes)
            .await
            .unwrap();
        assert_eq!(bytes, std::fs::read(&fixture.path).unwrap());
        assert_eq!(fixture.player.registry.len(), 1);
    }

    #[tokio::test]
    async fn checked_delivery_rejects_both_short_and_long_runtime_mismatches() {
        let Some(fixture) = DeliveryFixture::mp4().await else {
            return;
        };
        // The genuine two-second clip is too short for an episode and too long for a sample.
        for expected in [60.0, 0.5] {
            let result = fixture
                .deliver(expected, Instant::now() + Duration::from_secs(10))
                .await;
            assert!(
                matches!(result, Err(Miss::Reason(ref reason))
                    if reason == "The file does not match the expected running time."),
                "runtime mismatch ({expected}s expected) must fail before publishing a URL"
            );
            assert_eq!(fixture.player.registry.len(), 0);
        }
        assert_eq!(fixture.player.metrics()["delivery_errors"], 2);
    }

    #[tokio::test]
    async fn checked_delivery_rejects_an_expired_budget_without_publishing_a_url() {
        let Some(fixture) = DeliveryFixture::mp4().await else {
            return;
        };
        let result = fixture
            .deliver(2.0, Instant::now() - Duration::from_secs(1))
            .await;
        assert!(
            matches!(result, Err(Miss::Reason(ref reason))
                if reason == "Timed out preparing this source."),
            "even a valid MP4 must obey the source's delivery deadline"
        );
        assert_eq!(fixture.player.registry.len(), 0);
    }
}
