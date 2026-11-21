//! Turns a media id into something a player can open.
//!
//! media id -> title -> indexer search -> rank releases -> race the best swarms
//! -> a localhost URL, or an explicit "nothing available" with the reason.

use crate::config::Config;
use crate::indexers::{LiveResults, SearchOptions, SourceReport, search_all_report};
use crate::media_id::{self, MediaId};
use crate::model::{Detail, Kind, Release, Shape};
use crate::releases::{
    Audio, Codec, is_batch, is_playable_codec, movie_search_terms, normalize_title, parse_audio,
    parse_codec, parse_resolution, rank_movie_releases, rank_movie_releases_by, rank_releases,
    rank_releases_by, search_terms, series_search_terms,
};
use crate::torrent::{Engine, StreamStats};
use crate::{providers, serve, sources};
use futures::StreamExt as _;
use std::collections::{HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Which file of which torrent is playing, for asking how its stream is going.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamId {
    pub info_hash: String,
    pub file_index: usize,
    /// The originating addon row when known; `None` is a legacy hash-only selection.
    pub selection: Option<String>,
}

impl StreamId {
    /// What a retry, download picker or watch party must name to retain the exact file.
    pub fn selection_id(&self) -> String {
        self.selection
            .clone()
            .unwrap_or_else(|| self.info_hash.clone())
    }
}

/// A candidate that has not found peers after this long is joined by the next one.
const STAGGER: Duration = Duration::from_secs(6);

/// How many ranked releases a plan offers. More than a torrent is tried with, because a
/// debrid source only needs one of them to be cached and checking a hash is cheap, while
/// opening a torrent is not.
pub const OFFERED: usize = 24;

const FAILURE_LIMIT: usize = 512;
const FAILURE_TTL: Duration = Duration::from_secs(5 * 60);

/// Optional delivery context. A failure on one file or device must not penalise another.
/// Leave these unknown before a file has been selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FailureScope {
    pub file_index: Option<usize>,
    /// An opaque capability profile, not a device identifier or user history.
    pub capability: Option<String>,
}

struct RecentFailure {
    media: String,
    hash: String,
    scope: FailureScope,
    at: Instant,
}

/// Process-local hints only: bounded, expiring, never persisted or provider-wide.
#[derive(Default)]
pub struct FailureMemory(Mutex<VecDeque<RecentFailure>>);

impl FailureMemory {
    pub fn record_failure(&self, media: &str, hash: &str) {
        self.record_failure_scoped(media, hash, &FailureScope::default());
    }

    pub fn record_failure_scoped(&self, media: &str, hash: &str, scope: &FailureScope) {
        self.record_at(media, hash, scope, Instant::now());
    }

    fn record_at(&self, media: &str, hash: &str, scope: &FailureScope, now: Instant) {
        // Delivery adapters may call this directly; bound every stored external field.
        if media.len() > 128
            || media_id::parse(media).is_none()
            || hash.len() != 40
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || scope.capability.as_ref().is_some_and(|value| {
                value.is_empty() || value.len() > 128 || value.chars().any(char::is_control)
            })
        {
            return;
        }
        let mut failures = self.0.lock().unwrap_or_else(|error| error.into_inner());
        failures.retain(|failure| {
            now.saturating_duration_since(failure.at) < FAILURE_TTL
                && !(failure.media == media
                    && failure.hash.eq_ignore_ascii_case(hash)
                    && failure.scope == *scope)
        });
        while failures.len() >= FAILURE_LIMIT {
            failures.pop_front();
        }
        failures.push_back(RecentFailure {
            media: media.into(),
            hash: hash.to_ascii_lowercase(),
            scope: scope.clone(),
            at: now,
        });
    }

    /// Clears only the unscoped metadata/startup hint, not another device's failure.
    pub fn clear_failure(&self, media: &str, hash: &str) {
        self.clear_failure_scoped(media, hash, &FailureScope::default());
    }

    pub fn clear_failure_scoped(&self, media: &str, hash: &str, scope: &FailureScope) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|failure| {
                !(failure.media == media
                    && failure.hash.eq_ignore_ascii_case(hash)
                    && failure.scope == *scope)
            });
    }
}

/// How many of a plan's best releases the stream services are asked about.
const MOST_SERVICE_RELEASES: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    Ready {
        url: String,
        release: String,
        stream: StreamId,
    },
    /// Nothing could be played. A normal outcome, not an error: the UI says so.
    Unavailable { reason: String },
}

pub struct Streamer {
    config: Mutex<Config>,
    failures: FailureMemory,
    engine: Arc<Engine>,
    address: SocketAddr,
    /// Secret in every stream URL, so only the player can read the server.
    token: String,
}

impl Streamer {
    /// Starts the torrent engine, downloading under a directory it owns inside `dir`,
    /// and the localhost server the player reads from.
    pub async fn start(config: Config, dir: PathBuf) -> anyhow::Result<Streamer> {
        crate::indexers::remember_host_health_in(&dir);
        let engine = Arc::new(Engine::start(&config, &dir).await?);
        let server = serve::start(engine.clone()).await?;
        Ok(Streamer {
            config: Mutex::new(config),
            failures: FailureMemory::default(),
            engine,
            address: server.address,
            token: server.token,
        })
    }

    /// The torrent engine alone, for a server that reads the files itself and has no player
    /// to hand a localhost address to. `resolve` is not meant to be used on one of these.
    pub async fn engine_only(config: Config, dir: PathBuf) -> anyhow::Result<Streamer> {
        crate::indexers::remember_host_health_in(&dir);
        let engine = Arc::new(Engine::start(&config, &dir).await?);
        Ok(Streamer {
            config: Mutex::new(config),
            failures: FailureMemory::default(),
            engine,
            address: SocketAddr::from(([127, 0, 0, 1], 0)),
            token: String::new(),
        })
    }

    /// How a stream is going: bytes fetched, speed and peers.
    pub fn stats(&self, stream: &StreamId) -> Option<StreamStats> {
        self.engine.stats(&stream.info_hash, stream.file_index)
    }

    /// Replaces the settings used by later searches. The torrent limits are read once
    /// at start, so they apply after a restart.
    pub fn set_config(&self, config: Config) {
        *self.config.lock().unwrap() = config;
    }

    /// Delivery probes can report a failure without poisoning an entire provider.
    pub fn record_failure(&self, media: &str, hash: &str) {
        self.failures.record_failure(media, hash);
    }

    pub fn clear_failure(&self, media: &str, hash: &str) {
        self.failures.clear_failure(media, hash);
    }

    /// Scoped delivery reporting and Plan ordering for server adapters.
    pub fn failure_memory(&self) -> &FailureMemory {
        &self.failures
    }

    /// Opens the best of a plan's releases and says where to play it.
    pub async fn resolve_plan(&self, plan: &Plan, fresh: bool) -> Resolution {
        // A stream service a plugin brings is asked first: it plays at once, and when it has nothing the
        // torrent engine runs as before.
        if let Some(found) = self.try_services(plan).await {
            return found;
        }
        match self.open_best(plan, fresh).await {
            Ok((release, playable)) => {
                let selection = release.selection_id();
                Resolution::Ready {
                    url: serve::url(
                        self.address,
                        &self.token,
                        &playable.info_hash,
                        playable.file_index,
                    ),
                    release: release.title,
                    stream: StreamId {
                        info_hash: playable.info_hash,
                        file_index: playable.file_index,
                        selection: Some(selection),
                    },
                }
            }
            Err(reason) => unavailable(&reason),
        }
    }

    /// A playable link from one of the stream services, for the best few releases of the plan, or `None`.
    async fn try_services(&self, plan: &Plan) -> Option<Resolution> {
        let config = self.config.lock().unwrap().clone();
        let sources = crate::sources::current();
        from_services_in(
            &sources.services,
            &config,
            plan,
            crate::plugin::check_public_url,
        )
        .await
    }

    /// The title and the ranked releases for one episode or film, before any is opened.
    /// The reason is for the viewer, and is the whole story when it is an `Err`.
    pub async fn plan(&self, id: &str) -> Result<Plan, String> {
        if id.len() > 128 {
            return Err("Unrecognised media id.".into());
        }
        let config = self.config.lock().unwrap().clone();
        let Some(parsed) = media_id::parse(id) else {
            return Err("Unrecognised media id.".into());
        };
        // A title without an episode is a detail page. A film is the exception: it has
        // no episode by definition.
        if parsed.kind.shape() != Shape::Movie && parsed.episode.is_none() {
            return Err("No episode was requested.".into());
        }
        let started = Instant::now();
        let Some(mut detail) = providers::detail(&parsed.key()).await else {
            return Err("Unknown title.".into());
        };
        Self::validate_request(id, &detail)?;
        let detail_took = started.elapsed();
        let (terms, ranked, extra, source_reports, live) =
            find_releases(&config, &parsed, &mut detail).await;
        tracing::info!(
            "[stream] {id}: title after {detail_took:?}, releases after {:?} more: {} playable, {} others",
            started.elapsed().saturating_sub(detail_took),
            ranked.len(),
            extra.len()
        );
        Ok(Plan {
            id: id.to_string(),
            parsed,
            detail: detail.clone(),
            terms,
            source_reports,
            ranked,
            extra,
            live: live.map(|results| PlanLive {
                identity: PlanIdentity::new(id, &detail),
                results,
                excluded: HashSet::new(),
            }),
        })
    }

    /// Races the best releases of a plan and returns the first that is ready. The rest are
    /// dropped, which deletes their torrents.
    pub async fn open_best(
        &self,
        plan: &Plan,
        fresh: bool,
    ) -> Result<(Release, crate::torrent::Playable), String> {
        let limit = self.config.lock().unwrap().resolve_deadline_ms;
        self.open_best_until(plan, fresh, Instant::now() + Duration::from_millis(limit))
            .await
    }

    /// Uses the caller's remaining budget rather than restarting the deadline on a retry.
    pub async fn open_best_until(
        &self,
        plan: &Plan,
        fresh: bool,
        deadline: Instant,
    ) -> Result<(Release, crate::torrent::Playable), String> {
        plan.validate_request()?;
        let config = self.config.lock().unwrap().clone();
        let id = plan.id.as_str();
        let parsed = &plan.parsed;
        let episode = plan.playback_episode();
        let started = Instant::now();
        let deadline = deadline.min(started + Duration::from_millis(config.resolve_deadline_ms));
        race_plan(
            plan,
            &self.failures,
            fresh,
            config.max_candidates,
            deadline,
            |release| {
                let engine = self.engine.clone();
                let trackers = crate::sources::current().trackers.clone();
                async move {
                    let left = deadline.saturating_duration_since(Instant::now());
                    match engine
                        .open_for_identity(
                            &release,
                            episode,
                            parsed.season,
                            left,
                            fresh,
                            &trackers,
                            &plan.detail.media.title,
                            &plan.detail.media.title_variants,
                            plan.detail
                                .media
                                .year
                                .and_then(|year| u32::try_from(year).ok()),
                        )
                        .await
                    {
                        Ok(playable) => {
                            self.clear_failure(id, &release.info_hash);
                            self.failures.clear_failure_scoped(
                                id,
                                &release.info_hash,
                                &FailureScope {
                                    file_index: Some(playable.file_index),
                                    capability: None,
                                },
                            );
                            Ok(playable)
                        }
                        Err(_) => {
                            self.record_failure(id, &release.info_hash);
                            tracing::warn!("playback candidate failed");
                            Err(())
                        }
                    }
                }
            },
        )
        .await
    }

    /// Validates a request against available catalogue data without I/O.
    /// Unknown counts and absent episode metadata are not proof of nonexistence.
    pub fn validate_request(id: &str, detail: &Detail) -> Result<MediaId, String> {
        validate_request_at(id, detail, crate::calendar::now_ms())
    }

    /// The torrent engine, for a caller that reads the files itself.
    pub fn engine(&self) -> Arc<Engine> {
        self.engine.clone()
    }
}

/// What is known about one episode or film before any torrent is opened.
#[derive(Debug, Clone)]
pub struct Plan {
    pub id: String,
    pub parsed: MediaId,
    pub detail: Detail,
    /// The first of these is what a person would search for by hand.
    pub terms: Vec<String>,
    pub source_reports: Vec<SourceReport>,
    /// Releases found and ranked, best first. A source decides how many to try.
    pub ranked: Vec<Release>,
    /// Releases that fit the title and the episode but that the player cannot be counted on to
    /// decode (HEVC, AV1). Never tried on their own; offered when a viewer picks for themselves.
    pub extra: Vec<Release>,
    /// Opaque, caller-owned late search results. None for explicit pinned choices.
    pub live: Option<PlanLive>,
}

#[derive(Debug, Clone)]
pub struct PlanLive {
    results: LiveResults,
    identity: PlanIdentity,
    excluded: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PlanIdentity {
    id: String,
    catalog: String,
    kind: Kind,
    title: String,
    variants: Vec<String>,
    year: Option<i32>,
}

impl PlanIdentity {
    fn new(id: &str, detail: &Detail) -> Self {
        Self {
            id: id.into(),
            catalog: detail.media.id.clone(),
            kind: detail.media.kind,
            title: detail.media.title.clone(),
            variants: detail.media.title_variants.clone(),
            year: detail.media.year,
        }
    }
}

/// One orchestrator for initial and late candidates. All attempts are owned by this
/// future: success, deadline and caller cancellation drop every losing open.
async fn race_plan<T, F, Fut>(
    plan: &Plan,
    memory: &FailureMemory,
    fresh: bool,
    max_candidates: usize,
    deadline: Instant,
    open: F,
) -> Result<(Release, T), String>
where
    F: Fn(Release) -> Fut,
    Fut: std::future::Future<Output = Result<T, ()>>,
{
    plan.validate_request()?;
    let mut current = plan.demote_recent_failures(memory, fresh);
    let mut live = plan
        .live
        .as_ref()
        .filter(|live| live.identity == PlanIdentity::new(&plan.id, &plan.detail));
    if let Some(tail) = live {
        current = plan
            .with_late(&tail.results.snapshot())
            .demote_recent_failures(memory, fresh);
    }
    let at_once = max_candidates.clamp(1, OFFERED);
    let mut attempted = HashSet::new();
    let mut pending = futures::stream::FuturesUnordered::new();
    let mut next_start = Instant::now();
    loop {
        if Instant::now() >= deadline {
            return Err("Timed out waiting for a playable source.".into());
        }
        let next = current
            .ranked
            .iter()
            .find(|release| !attempted.contains(&release.selection_id().to_ascii_lowercase()))
            .cloned();
        let room = attempted.len() < OFFERED && pending.len() < at_once && next.is_some();
        if room && (pending.is_empty() || Instant::now() >= next_start) {
            let release = next.unwrap();
            attempted.insert(release.selection_id().to_ascii_lowercase());
            let attempt = open(release.clone());
            pending.push(async move { attempt.await.map(|ready| (release, ready)) });
            next_start = Instant::now() + STAGGER;
            continue;
        }
        if pending.is_empty() && (!room && live.is_none() || attempted.len() == OFFERED) {
            return Err(if attempted.is_empty() {
                plan.no_sources_reason().into()
            } else {
                "Found releases, but none could be played.".into()
            });
        }
        tokio::select! {
            _ = tokio::time::sleep_until(deadline.into()) => {
                return Err("Timed out waiting for a playable source.".into());
            }
            result = pending.next(), if !pending.is_empty() => {
                if let Some(Ok(ready)) = result { return Ok(ready); }
                next_start = Instant::now();
            }
            rows = async { live.unwrap().results.changed().await }, if live.is_some() && attempted.len() < OFFERED => {
                match rows {
                    Some(rows) => {
                        current = plan.with_late(&rows).demote_recent_failures(memory, fresh);
                        // A genuine new answer need not wait behind a silent swarm's stagger.
                        next_start = Instant::now();
                    }
                    None => live = None,
                }
            }
            _ = tokio::time::sleep_until(next_start.into()), if room => {}
        }
    }
}

/// How many candidates must have failed before the one at `index` starts without waiting for its
/// turn on the clock: all those ahead of it among the first `at_once`, and past those, enough that
/// no more than `at_once` are open.
#[cfg(test)]
fn failures_to_start(index: usize, at_once: usize) -> usize {
    if index < at_once {
        index
    } else {
        index + 1 - at_once
    }
}

/// Reported positive health leads, then unknown health, then observed zero. Every candidate
/// still needs actual metadata and startup bytes; upstream counts are not availability proof.
fn candidates(ranked: &[Release]) -> Vec<&Release> {
    let (seeded, other): (Vec<&Release>, Vec<&Release>) = ranked
        .iter()
        .partition(|release| release.seeders.is_some_and(|count| count > 0));
    let (unknown, dead): (Vec<&Release>, Vec<&Release>) = other
        .into_iter()
        .partition(|release| release.seeders.is_none());
    seeded.into_iter().chain(unknown).chain(dead).collect()
}

/// Asks `services` about the best few releases of `plan`, and says where to play the first link. A service that
/// needs a key and has none in `config` is left alone.
async fn from_services_in(
    services: &[crate::services::ServiceDef],
    config: &Config,
    plan: &Plan,
    public: fn(&str) -> anyhow::Result<()>,
) -> Option<Resolution> {
    if services.is_empty() || plan.validate_request().is_err() {
        return None;
    }
    let deadline = Instant::now() + Duration::from_millis(config.service_timeout_ms);
    let media = &plan.detail.media;
    let wanted = crate::services::Wanted {
        title: media.title.clone(),
        titles: media.title_variants.clone(),
        year: media.year.and_then(|year| u32::try_from(year).ok()),
        season: plan.parsed.season,
        episode: plan.playback_episode(),
    };
    for release in candidates(&plan.ranked)
        .into_iter()
        .take(MOST_SERVICE_RELEASES)
    {
        for service in services {
            let key = config.service_keys.get(&service.id);
            if service.key_label.is_some() && key.is_none_or(|key| key.trim().is_empty()) {
                continue;
            }
            let key = key.map(String::as_str).unwrap_or_default();
            match crate::services::resolve_with_public(
                service, key, release, &wanted, deadline, public,
            )
            .await
            {
                Ok(Some(url)) => {
                    return Some(Resolution::Ready {
                        url,
                        release: release.title.clone(),
                        stream: StreamId {
                            info_hash: release.info_hash.clone(),
                            file_index: release.file_idx.unwrap_or(0),
                            selection: Some(release.selection_id()),
                        },
                    });
                }
                Ok(None) => {}
                Err(error) => tracing::debug!("[service] {}: {error:#}", service.id),
            }
        }
    }
    None
}

fn unavailable(reason: &str) -> Resolution {
    Resolution::Unavailable {
        reason: reason.to_string(),
    }
}

fn validate_request_at(id: &str, detail: &Detail, now_ms: i64) -> Result<MediaId, String> {
    if id.len() > 128 {
        return Err("Unrecognised media id.".into());
    }
    let parsed = media_id::parse(id).ok_or("Unrecognised media id.")?;
    if parsed.kind != detail.media.kind || parsed.key() != detail.media.id {
        return Err("Unknown title.".into());
    }
    if parsed.kind.shape() == Shape::Movie {
        return Ok(parsed);
    }
    let number = parsed.episode.ok_or("No episode was requested.")?;
    if number == 0 {
        return Err("This episode does not exist.".into());
    }
    // Zero counts also represent unknown metadata. Series totals may omit specials.
    if parsed.season != Some(0)
        && detail
            .media
            .episodes
            .is_some_and(|count| count > 0 && number > count)
    {
        return Err("This episode does not exist.".into());
    }
    let episode = detail
        .episodes
        .iter()
        .find(|episode| episode.number == number && episode.season == parsed.season);
    // A populated series catalogue with season coordinates establishes which
    // season/episode pairs exist. Empty or coordinate-incomplete data cannot.
    if parsed.kind.shape() == Shape::Series
        && !detail.episodes.is_empty()
        && detail
            .episodes
            .iter()
            .all(|episode| episode.season.is_some())
        && episode.is_none()
    {
        return Err("This episode does not exist.".into());
    }
    if let Some(date) = episode.and_then(|episode| episode.airdate.as_deref())
        && let Some(airdate) = crate::calendar::parse_date_ms(date)
    {
        // Reject only real Gregorian dates: the helper normalises invalid days.
        let (year, month, day) = crate::calendar::civil_from_ms(airdate);
        let valid_date = format!("{year:04}-{month:02}-{day:02}");
        if date.starts_with(&valid_date)
            && airdate.div_euclid(crate::calendar::DAY_MS)
                > now_ms.div_euclid(crate::calendar::DAY_MS)
        {
            return Err("This episode has not aired yet.".into());
        }
    }
    Ok(parsed)
}

/// The search terms, the releases they found that can be played, best first, and the others
/// that fit but may not decode.
async fn find_releases(config: &Config, parsed: &MediaId, detail: &mut Detail) -> FoundReleases {
    let media = &detail.media;
    let titles = if media.title_variants.is_empty() {
        vec![media.title.clone()]
    } else {
        media.title_variants.clone()
    };
    let found = find_releases_named(config, parsed, detail, &titles).await;
    if !found.1.is_empty() || parsed.kind.shape() == Shape::Anime {
        return found;
    }
    // A catalogue of films and series knows a Japanese show by one spelling, and releases are
    // named by another ("Tomei na Yoru ni Kakeru Kimi to" is "Love Unseen Beneath the Clear Night
    // Sky" at the indexers). When nothing was found, the anime catalogue is asked for the names
    // it knows the same title by, and the search is made again with them.
    let aliases = alias_titles(media).await;
    if aliases.is_empty() {
        return found;
    }
    let mut more = titles;
    more.extend(aliases);
    let again = find_releases_named(config, parsed, detail, &more).await;
    detail.media.title_variants = more;
    let mut merged = merge_searches(found, again);
    merged.1.truncate(OFFERED);
    merged.2.truncate(OFFERED);
    merged
}

type FoundReleases = (
    Vec<String>,
    Vec<Release>,
    Vec<Release>,
    Vec<SourceReport>,
    Option<LiveResults>,
);

fn merge_searches(mut first: FoundReleases, second: FoundReleases) -> FoundReleases {
    for term in second.0 {
        if !first.0.contains(&term) {
            first.0.push(term);
        }
    }
    for release in second.1 {
        let selection = release.selection_id();
        first.2.retain(|other| other.selection_id() != selection);
        if !first
            .1
            .iter()
            .any(|other| other.selection_id() == selection)
        {
            first.1.push(release);
        }
    }
    for release in second.2 {
        let selection = release.selection_id();
        if !first
            .1
            .iter()
            .chain(&first.2)
            .any(|other| other.selection_id() == selection)
        {
            first.2.push(release);
        }
    }
    first.3.extend(second.3);
    first.4 = match (first.4.take(), second.4) {
        (Some(a), Some(b)) => Some(a.join(b)),
        (a, b) => a.or(b),
    };
    first
}

/// The other names the anime catalogue knows `media` by, when it has the very same title.
async fn alias_titles(media: &crate::model::Media) -> Vec<String> {
    for query in alias_queries(&media.title) {
        let Ok(found) = crate::search::search_kind(Kind::Anime, &query).await else {
            continue;
        };
        for candidate in found.iter().take(3) {
            let Some(detail) = providers::detail(&candidate.id).await else {
                continue;
            };
            let names = known_names(&detail.media);
            if is_same_title(media, &detail.media, &names) {
                return names;
            }
        }
    }
    Vec::new()
}

/// What to ask the anime catalogue: the whole title, then each part of it that is long enough to
/// mean something (a spelling that differs in one word still has a part that is the same).
fn alias_queries(title: &str) -> Vec<String> {
    let mut parts: Vec<String> = title
        .split([',', ':', ';', '(', ')'])
        .map(|part| part.trim().to_string())
        .filter(|part| part.chars().count() >= 8 && part != title)
        .collect();
    parts.sort_by_key(|part| std::cmp::Reverse(part.chars().count()));
    let mut queries = vec![title.to_string()];
    queries.extend(parts);
    queries
}

/// A title with the differences between romanisations taken out ("Tomei" and "Toumei", "Shoujo"
/// and "Shojo"), and its spaces, for comparing two spellings of one name.
fn loose_key(title: &str) -> String {
    normalize_title(title)
        .replace("ou", "o")
        .replace("oo", "o")
        .replace("uu", "u")
        .replace(' ', "")
}

/// Every name a card carries: its title and its variants.
fn known_names(media: &crate::model::Media) -> Vec<String> {
    let mut names = vec![media.title.clone()];
    names.extend(media.title_variants.iter().cloned());
    names
}

/// AniList uses `MOVIE`, while Anime Kitsu uses `movie`; both need film discovery and ranking.
fn is_film(media: &crate::model::Media) -> bool {
    media.kind.shape() == Shape::Movie
        || media
            .format
            .as_deref()
            .is_some_and(|format| format.eq_ignore_ascii_case("movie"))
}

/// Whether the anime `candidate` (with these `names`) is `media`: one of its names is the title,
/// and the years, when both are known, are no more than a year apart.
fn is_same_title(
    media: &crate::model::Media,
    candidate: &crate::model::Media,
    names: &[String],
) -> bool {
    let wanted = loose_key(&media.title);
    let named = names.iter().any(|name| loose_key(name) == wanted);
    let near = match (media.year, candidate.year) {
        (Some(a), Some(b)) => (a - b).abs() <= 1,
        _ => true,
    };
    named && near
}

async fn find_releases_named(
    config: &Config,
    parsed: &MediaId,
    detail: &Detail,
    titles: &[String],
) -> FoundReleases {
    let media = &detail.media;
    let year = media.year.and_then(|year| u32::try_from(year).ok());
    // Torrentio and EZTV are searched by IMDb id rather than by text, and Torrentio
    // takes the season and episode in its path. Indexers that need none of it ignore it.
    // An id from a mapper (Kitsu, for Torrentio) keeps the catalogue's number only when the mapper says it
    // is the same episode. An absent or mismatched mapping is simply no result from that indexer. Only
    // an indexer that names the mapper and has its hole somewhere in its request makes the mapping
    // request; the streams are still found at the indexer's own address.
    let mapped = match parsed {
        MediaId {
            episode: Some(episode),
            ..
        } => {
            let sources = sources::current();
            let mapper = sources.indexers.iter().find_map(|indexer| {
                let mapper = indexer.mapper_id()?;
                (indexer.kinds.contains(&parsed.kind) && indexer.request_uses(mapper))
                    .then(|| mapper.to_string())
            });
            match mapper {
                Some(mapper) => {
                    providers::mapped_episode(&mapper, parsed.kind, &parsed.provider_id, *episode)
                        .await
                }
                None => None,
            }
        }
        _ => None,
    };
    let options = SearchOptions {
        kind: parsed.kind,
        imdb_id: detail.imdb_id.clone(),
        mapped,
        season: parsed.season,
        episode: parsed.episode,
    };

    let (terms, mut ranked, mut all, reports, live) = match (parsed.kind.shape(), parsed.episode) {
        (Shape::Movie, _) => {
            let terms = movie_search_terms(titles, year);
            let outcome = search_all_report(config, &terms, &options, |found| {
                !rank_movie_releases(found, year, &media.title, titles).is_empty()
            })
            .await;
            let found = outcome.releases;
            let ranked = rank_movie_releases(&found, year, &media.title, titles);
            let all = rank_movie_releases_by(&found, year, &media.title, titles, false);
            (terms, ranked, all, outcome.sources, outcome.live)
        }
        (Shape::Series, Some(episode)) => {
            let terms = series_search_terms(titles, parsed.season, Some(episode));
            let outcome = search_all_report(config, &terms, &options, |found| {
                !rank_releases(found, episode, &media.title, titles, parsed.season).is_empty()
            })
            .await;
            let found = outcome.releases;
            let ranked = rank_releases(&found, episode, &media.title, titles, parsed.season);
            let all = rank_releases_by(&found, episode, &media.title, titles, parsed.season, false);
            (terms, ranked, all, outcome.sources, outcome.live)
        }
        (Shape::Anime, Some(_)) if is_film(media) => {
            // Anime films are still addressed as `anime-...-ep1`, but discovery and ranking are
            // film searches, not episode searches. Kitsu uses the movie route above as well.
            let terms = movie_search_terms(titles, year);
            let outcome = search_all_report(config, &terms, &options, |found| {
                !rank_movie_releases(found, year, &media.title, titles).is_empty()
            })
            .await;
            let found = outcome.releases;
            let ranked = rank_movie_releases(&found, year, &media.title, titles);
            let all = rank_movie_releases_by(&found, year, &media.title, titles, false);
            (terms, ranked, all, outcome.sources, outcome.live)
        }
        (Shape::Anime, Some(episode)) => {
            let terms = search_terms(titles, Some(episode));
            let outcome = search_all_report(config, &terms, &options, |found| {
                !rank_releases(found, episode, &media.title, titles, parsed.season).is_empty()
            })
            .await;
            let found = outcome.releases;
            let ranked = rank_releases(&found, episode, &media.title, titles, parsed.season);
            let all = rank_releases_by(&found, episode, &media.title, titles, parsed.season, false);
            (terms, ranked, all, outcome.sources, outcome.live)
        }
        (_, None) => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), None),
    };
    ranked.truncate(OFFERED);
    all.retain(|release| {
        !ranked
            .iter()
            .any(|kept| kept.selection_id() == release.selection_id())
    });
    all.truncate(OFFERED);
    (terms, ranked, all, reports, live)
}

/// One release a viewer may pick, with what can be told from its name.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Choice {
    /// A hash alone identifies the swarm; this keeps the addon's selected pack file.
    pub id: String,
    pub title: String,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    /// Which indexer found it.
    pub source: String,
    pub sources: Vec<String>,
    /// 480, 720, 1080, 2160.
    pub resolution: Option<u32>,
    /// `h264`, `hevc`, `av1`.
    pub codec: Option<&'static str>,
    /// `lossless` (FLAC, DTS, AC-3...) or `compressed` (AAC, MP3, Opus).
    pub audio: Option<&'static str>,
    /// A whole season or series in one torrent, of which the episode is one file.
    pub pack: bool,
    /// Not played unless picked: a codec that may not decode where it is shown.
    pub uncommon: bool,
}

fn normalized_selection(id: &str) -> Option<String> {
    Release::parse_selection_id(id).map(|(hash, file_idx)| match file_idx {
        Some(file_idx) => format!("{hash}:{file_idx}"),
        None => hash,
    })
}

/// A legacy bare hash excludes its whole swarm; a scoped id excludes only that addon file.
fn selection_excluded(excluded: &HashSet<String>, release: &Release) -> bool {
    excluded.contains(&release.info_hash.to_ascii_lowercase())
        || (release.file_idx.is_some()
            && excluded.contains(&release.selection_id().to_ascii_lowercase()))
}

fn choice(release: &Release, uncommon: bool) -> Choice {
    Choice {
        id: release.selection_id(),
        title: release.title.clone(),
        size_bytes: release.size_bytes,
        seeders: release.seeders,
        source: release.source.clone(),
        sources: release.sources.clone(),
        resolution: parse_resolution(&release.title),
        codec: parse_codec(&release.title).map(|codec| match codec {
            Codec::H264 => "h264",
            Codec::Hevc => "hevc",
            Codec::Av1 => "av1",
        }),
        audio: parse_audio(&release.title).map(|audio| match audio {
            Audio::Lossless => "lossless",
            Audio::Compressed => "compressed",
        }),
        pack: is_batch(&release.title),
        uncommon,
    }
}

impl Plan {
    /// Current ranked evidence without another provider search. Server delivery
    /// adapters can await `wait_for_late` using their existing caller deadline.
    pub fn latest(&self) -> Plan {
        match &self.live {
            Some(live) => self.with_late(&live.results.snapshot()),
            None => self.clone(),
        }
    }

    /// Collects the existing search's arrivals until it finishes or this bounded wait ends.
    /// Accumulated evidence is retained on timeout; no provider request is restarted.
    pub async fn latest_until(&self, deadline: Instant) -> Plan {
        let has_live = self
            .live
            .as_ref()
            .is_some_and(|live| live.identity == PlanIdentity::new(&self.id, &self.detail));
        let mut plan = self.latest();
        while has_live && plan.live.is_some() && Instant::now() < deadline {
            match tokio::time::timeout_at(deadline.into(), plan.wait_for_late()).await {
                Ok(Some(next)) => plan = next,
                _ => break,
            }
        }
        plan.latest()
    }

    pub async fn wait_for_late(&self) -> Option<Plan> {
        let live = self
            .live
            .as_ref()
            .filter(|live| live.identity == PlanIdentity::new(&self.id, &self.detail))?;
        Some(self.with_late(&live.results.changed().await?))
    }

    fn with_late(&self, rows: &[Release]) -> Plan {
        let Some(live) = self
            .live
            .as_ref()
            .filter(|live| live.identity == PlanIdentity::new(&self.id, &self.detail))
        else {
            return self.clone();
        };
        let names = known_names(&self.detail.media);
        let year = self
            .detail
            .media
            .year
            .and_then(|year| u32::try_from(year).ok());
        let filtered: Vec<_> = rows
            .iter()
            .filter(|release| {
                !selection_excluded(&live.excluded, release)
                    && !year.is_some_and(|wanted| {
                        crate::releases::matching_title_year(&release.title, &names)
                            .is_some_and(|named| named != wanted)
                    })
            })
            .cloned()
            .collect();
        let mut plan = self.clone();
        // Keep late uncommon rows, not just those admitted by the initial codec filter.
        let extras = match self.parsed.episode {
            _ if is_film(&self.detail.media) => {
                rank_movie_releases_by(&filtered, year, &self.detail.media.title, &names, false)
            }
            Some(episode) => rank_releases_by(
                &filtered,
                episode,
                &self.detail.media.title,
                &names,
                self.parsed.season,
                false,
            ),
            None => Vec::new(),
        }
        .into_iter()
        .filter(|release| !is_playable_codec(&release.title))
        .filter(|release| {
            !plan.extra.iter().any(|known| {
                known
                    .selection_id()
                    .eq_ignore_ascii_case(&release.selection_id())
            })
        })
        .collect::<Vec<_>>();
        plan.extra.extend(extras);
        let ranked = match self.parsed.episode {
            _ if is_film(&self.detail.media) => {
                rank_movie_releases(&filtered, year, &self.detail.media.title, &names)
            }
            Some(episode) => rank_releases(
                &filtered,
                episode,
                &self.detail.media.title,
                &names,
                self.parsed.season,
            ),
            None => Vec::new(),
        };
        for release in ranked {
            if !plan.ranked.iter().any(|known| {
                known
                    .selection_id()
                    .eq_ignore_ascii_case(&release.selection_id())
            }) {
                plan.ranked.push(release);
            }
        }
        // Re-rank unattempted evidence, not merely append in arrival order.
        let mut reranked = match self.parsed.episode {
            _ if is_film(&self.detail.media) => {
                rank_movie_releases(&plan.ranked, year, &self.detail.media.title, &names)
            }
            Some(episode) => rank_releases(
                &plan.ranked,
                episode,
                &self.detail.media.title,
                &names,
                self.parsed.season,
            ),
            None => Vec::new(),
        };
        reranked.truncate(OFFERED * 2);
        plan.ranked = reranked;
        plan
    }

    /// Anime films keep an `ep1` address for compatibility, but their files are selected as films.
    pub fn playback_episode(&self) -> Option<u32> {
        if is_film(&self.detail.media) {
            None
        } else {
            self.parsed.episode
        }
    }

    /// Rechecks public/mutable plans at delivery, including the parsed identity.
    pub fn validate_request(&self) -> Result<(), String> {
        let parsed = Streamer::validate_request(&self.id, &self.detail)?;
        if parsed != self.parsed {
            return Err("Unrecognised media id.".into());
        }
        Ok(())
    }

    /// Recent failures follow all other valid candidates, never disappear. Fresh retries
    /// and explicitly pinned (single-release) plans bypass failure demotion.
    /// Read `ranked` directly on the returned plan to retain this ordering.
    pub fn demote_recent_failures(&self, memory: &FailureMemory, fresh: bool) -> Plan {
        self.demote_recent_failures_scoped(memory, fresh, &FailureScope::default())
    }

    pub fn demote_recent_failures_scoped(
        &self,
        memory: &FailureMemory,
        fresh: bool,
        scope: &FailureScope,
    ) -> Plan {
        self.demote_at(memory, fresh, scope, Instant::now())
    }

    fn demote_at(
        &self,
        memory: &FailureMemory,
        fresh: bool,
        scope: &FailureScope,
        now: Instant,
    ) -> Plan {
        let ordered = candidates(&self.ranked);
        if fresh || self.ranked.len() <= 1 {
            return Plan {
                ranked: ordered.into_iter().cloned().collect(),
                ..self.clone()
            };
        }
        let mut failures = memory.0.lock().unwrap_or_else(|error| error.into_inner());
        failures.retain(|failure| now.saturating_duration_since(failure.at) < FAILURE_TTL);
        let (healthy, failed): (Vec<_>, Vec<_>) = ordered.into_iter().partition(|release| {
            !failures.iter().any(|failure| {
                failure.media == self.id
                    && failure.hash.eq_ignore_ascii_case(&release.info_hash)
                    && failure.scope == *scope
            })
        });
        Plan {
            ranked: healthy.into_iter().chain(failed).cloned().collect(),
            ..self.clone()
        }
    }
    /// Every release that fits, in the order they would be tried, and then the ones with a codec
    /// that may not decode. For a viewer who wants to pick one.
    pub fn choices(&self) -> Vec<Choice> {
        let latest = self.latest();
        latest
            .ranked
            .iter()
            .map(|release| choice(release, false))
            .chain(latest.extra.iter().map(|release| choice(release, true)))
            .collect()
    }

    pub fn no_sources_reason(&self) -> &'static str {
        if !self.source_reports.is_empty()
            && self
                .source_reports
                .iter()
                .all(|report| report.answered == 0)
        {
            "Source providers could not be reached. Try again shortly."
        } else if self
            .source_reports
            .iter()
            .any(|report| report.failed > 0 || report.pending > 0)
        {
            "No matching source found in the providers that answered. Some providers are unavailable; try again shortly."
        } else {
            "No matching source is currently listed for this episode or film."
        }
    }

    /// Hashes for automatic delivery probing, with the same failure ordering and total
    /// trial cap as torrent opening. Explicit/fresh retries bypass failure demotion.
    pub fn candidate_hashes_with_failures(
        &self,
        count: usize,
        memory: &FailureMemory,
        fresh: bool,
    ) -> Vec<String> {
        self.demote_recent_failures(memory, fresh)
            .ranked
            .into_iter()
            .take(count.min(OFFERED))
            .map(|release| release.info_hash)
            .collect()
    }

    /// The info hashes of the first `count` releases, in the order they are tried.
    pub fn candidate_hashes(&self, count: usize) -> Vec<String> {
        candidates(&self.ranked)
            .into_iter()
            .take(count)
            .map(|release| release.info_hash.clone())
            .collect()
    }

    /// The plan without the releases that already failed to play. Accepts legacy bare hashes
    /// and file-scoped selection ids; malformed exclusions are ignored rather than broadened.
    pub fn without(&self, selections: &[String]) -> Plan {
        let excluded: HashSet<String> = selections
            .iter()
            .filter_map(|selection| normalized_selection(selection))
            .collect();
        let mut live = self.live.clone();
        if let Some(live) = &mut live {
            live.excluded.extend(excluded.iter().cloned());
        }
        let wanted = |release: &Release| !selection_excluded(&excluded, release);
        Plan {
            ranked: self.ranked.iter().filter(|r| wanted(r)).cloned().collect(),
            extra: self.extra.iter().filter(|r| wanted(r)).cloned().collect(),
            live,
            ..self.clone()
        }
    }

    /// The plan with only the release a viewer chose, wherever it was listed.
    pub fn only_selected(&self, info_hash: &str, file_idx: Option<usize>) -> Option<Plan> {
        let latest = self.latest();
        let release = latest.ranked.iter().chain(&latest.extra).find(|release| {
            release.info_hash.eq_ignore_ascii_case(info_hash)
                && file_idx.is_none_or(|file| release.file_idx == Some(file))
        })?;
        Some(Plan {
            ranked: vec![release.clone()],
            extra: Vec::new(),
            live: None,
            ..self.clone()
        })
    }

    /// Hash-only picking remains compatible with clients predating file-scoped addons.
    pub fn only(&self, info_hash: &str) -> Option<Plan> {
        self.only_selected(info_hash, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn late_plan(
        title: &str,
    ) -> (Plan, Arc<tokio::sync::Notify>, tokio::task::JoinHandle<()>) {
        late_plan_hash(title, &"b".repeat(40)).await
    }

    async fn late_plan_hash(
        title: &str,
        hash: &str,
    ) -> (Plan, Arc<tokio::sync::Notify>, tokio::task::JoinHandle<()>) {
        use axum::{Router, routing::get};
        use serde_json::json;
        let (hash, file_idx) = Release::parse_selection_id(hash).expect("test selection");
        let gate = Arc::new(tokio::sync::Notify::new());
        let held = gate.clone();
        let title = title.to_string();
        let app = Router::new()
            .route(
                "/first",
                get(|| async {
                    json!({"items":[{"title":"Show S01E01 1080p x264", "hash":"a".repeat(40)}]})
                        .to_string()
                }),
            )
            .route(
                "/late",
                get(move || {
                    let (gate, title, hash) = (held.clone(), title.clone(), hash.clone());
                    async move {
                        gate.notified().await;
                        let mut row = json!({"title":title, "hash":hash});
                        if let Some(file_idx) = file_idx {
                            row["fileIdx"] = json!(file_idx);
                        }
                        json!({"items":[row,
                        {"title":"Show S01E01 1080p x264", "hash":"a".repeat(40)}]})
                        .to_string()
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let defs: Vec<crate::definition::IndexerDef> = ["first", "late"].iter().map(|path|
            serde_json::from_value(json!({"id":format!("progressive-{path}-{base}"),"name":path,
                "kinds":["series"],"hosts":[base],"request":{"path":format!("/{path}")},
"response":{"format":"json","rows":"items","fields":{"title":"title","hash":"hash","file_idx":"fileIdx"}}})).unwrap()).collect();
        let chosen = defs.iter().collect::<Vec<_>>();
        let mut plan = plan_with(Vec::new(), Vec::new());
        let options = SearchOptions {
            kind: Kind::Series,
            imdb_id: None,
            mapped: None,
            season: Some(1),
            episode: Some(1),
        };
        let result = crate::indexers::search_in(
            &crate::sources::current(),
            &chosen,
            &Config {
                indexer_timeout_ms: 5000,
                search_ceiling_ms: 2000,
                ..Config::default()
            },
            &["Show S01E01".into()],
            &options,
            |rows| !rank_releases(rows, 1, "Show", &[], Some(1)).is_empty(),
        )
        .await;
        assert_eq!(result.releases.len(), 1);
        plan.ranked = rank_releases(&result.releases, 1, "Show", &[], Some(1));
        plan.source_reports = result.sources;
        plan.live = Some(PlanLive {
            results: result.live.unwrap(),
            identity: PlanIdentity::new(&plan.id, &plan.detail),
            excluded: HashSet::new(),
        });
        (plan, gate, server)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn late_indexer_candidate_delivers_bytes_from_a_loopback_swarm() {
        use librqbit::{
            AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session,
            SessionOptions, create_torrent, spawn_utils::BlockingSpawner,
        };
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let base =
            std::env::temp_dir().join(format!("akidev-progressive-swarm-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join("Show S01E01.mkv");
        std::fs::write(&path, vec![42u8; 128 * 1024]).unwrap();
        let seeder = Session::new_with_opts(
            base.clone(),
            SessionOptions {
                dht: None,
                disable_trackers: true,
                persistence: None,
                fastresume: false,
                listen: Some(ListenerOptions {
                    listen_addr: "127.0.0.1:0".parse().unwrap(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let torrent = create_torrent(
            &path,
            CreateTorrentOptions {
                name: None,
                trackers: Vec::new(),
                piece_length: Some(16 * 1024),
            },
            &BlockingSpawner::new(2),
        )
        .await
        .unwrap();
        let handle = seeder
            .add_torrent(
                AddTorrent::from_bytes(torrent.as_bytes().unwrap()),
                Some(AddTorrentOptions {
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap()
            .into_handle()
            .unwrap();
        handle.wait_until_completed().await.unwrap();
        let hash = torrent.info_hash().as_string();
        let (plan, gate, server) = late_plan_hash("Show S01E01 720p x264", &hash).await;
        let leech = Session::new_with_opts(
            base.join("leech"),
            SessionOptions {
                dht: None,
                disable_trackers: true,
                persistence: None,
                fastresume: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            2,
            Instant::now() + Duration::from_secs(10),
            |release| {
                let (gate, leech) = (gate.clone(), leech.clone());
                let peer = seeder.listen_addr().unwrap();
                async move {
                    if release.info_hash == "a".repeat(40) {
                        gate.notify_one();
                        return Err(());
                    }
                    let handle = leech
                        .add_torrent(
                            AddTorrent::from_url(&release.magnet),
                            Some(AddTorrentOptions {
                                initial_peers: Some(vec![peer]),
                                ..Default::default()
                            }),
                        )
                        .await
                        .map_err(|_| ())?
                        .into_handle()
                        .ok_or(())?;
                    handle.wait_until_completed().await.map_err(|_| ())?;
                    Ok(handle)
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(result.0.info_hash, hash);
        assert_eq!(
            std::fs::read(base.join("leech/Show S01E01.mkv")).unwrap(),
            vec![42u8; 128 * 1024]
        );
        server.abort();
        leech.stop().await;
        seeder.stop().await;
    }

    #[tokio::test]
    async fn late_exact_source_wins_after_initial_failure_without_retrying_hash() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (plan, gate, server) = late_plan("Show S01E01 720p x264").await;
        let attempted = Mutex::new(Vec::new());
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            3,
            Instant::now() + Duration::from_secs(2),
            |release| {
                attempted.lock().unwrap().push(release.info_hash.clone());
                let gate = gate.clone();
                async move {
                    if release.info_hash == "a".repeat(40) {
                        gate.notify_one();
                        Err(())
                    } else {
                        Ok("ready")
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(result.0.info_hash, "b".repeat(40));
        assert_eq!(*attempted.lock().unwrap(), ["a".repeat(40), "b".repeat(40)]);
        server.abort();
    }

    #[tokio::test]
    async fn late_source_races_while_initial_attempt_is_still_pending() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (plan, gate, server) = late_plan("Show S01E01 720p x264").await;
        let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct Guard(Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            2,
            Instant::now() + Duration::from_secs(2),
            |release| {
                let (gate, dropped) = (gate.clone(), dropped.clone());
                async move {
                    if release.info_hash == "a".repeat(40) {
                        let _guard = Guard(dropped);
                        gate.notify_one();
                        futures::future::pending::<()>().await;
                    }
                    Ok(())
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(result.0.info_hash, "b".repeat(40));
        assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn wrong_late_title_episode_season_year_and_mutated_catalog_are_rejected() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        for title in [
            "Other Show S01E01 x264",
            "Show S01E02 x264",
            "Show S02E01 x264",
            "Show 1999 S01E01 x264",
        ] {
            let (mut plan, gate, server) = late_plan(title).await;
            if title.contains("1999") {
                plan.detail.media.year = Some(2024);
                plan.live.as_mut().unwrap().identity = PlanIdentity::new(&plan.id, &plan.detail);
            }
            gate.notify_one();
            let latest = plan.wait_for_late().await.unwrap();
            assert!(
                !latest
                    .ranked
                    .iter()
                    .any(|release| release.info_hash == "b".repeat(40)),
                "{title}"
            );
            plan.detail.media.title = "Changed Catalog Title".into();
            assert!(plan.wait_for_late().await.is_none());
            server.abort();
        }
    }

    #[tokio::test]
    async fn pinning_and_repeated_exclusions_cannot_substitute_late_hashes() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (plan, gate, server) = late_plan("Show S01E01 720p x264").await;
        let pinned = plan.only(&"a".repeat(40)).unwrap();
        assert!(pinned.live.is_none());
        let excluded = plan.without(&["A".repeat(40)]).without(&["B".repeat(40)]);
        gate.notify_one();
        assert!(excluded.wait_for_late().await.unwrap().ranked.is_empty());
        let tried = Mutex::new(Vec::new());
        assert!(
            race_plan(
                &pinned,
                &FailureMemory::default(),
                false,
                3,
                Instant::now() + Duration::from_secs(1),
                |release| {
                    tried.lock().unwrap().push(release.info_hash);
                    async { Err::<(), _>(()) }
                }
            )
            .await
            .is_err()
        );
        assert_eq!(*tried.lock().unwrap(), ["a".repeat(40)]);
        server.abort();
    }

    #[tokio::test]
    async fn progressive_race_enforces_total_trial_and_concurrent_open_caps() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let plan = plan_with(
            (0..40)
                .map(|index| Release {
                    info_hash: format!("{index:040x}"),
                    ..release("Show S01E01 x264", 1)
                })
                .collect(),
            Vec::new(),
        );
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let tried = AtomicUsize::new(0);
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            3,
            Instant::now() + Duration::from_secs(3),
            |_| async {
                tried.fetch_add(1, Ordering::SeqCst);
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(1)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                Err::<(), _>(())
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(tried.load(Ordering::SeqCst), OFFERED);
        assert!(peak.load(Ordering::SeqCst) <= 3);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn late_wait_respects_deadline_and_can_be_cancelled_then_resumed() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (plan, gate, server) = late_plan("Show S01E01 720p x264").await;
        let start = Instant::now();
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            1,
            start + Duration::from_millis(30),
            |_| async { Err::<(), _>(()) },
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            "Timed out waiting for a playable source."
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), plan.wait_for_late())
                .await
                .is_err()
        );
        gate.notify_one();
        let latest = tokio::time::timeout(Duration::from_secs(1), plan.wait_for_late())
            .await
            .unwrap()
            .unwrap();
        assert!(
            latest
                .ranked
                .iter()
                .any(|release| release.info_hash == "b".repeat(40))
        );
        assert_eq!(latest.ranked.len(), 2);
        let (pending, pending_gate, pending_server) =
            late_plan("Other Show S01E01 720p x264").await;
        let held = pending
            .latest_until(Instant::now() + Duration::from_millis(20))
            .await;
        assert_eq!(held.ranked.len(), 1, "timeout keeps the initial evidence");
        pending_gate.notify_one();
        server.abort();
        pending_server.abort();
    }

    #[tokio::test]
    async fn a_finished_late_tail_returns_file_scoped_rows_even_when_the_initial_snapshot_is_empty()
    {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (mut plan, gate, server) =
            late_plan_hash("Show S01E01 720p x264", &format!("{}:0", "b".repeat(40))).await;
        plan.ranked.clear();
        assert!(plan.ranked.is_empty());
        gate.notify_one();
        let latest = plan
            .latest_until(Instant::now() + Duration::from_secs(1))
            .await;
        let selected = latest.only_selected(&"b".repeat(40), Some(0)).unwrap();
        assert_eq!(
            selected.ranked[0].selection_id(),
            format!("{}:0", "b".repeat(40))
        );
        assert!(
            latest
                .choices()
                .iter()
                .any(|choice| choice.id == "a".repeat(40))
        );
        server.abort();
    }

    fn release(title: &str, seeders: u32) -> Release {
        Release {
            title: title.into(),
            info_hash: "a".repeat(40),
            file_idx: None,
            magnet: String::new(),
            size_bytes: None,
            seeders: Some(seeders),
            source: "test".into(),
            sources: vec!["test".into()],
        }
    }
    #[tokio::test]
    async fn file_scoped_rows_survive_alias_merges_and_are_raced_separately() {
        let mut first_file = release("Show S01E01 1080p x264", 5);
        first_file.file_idx = Some(0);
        let mut second_file = release("Show S01E01 720p HEVC", 5);
        second_file.file_idx = Some(1);
        let first = (
            Vec::new(),
            vec![first_file.clone()],
            Vec::new(),
            Vec::new(),
            None,
        );
        let second = (
            Vec::new(),
            vec![second_file.clone()],
            Vec::new(),
            Vec::new(),
            None,
        );
        assert_eq!(merge_searches(first, second).1.len(), 2);

        let plan = plan_with(vec![first_file, second_file], Vec::new());
        assert_eq!(plan.choices().len(), 2);
        let result = race_plan(
            &plan,
            &FailureMemory::default(),
            false,
            1,
            Instant::now() + Duration::from_secs(1),
            |release| async move {
                if release.file_idx == Some(0) {
                    Err(())
                } else {
                    Ok(())
                }
            },
        )
        .await;
        assert_eq!(result.unwrap().0.file_idx, Some(1));
    }

    #[tokio::test]
    async fn late_file_rows_and_explicitly_pinned_uncommon_rows_keep_their_identity() {
        let _guard = crate::indexers::tests::NETWORK_TEST.lock().await;
        let (initial, gate, server) =
            late_plan_hash("Show S01E01 720p x264", &format!("{}:1", "b".repeat(40))).await;
        let mut original = release("Show S01E01 1080p HEVC", 3);
        original.file_idx = Some(0);
        let plan = Plan {
            live: initial.live.clone(),
            ..plan_with(initial.ranked, vec![original])
        };
        // The fixture's live identity belongs to the same detail.
        gate.notify_one();
        let latest = plan.wait_for_late().await.unwrap();
        assert!(latest.ranked.iter().any(|row| row.file_idx == Some(1)));
        assert!(latest.extra.iter().any(|row| row.file_idx == Some(0)));
        let picked = latest.only_selected(&"a".repeat(40), Some(0)).unwrap();
        assert_eq!(picked.ranked[0].file_idx, Some(0));
        assert_eq!(picked.ranked[0].title, "Show S01E01 1080p HEVC");
        assert!(picked.live.is_none());
        assert!(latest.only_selected(&"a".repeat(40), Some(2)).is_none());
        server.abort();
    }

    fn titles(found: Vec<&Release>) -> Vec<&str> {
        found
            .into_iter()
            .map(|release| release.title.as_str())
            .collect()
    }

    #[test]
    fn recent_failure_demotes_but_never_discards_and_success_clears() {
        let memory = FailureMemory::default();
        let plan = plan_with(vec![named("failed", 'a'), named("other", 'b')], Vec::new());
        memory.record_failure(&plan.id, &"A".repeat(40));
        let ordered = plan.demote_recent_failures(&memory, false);
        assert_eq!(titles(ordered.ranked.iter().collect()), ["other", "failed"]);
        assert_eq!(ordered.ranked.len(), 2);
        assert_eq!(
            titles(
                plan.demote_recent_failures(&memory, true)
                    .ranked
                    .iter()
                    .collect()
            ),
            ["failed", "other"]
        );
        let pinned = plan.only(&"a".repeat(40)).unwrap();
        assert_eq!(
            pinned.demote_recent_failures(&memory, false).ranked.len(),
            1
        );
        memory.clear_failure(&plan.id, &"a".repeat(40));
        assert_eq!(
            titles(
                plan.demote_recent_failures(&memory, false)
                    .ranked
                    .iter()
                    .collect()
            ),
            ["failed", "other"]
        );
    }

    #[test]
    fn failures_are_scoped_to_media_hash_file_and_capability() {
        let memory = FailureMemory::default();
        let plan = plan_with(vec![named("a", 'a'), named("b", 'b')], Vec::new());
        let scope = FailureScope {
            file_index: Some(1),
            capability: Some("h264".into()),
        };
        memory.record_failure_scoped(&plan.id, &"a".repeat(40), &scope);
        assert_eq!(
            plan.demote_recent_failures_scoped(&memory, false, &scope)
                .ranked[0]
                .title,
            "b"
        );
        for other in [
            FailureScope::default(),
            FailureScope {
                file_index: Some(2),
                ..scope.clone()
            },
            FailureScope {
                capability: Some("hevc".into()),
                ..scope.clone()
            },
        ] {
            assert_eq!(
                plan.demote_recent_failures_scoped(&memory, false, &other)
                    .ranked[0]
                    .title,
                "a"
            );
        }
        let mut other_episode = plan.clone();
        other_episode.id = "series-1-s1e2".into();
        assert_eq!(
            other_episode
                .demote_recent_failures_scoped(&memory, false, &scope)
                .ranked[0]
                .title,
            "a"
        );
        memory.clear_failure(&plan.id, &"a".repeat(40));
        assert_eq!(
            plan.demote_recent_failures_scoped(&memory, false, &scope)
                .ranked[0]
                .title,
            "b"
        );
        memory.clear_failure_scoped(&plan.id, &"a".repeat(40), &scope);
        assert_eq!(
            plan.demote_recent_failures_scoped(&memory, false, &scope)
                .ranked[0]
                .title,
            "a"
        );
    }

    #[test]
    fn expired_failures_and_invalid_inputs_are_not_remembered() {
        let memory = FailureMemory::default();
        let plan = plan_with(vec![named("a", 'a'), named("b", 'b')], Vec::new());
        let now = Instant::now();
        memory.record_at(&plan.id, &"a".repeat(40), &FailureScope::default(), now);
        assert_eq!(
            plan.demote_at(
                &memory,
                false,
                &FailureScope::default(),
                now + FAILURE_TTL - Duration::from_nanos(1)
            )
            .ranked[0]
                .title,
            "b"
        );
        assert_eq!(
            plan.demote_at(&memory, false, &FailureScope::default(), now + FAILURE_TTL)
                .ranked[0]
                .title,
            "a"
        );
        memory.record_failure("invalid", &"a".repeat(40));
        memory.record_failure(&plan.id, "not a hash");
        memory.record_failure_scoped(
            &plan.id,
            &"a".repeat(40),
            &FailureScope {
                file_index: None,
                capability: Some("x".repeat(129)),
            },
        );
        assert!(memory.0.lock().unwrap().is_empty());
    }

    #[test]
    fn automatic_trial_budget_is_bounded_independently_of_requested_count() {
        let memory = FailureMemory::default();
        let ranked = (0..OFFERED + 10)
            .map(|index| Release {
                info_hash: format!("{index:040x}"),
                ..release("trial", 1)
            })
            .collect();
        let plan = plan_with(ranked, Vec::new());
        assert_eq!(
            plan.candidate_hashes_with_failures(usize::MAX, &memory, false)
                .len(),
            OFFERED
        );
        assert_eq!(
            plan.candidate_hashes_with_failures(2, &memory, false).len(),
            2
        );
    }

    #[test]
    fn failure_memory_is_bounded_and_deduplicated() {
        let memory = FailureMemory::default();
        let now = Instant::now();
        for index in 0..(FAILURE_LIMIT + 10) {
            memory.record_at(
                "series-1-s1e1",
                &format!("{index:040x}"),
                &FailureScope::default(),
                now,
            );
        }
        let failures = memory.0.lock().unwrap();
        assert_eq!(failures.len(), FAILURE_LIMIT);
        assert_eq!(failures.front().unwrap().hash, format!("{:040x}", 10));
        drop(failures);
        memory.record_at(
            "series-1-s1e1",
            &format!("{:040x}", FAILURE_LIMIT + 9),
            &FailureScope::default(),
            now,
        );
        assert_eq!(memory.0.lock().unwrap().len(), FAILURE_LIMIT);
    }

    #[test]
    fn a_failed_seeded_candidate_follows_even_a_valid_zero_seed_candidate() {
        let memory = FailureMemory::default();
        let plan = plan_with(
            vec![
                named("failed", 'a'),
                Release {
                    seeders: Some(0),
                    ..named("valid", 'b')
                },
            ],
            Vec::new(),
        );
        memory.record_failure(&plan.id, &"a".repeat(40));
        assert_eq!(
            titles(
                plan.demote_recent_failures(&memory, false)
                    .ranked
                    .iter()
                    .collect()
            ),
            ["valid", "failed"]
        );
    }

    #[test]
    fn a_release_nobody_seeds_is_tried_last() {
        let ranked = [
            release("dead", 0),
            release("alive", 4),
            release("also alive", 1),
        ];
        assert_eq!(titles(candidates(&ranked)), ["alive", "also alive", "dead"]);
    }

    #[test]
    fn unseeded_releases_are_tried_when_they_are_all_there_is() {
        let ranked = [release("a", 0), release("b", 0)];
        assert_eq!(titles(candidates(&ranked)), ["a", "b"]);
    }

    #[test]
    fn unknown_health_is_tried_before_observed_zero() {
        let dead = release("dead", 0);
        let mut unknown = release("unknown", 0);
        unknown.seeders = None;
        let alive = release("alive", 4);
        assert_eq!(
            titles(candidates(&[dead, unknown, alive])),
            ["alive", "unknown", "dead"]
        );
    }

    #[test]
    fn a_candidate_starts_when_enough_ahead_have_failed() {
        // Three at once: the first three wait for everyone ahead, the fourth for one failure.
        let needed: Vec<usize> = (0..6).map(|index| failures_to_start(index, 3)).collect();
        assert_eq!(needed, [0, 1, 2, 1, 2, 3]);
    }

    #[test]
    fn every_release_is_a_candidate() {
        let ranked = [
            release("a", 9),
            release("b", 8),
            release("c", 7),
            release("d", 6),
        ];
        assert_eq!(titles(candidates(&ranked)), ["a", "b", "c", "d"]);
        assert!(candidates(&[]).is_empty());
    }

    #[test]
    fn provider_failures_are_not_reported_as_no_matches() {
        let mut plan = plan_with(Vec::new(), Vec::new());
        plan.source_reports = vec![SourceReport {
            source: "test".into(),
            requests: 1,
            failed: 1,
            ..SourceReport::default()
        }];
        assert_eq!(
            plan.no_sources_reason(),
            "Source providers could not be reached. Try again shortly."
        );
        plan.source_reports[0].answered = 1;
        plan.source_reports[0].failed = 0;
        assert_eq!(
            plan.no_sources_reason(),
            "No matching source is currently listed for this episode or film."
        );
    }

    #[test]
    fn alias_searches_preserve_uncommon_choices_and_prior_results() {
        let first = (
            vec!["original".into()],
            vec![named("first", 'a')],
            Vec::new(),
            Vec::new(),
            None,
        );
        let second = (
            vec!["alias".into()],
            Vec::new(),
            vec![named("alias HEVC", 'b')],
            Vec::new(),
            None,
        );
        let merged = merge_searches(first, second);
        assert_eq!(merged.0, ["original", "alias"]);
        assert_eq!(merged.1[0].title, "first");
        assert_eq!(merged.2[0].title, "alias HEVC");
    }

    #[test]
    fn releases_that_failed_to_play_are_left_out() {
        let mut a = release("a", 9);
        a.info_hash = "a".repeat(40);
        let mut b = release("b", 8);
        b.info_hash = "b".repeat(40);
        let plan = plan_with(vec![a, b], Vec::new());
        let left = plan.without(&["A".repeat(40)]);
        assert_eq!(titles(candidates(&left.ranked)), ["b"]);
    }

    #[test]
    fn scoped_exclusions_keep_sibling_files_but_legacy_hashes_exclude_the_swarm() {
        let mut first = release("first", 9);
        first.file_idx = Some(0);
        let mut second = release("second", 8);
        second.file_idx = Some(1);
        let plan = plan_with(vec![first.clone(), second.clone()], Vec::new());
        let left = plan.without(&[first.selection_id().to_ascii_uppercase()]);
        assert_eq!(left.ranked, vec![second.clone()]);
        assert!(plan.without(&[first.info_hash.clone()]).ranked.is_empty());
        // An invalid suffix never becomes a hash-wide exclusion.
        assert_eq!(
            plan.without(&[format!("{}:bad", first.info_hash)])
                .ranked
                .len(),
            2
        );
    }

    fn card(title: &str, variants: &[&str], year: Option<i32>) -> crate::model::Media {
        let mut media = plan_with(Vec::new(), Vec::new()).detail.media;
        media.title = title.into();
        media.title_variants = variants.iter().map(|v| v.to_string()).collect();
        media.year = year;
        media
    }

    #[test]
    fn anilist_movie_format_uses_film_discovery_and_file_selection() {
        let mut media = card("Rascal Does Not Dream of a Dear Friend", &[], Some(2026));
        media.kind = Kind::Anime;
        media.format = Some("MOVIE".into());
        assert!(is_film(&media));
        let mut plan = plan_with(Vec::new(), Vec::new());
        plan.parsed = media_id::parse("anime-199340-ep1").unwrap();
        plan.detail.media = media.clone();
        assert_eq!(plan.playback_episode(), None);
        media.format = Some("movie".into());
        assert!(is_film(&media));
        media.format = Some("TV".into());
        assert!(!is_film(&media));
        plan.detail.media = media;
        assert_eq!(plan.playback_episode(), Some(1));
    }

    #[test]
    fn a_title_is_asked_whole_and_then_by_its_longer_parts() {
        assert_eq!(
            alias_queries("Tomei na Yoru ni Kakeru Kimi to, Me ni Mienai Koi wo Shita"),
            [
                "Tomei na Yoru ni Kakeru Kimi to, Me ni Mienai Koi wo Shita",
                "Tomei na Yoru ni Kakeru Kimi to",
                "Me ni Mienai Koi wo Shita"
            ]
        );
        assert_eq!(alias_queries("Naruto"), ["Naruto"]);
    }

    #[test]
    fn an_anime_with_the_same_title_and_about_the_same_year_lends_its_other_names() {
        let show = card(
            "Tomei na Yoru ni Kakeru Kimi to, Me ni Mienai Koi wo Shita",
            &[],
            Some(2026),
        );
        let anime = card(
            "Love Unseen Beneath the Clear Night Sky",
            &["Toumei na Yoru ni Kakeru Kimi to, Me ni Mienai Koi wo Shita."],
            Some(2025),
        );
        assert!(is_same_title(&show, &anime, &known_names(&anime)));
        // Another show that only looks similar, or the same name from decades away, is not it.
        let other = card("Another Show", &["Tomei na Yoru"], Some(2025));
        assert!(!is_same_title(&show, &other, &known_names(&other)));
        let old = card(
            "Love Unseen",
            &["Tomei na Yoru ni Kakeru Kimi to Me ni Mienai Koi wo Shita"],
            Some(1999),
        );
        assert!(!is_same_title(&show, &old, &known_names(&old)));
    }

    fn plan_with(ranked: Vec<Release>, extra: Vec<Release>) -> Plan {
        use crate::model::Media;
        Plan {
            id: "series-1-s1e1".into(),
            parsed: media_id::parse("series-1-s1e1").unwrap(),
            detail: Detail {
                media: Media {
                    id: "series-1".into(),
                    kind: Kind::Series,
                    title: "Show".into(),
                    title_variants: Vec::new(),
                    poster: None,
                    banner: None,
                    description: None,
                    format: None,
                    year: None,
                    episodes: None,
                    runtime_minutes: None,
                    status: None,
                    score: None,
                    popularity: 0,
                    genres: Vec::new(),
                    is_adult: false,
                },
                studios: Vec::new(),
                trailer: None,
                episodes: Vec::new(),
                cast: Vec::new(),
                related: Vec::new(),
                recommended: Vec::new(),
                imdb_id: None,
            },
            terms: Vec::new(),
            source_reports: Vec::new(),
            ranked,
            extra,
            live: None,
        }
    }

    fn catalogue_episode(
        season: Option<u32>,
        number: u32,
        airdate: Option<&str>,
    ) -> crate::model::Episode {
        crate::model::Episode {
            id: match season {
                Some(season) => format!("series-1-s{season}e{number}"),
                None => format!("anime-1-ep{number}"),
            },
            season,
            number,
            name: None,
            airdate: airdate.map(str::to_string),
        }
    }

    #[test]
    fn public_request_validation_rejects_invalid_identity_and_episode_bounds() {
        let mut plan = plan_with(Vec::new(), Vec::new());
        assert!(plan.validate_request().is_ok());
        assert!(
            Streamer::validate_request(&format!("series-{}-s1e1", "1".repeat(129)), &plan.detail)
                .is_err()
        );
        for id in [
            "invalid",
            "series-1",
            "series-1-s1e0",
            "series-2-s1e1",
            "movie-1",
        ] {
            assert!(
                Streamer::validate_request(id, &plan.detail).is_err(),
                "{id}"
            );
        }
        plan.detail.media.episodes = Some(12);
        assert!(Streamer::validate_request("series-1-s1e12", &plan.detail).is_ok());
        assert!(Streamer::validate_request("series-1-s1e13", &plan.detail).is_err());
        plan.parsed.episode = Some(2);
        assert!(plan.validate_request().is_err());
        plan.detail.media.kind = Kind::Movie;
        assert!(Streamer::validate_request("series-1-s1e1", &plan.detail).is_err());
    }

    #[test]
    fn series_catalogue_rejects_missing_pairs_but_accepts_supported_specials() {
        let mut detail = plan_with(Vec::new(), Vec::new()).detail;
        detail.media.episodes = Some(2);
        detail.episodes = vec![
            catalogue_episode(Some(1), 1, None),
            catalogue_episode(Some(1), 2, None),
            catalogue_episode(Some(0), 5, None),
        ];
        assert!(Streamer::validate_request("series-1-s1e2", &detail).is_ok());
        assert!(Streamer::validate_request("series-1-s0e5", &detail).is_ok());
        for id in ["series-1-s2e1", "series-1-s0e1", "series-1-s1e3"] {
            assert!(Streamer::validate_request(id, &detail).is_err(), "{id}");
        }
    }

    #[test]
    fn unknown_counts_and_incomplete_metadata_do_not_invent_limits() {
        let mut detail = plan_with(Vec::new(), Vec::new()).detail;
        for count in [None, Some(0)] {
            detail.media.episodes = count;
            assert!(Streamer::validate_request("series-1-s9e999", &detail).is_ok());
        }
        detail.episodes = vec![catalogue_episode(None, 1, None)];
        assert!(Streamer::validate_request("series-1-s9e999", &detail).is_ok());
        detail.media.kind = Kind::Anime;
        detail.media.id = "anime-1".into();
        detail.media.episodes = Some(12);
        assert!(Streamer::validate_request("anime-1-ep12", &detail).is_ok());
        assert!(Streamer::validate_request("anime-1-ep13", &detail).is_err());
        detail.media.episodes = None;
        assert!(Streamer::validate_request("anime-1-ep999", &detail).is_ok());
        detail.media.kind = Kind::Movie;
        detail.media.id = "movie-1".into();
        assert!(Streamer::validate_request("movie-1", &detail).is_ok());
    }

    #[test]
    fn airdate_validation_uses_utc_days_and_ignores_unknown_or_invalid_dates() {
        let mut detail = plan_with(Vec::new(), Vec::new()).detail;
        let today = crate::calendar::parse_date_ms("2026-02-28").unwrap();
        for (date, allowed) in [
            (Some("2026-02-27"), true),
            (Some("2026-02-28T23:59:59Z"), true),
            (Some("2026-03-01T00:00:00Z"), false),
            (Some("2026-02-30"), true),
            (Some("soon"), true),
            (None, true),
        ] {
            detail.episodes = vec![catalogue_episode(Some(1), 1, date)];
            assert_eq!(
                validate_request_at("series-1-s1e1", &detail, today).is_ok(),
                allowed,
                "{date:?}"
            );
            assert_eq!(
                validate_request_at(
                    "series-1-s1e1",
                    &detail,
                    today + crate::calendar::DAY_MS - 1
                )
                .is_ok(),
                allowed
            );
        }
        detail.episodes[0].airdate = Some("9999-12-31".into());
        let mut plan = plan_with(Vec::new(), Vec::new());
        plan.detail = detail;
        assert_eq!(
            plan.validate_request().unwrap_err(),
            "This episode has not aired yet."
        );
    }

    fn named(title: &str, hash: char) -> Release {
        Release {
            info_hash: hash.to_string().repeat(40),
            ..release(title, 3)
        }
    }

    #[test]
    fn what_can_be_told_from_a_name_is_listed_and_the_uncommon_come_last() {
        let plan = plan_with(
            vec![named("Show S01E01 1080p WEB-DL x264 AAC", 'a')],
            vec![named("Show S01E01 2160p HEVC FLAC", 'b')],
        );
        let choices = plan.choices();
        assert_eq!(choices.len(), 2);
        assert_eq!(
            (
                choices[0].resolution,
                choices[0].codec,
                choices[0].audio,
                choices[0].uncommon
            ),
            (Some(1080), Some("h264"), Some("compressed"), false)
        );
        assert_eq!(
            (
                choices[1].resolution,
                choices[1].codec,
                choices[1].audio,
                choices[1].uncommon
            ),
            (Some(2160), Some("hevc"), Some("lossless"), true)
        );
        assert_eq!(choices[0].id, "a".repeat(40));
    }

    #[test]
    fn a_release_that_was_picked_is_the_only_one_tried_wherever_it_was_listed() {
        let plan = plan_with(
            vec![named("one", 'a'), named("two", 'b')],
            vec![named("three hevc", 'c')],
        );
        for (hash, title) in [('b', "two"), ('c', "three hevc")] {
            let only = plan
                .only(&hash.to_string().repeat(40).to_uppercase())
                .unwrap();
            assert_eq!(only.ranked.len(), 1);
            assert_eq!(only.ranked[0].title, title);
            assert!(only.extra.is_empty());
        }
        assert!(plan.only(&"d".repeat(40)).is_none());
    }

    #[test]
    fn selected_file_does_not_downgrade_to_another_episode_title() {
        let mut episode = named("Show S01E01 1080p x264", 'a');
        episode.file_idx = Some(0);
        let mut wrong = named("Show S01E02 1080p x264", 'a');
        wrong.file_idx = Some(1);
        let plan = plan_with(vec![episode, wrong], Vec::new());
        let selected = plan.only_selected(&"A".repeat(40), Some(0)).unwrap();
        assert_eq!(selected.ranked[0].file_idx, Some(0));
        assert_eq!(selected.ranked[0].title, "Show S01E01 1080p x264");
        assert!(plan.only_selected(&"a".repeat(40), Some(9)).is_none());
        assert_eq!(
            plan.only(&"a".repeat(40)).unwrap().ranked[0].file_idx,
            Some(0)
        );
    }

    #[cfg(feature = "code-plugins")]
    #[tokio::test]
    async fn a_stream_service_with_its_key_gives_the_link_the_player_opens_and_one_without_a_key_is_left_alone()
     {
        use crate::services::tests::{
            Fake, HASH, KEY, LENIENT, release as service_release, service, torbox,
        };
        let fake = std::sync::Arc::new(Mutex::new(Fake {
            cached: true,
            ..Fake::default()
        }));
        let base = torbox(fake.clone()).await;
        let services = [service(&base, &["127.0.0.1"])];
        let plan = plan_with(vec![service_release()], Vec::new());

        // No key: the service is not asked at all.
        let nothing = from_services_in(&services, &Config::default(), &plan, LENIENT).await;
        assert!(nothing.is_none());
        assert!(!fake.lock().unwrap().added);

        // With its key: the link for this episode, named like any other resolution.
        let mut config = Config::default();
        config.service_keys.insert("torbox".into(), KEY.into());
        let found = from_services_in(&services, &config, &plan, LENIENT).await;
        match found {
            Some(Resolution::Ready {
                url,
                release,
                stream,
            }) => {
                assert!(url.ends_with("/cdn/4.mkv"), "{url}");
                assert_eq!(release, "Show S01 1080p WEB");
                assert_eq!(stream.info_hash, HASH);
                assert_eq!(stream.selection.as_deref(), Some(HASH));
            }
            other => panic!("expected a link, got {other:?}"),
        }
    }
}
