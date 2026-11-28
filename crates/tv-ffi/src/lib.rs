//! What the Android TV app asks of the Rust side. Plain blocking calls: Kotlin runs them off the
//! main thread. The TV holds the TorBox key itself, so the player takes the link straight from
//! TorBox and nothing goes through a server or onto the disk.

mod torbox;

use akidev_core::config::Config;
use akidev_core::media_id;
use akidev_core::model::Release;
use akidev_core::model::{Filters, Kind, Media, Shape, Sort};
use akidev_core::releases::{Codec, VideoFile, parse_codec, parse_resolution, pick_for_identity};
use akidev_core::screens::RowStyle;
use akidev_core::{providers, search as catalogue_search, sources, stream, subtitles};
use std::path::PathBuf;
use std::sync::{Once, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;
use torbox::TorBox;

uniffi::setup_scaffolding!();

/// Each row of the home screen holds this many titles.
const ROW_SIZE: u32 = 20;
/// How many cached releases are tried before giving up: each costs a few calls.
const MOST_TRIES: usize = 4;
/// How many copies the player's panel lists.
const MOST_SOURCES: usize = 10;
/// The longest to look for a playable file.
const FIND_FOR: Duration = Duration::from_secs(75);
/// The longest to ask the subtitle sources.
const SUBTITLES_FOR: Duration = Duration::from_secs(8);

#[derive(uniffi::Record)]
pub struct Title {
    pub id: String,
    pub name: String,
    pub year: Option<i32>,
    pub poster: Option<String>,
    pub score: Option<u32>,
    /// What the home hero shows while this title has focus.
    pub banner: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    /// For a title that is not out yet, the words to show: "Coming 27 January 2027". It cannot be played.
    pub coming_soon: Option<String>,
}

#[derive(uniffi::Record)]
pub struct Row {
    pub name: String,
    /// `movie`, `series` or `anime` (or a kind a plugin brought), for the tabs of the home screen.
    pub kind: String,
    /// How the row is ordered and which genre it holds, so "View all" opens the same list in full.
    pub sort: String,
    pub genre: Option<String>,
    /// A "Top 10": the screen draws a big number beside each title.
    pub ranked: bool,
    pub titles: Vec<Title>,
}

#[derive(uniffi::Record)]
pub struct Episode {
    /// The id to play.
    pub id: String,
    pub season: Option<u32>,
    pub number: u32,
    pub name: Option<String>,
    /// "2008-02-10", when the provider knows it.
    pub airdate: Option<String>,
    /// False for an episode that has not aired yet: it cannot be played.
    pub released: bool,
}

#[derive(uniffi::Record)]
pub struct Detail {
    pub title: Title,
    /// The provider's word for it ("RELEASING", "FINISHED"), as the catalogue gives it.
    pub status: Option<String>,
    pub runtime_minutes: Option<u32>,
    /// What to play for a film; for a series, play one of `episodes`.
    pub play_id: Option<String>,
    pub episodes: Vec<Episode>,
}

/// A file ready to play. The link carries the key: do not show or log it.
#[derive(uniffi::Record)]
pub struct Stream {
    pub url: String,
    /// Which release it came from, so a failed play can ask for another one (`skip` of `find_stream`).
    pub hash: String,
}

/// A subtitle file from outside the video.
#[derive(uniffi::Record)]
pub struct Subtitle {
    pub language: String,
    pub label: String,
    pub url: String,
}

#[derive(Debug, uniffi::Error)]
pub enum TvError {
    Failed { reason: String },
}

impl std::fmt::Display for TvError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Failed { reason } = self;
        write!(out, "{reason}")
    }
}

/// A line in the Android log (`adb logcat -s AkiTV`), with the time since `started`. Never put the key or a link in it.
fn note(started: std::time::Instant, text: &str) {
    let line = format!("{:>6} ms  {text}", started.elapsed().as_millis());
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            fn __android_log_write(
                priority: i32,
                tag: *const std::ffi::c_char,
                text: *const std::ffi::c_char,
            ) -> i32;
        }
        if let Ok(line) = std::ffi::CString::new(line) {
            unsafe { __android_log_write(4, c"AkiTV".as_ptr(), line.as_ptr()) };
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}

fn failed(reason: impl ToString) -> TvError {
    TvError::Failed {
        reason: reason.to_string(),
    }
}

/// Makes the calling thread nicer to the others (nice 10). Linux sets this for one thread when `who` is 0.
fn lower_priority() {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            fn setpriority(which: i32, who: u32, priority: i32) -> i32;
        }
        const PRIO_PROCESS: i32 = 0;
        unsafe {
            setpriority(PRIO_PROCESS, 0, 10);
        }
    }
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        // Two workers, at a lower priority than the screen and the keyboard: the TV has three weak cores, and a search in the
        // background (TLS handshakes, JSON) made the keyboard stutter while the viewer typed.
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("aki-engine")
            .on_thread_start(lower_priority)
            .enable_all()
            .build()
            .expect("a tokio runtime")
    })
}

/// Call once at start. Uses the newest definitions saved by an earlier run, and keeps them fresh.
#[uniffi::export]
pub fn start(dir: String) {
    // The activity can be made again (a change of language, for one): the updater must run only once.
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let dir = PathBuf::from(dir);
        let _ = DEFINITIONS_DIR.set(dir.clone());
        CHECKED_AT.store(now_seconds(), std::sync::atomic::Ordering::Relaxed);
        sources::load_saved(&dir);
        // New definitions are fetched in the background and saved; this run keeps what it started with (the next start uses them).
        sources::apply_updates_at_next_start();
        // The rows and titles of the last run are shown at once while newer answers come in.
        providers::keep_answers_in(dir.join("catalogue"));
        runtime().spawn(sources::keep_fresh(dir));
    });
}

/// Where the definitions are saved, so a check can be asked for later (see `check_definitions`).
static DEFINITIONS_DIR: OnceLock<PathBuf> = OnceLock::new();
/// When the definitions were last asked for (seconds since 1970), so a screen coming back does not ask again within a minute.
static CHECKED_AT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The least time between two checks that the app asks for itself.
const CHECK_NO_SOONER_THAN: u64 = 60;

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Whether newer definitions have been fetched and saved: the app uses them from its next start.
#[uniffi::export]
pub fn definitions_waiting() -> bool {
    sources::waiting_version().is_some()
}

/// Looks for newer definitions now (in the background; this returns at once). The app calls it each time it comes back to the
/// screen, so a TV that woke from standby does not wait for the next check of the hourly loop. A newer signed copy is saved and
/// used from the next start (`definitions_waiting` then says so).
#[uniffi::export]
pub fn check_definitions() {
    let now = now_seconds();
    if now.saturating_sub(CHECKED_AT.load(std::sync::atomic::Ordering::Relaxed))
        < CHECK_NO_SOONER_THAN
    {
        return;
    }
    CHECKED_AT.store(now, std::sync::atomic::Ordering::Relaxed);
    if let Some(dir) = DEFINITIONS_DIR.get().cloned() {
        runtime().spawn(async move {
            let _ = sources::refresh(&dir).await;
        });
    }
}

fn title_of(media: Media) -> Title {
    Title {
        coming_soon: media.coming_soon(akidev_core::calendar::now_ms()),
        id: media.id,
        name: media.title,
        year: media.year,
        poster: media.poster,
        score: media.score,
        banner: media.banner,
        description: media.description,
        genres: media.genres,
    }
}

/// One tab of the top bar, from the catalogue definitions.
#[derive(uniffi::Record)]
pub struct TabInfo {
    /// `movie`, `series`, `anime`, or a kind a plugin brought: the same word a home row carries.
    pub kind: String,
    pub label: String,
}

/// The tabs after Home, as the definitions in use list them, so a tab added to the definitions appears here without a new release.
#[uniffi::export]
pub fn tabs() -> Vec<TabInfo> {
    sources::screens()
        .tabs
        .into_iter()
        .map(|tab| TabInfo {
            kind: tab.kind.as_str().to_string(),
            label: tab.label,
        })
        .collect()
}

/// The version of the definitions in use. It rises when newer ones were downloaded, which is when the app asks for its screens again.
#[uniffi::export]
pub fn sources_version() -> u64 {
    sources::current().version
}

/// What the home screen knows about a row before its titles come: its name and its kind. This costs nothing (it is in the
/// definitions), so the screen can draw its rows at once and fill each as its titles arrive.
#[derive(uniffi::Record)]
pub struct RowInfo {
    pub name: String,
    pub kind: String,
}

/// The rows of the home screen, as the app's definitions list them, in order.
#[uniffi::export]
pub fn home_rows() -> Vec<RowInfo> {
    sources::screens()
        .home
        .iter()
        .map(|row| RowInfo {
            name: row.title.clone(),
            kind: row.kind.as_str().to_string(),
        })
        .collect()
}

/// The titles of row `index` of `home_rows`. A row that fails or is empty is an error, so the screen can leave it out.
#[uniffi::export]
pub fn home_row(index: u32) -> Result<Row, TvError> {
    let screens = sources::screens();
    let row = screens
        .home
        .get(index as usize)
        .ok_or_else(|| failed("That row is not in the definitions."))?;
    let filters = Filters {
        sort: Some(row.sort),
        genre: row.genre.clone(),
        page: 1,
        per_page: ROW_SIZE,
        ..Default::default()
    };
    let page = runtime()
        .block_on(providers::list_row(
            row.kind,
            row.provider.as_deref(),
            &filters,
        ))
        .map_err(failed)?;
    let mut results = page.results;
    if results.is_empty() {
        return Err(failed("This row has no titles."));
    }
    row.style.trim(&mut results);
    Ok(Row {
        name: row.title.clone(),
        kind: row.kind.as_str().to_string(),
        sort: row.sort.as_str().to_string(),
        genre: row.genre.clone(),
        ranked: row.style == RowStyle::Ranked,
        titles: results.into_iter().map(title_of).collect(),
    })
}

/// How many titles one page of "View all" holds.
const BROWSE_PAGE: u32 = 30;

/// One page of a full list, and whether another page follows.
#[derive(uniffi::Record)]
pub struct Browse {
    pub titles: Vec<Title>,
    pub more: bool,
}

/// A page of one catalogue (`kind` set), or of all of them together (`kind` empty), as the website's browse pages do.
#[uniffi::export]
pub fn browse(
    kind: Option<String>,
    sort: String,
    genre: Option<String>,
    page: u32,
) -> Result<Browse, TvError> {
    let kinds = match kind.as_deref() {
        None => sources::screens().kinds(),
        Some(word) => {
            vec![Kind::parse(word).ok_or_else(|| failed("That catalogue is not known."))?]
        }
    };
    let filters = Filters {
        sort: Some(Sort::parse(&sort).ok_or_else(|| failed("That order is not known."))?),
        genre: genre.filter(|genre| !genre.is_empty()),
        page: page.max(1),
        per_page: BROWSE_PAGE,
        ..Default::default()
    };
    let answers = runtime().block_on(futures::future::join_all(kinds.iter().map(|&kind| {
        let filters = filters.clone();
        async move { providers::list(kind, &filters).await }
    })));
    let answered: Vec<_> = answers.into_iter().filter_map(Result::ok).collect();
    if answered.is_empty() {
        return Err(failed("No catalogue answered."));
    }
    let more = answered.iter().any(|page| page.has_next);
    let merged = catalogue_search::interleave(
        answered.into_iter().map(|page| page.results).collect(),
        usize::MAX,
    );
    Ok(Browse {
        titles: merged.into_iter().map(title_of).collect(),
        more,
    })
}

/// One way to order a list.
#[derive(uniffi::Record)]
pub struct SortInfo {
    pub sort: String,
    pub label: String,
}

/// What the filters of "View all" offer: the orders, and the genres of one catalogue (or of all, when `kind` is empty).
#[derive(uniffi::Record)]
pub struct BrowseOptions {
    pub sorts: Vec<SortInfo>,
    pub genres: Vec<String>,
}

#[uniffi::export]
pub fn browse_options(kind: Option<String>) -> BrowseOptions {
    let screens = sources::screens();
    BrowseOptions {
        sorts: screens
            .sorts
            .iter()
            .map(|option| SortInfo {
                sort: option.sort.as_str().to_string(),
                label: option.label.clone(),
            })
            .collect(),
        genres: screens
            .genres(kind.as_deref().and_then(Kind::parse))
            .to_vec(),
    }
}

/// Looks a title up in the catalogues of the app (films, series and anime).
#[uniffi::export]
pub fn search(term: String) -> Result<Vec<Title>, TvError> {
    let found = runtime()
        .block_on(catalogue_search::search(&term, &Kind::BUILT_IN, 30))
        .map_err(failed)?;
    Ok(found.into_iter().map(title_of).collect())
}

/// One title with what the screen needs to play it.
#[uniffi::export]
pub fn detail(id: String) -> Result<Detail, TvError> {
    let detail = runtime()
        .block_on(providers::detail(&id))
        .ok_or_else(|| failed("That title is not in the catalogue."))?;
    let media = detail.media;
    let now = akidev_core::calendar::now_ms();
    let coming = media.coming_soon(now).is_some();
    // Out when the title is, the air date is not in the future, and (for a show still airing) the number is before the next one to air.
    let is_out = |number: u32, airdate: Option<&str>| {
        !coming
            && !airdate.is_some_and(|date| akidev_core::calendar::is_future_date(date, now))
            && !media.next_episode.is_some_and(|next| number >= next)
    };
    let mut episodes: Vec<Episode> = detail
        .episodes
        .into_iter()
        .map(|e| Episode {
            released: is_out(e.number, e.airdate.as_deref()),
            id: e.id,
            season: e.season,
            number: e.number,
            name: e.name,
            airdate: e.airdate,
        })
        .collect();
    // An anime whose provider gives only a count: the ids follow a pattern.
    if episodes.is_empty()
        && let Some(count) = media.episodes.filter(|count| *count > 0)
        && let Some(parsed) = media_id::parse(&media.id)
    {
        episodes = (1..=count)
            .map(|number| Episode {
                id: media_id::anime_episode_id(media.kind, &parsed.provider_id, number),
                season: None,
                number,
                name: None,
                airdate: None,
                released: is_out(number, None),
            })
            .collect();
    }
    let play_id = (!coming && media.kind.shape() == Shape::Movie).then(|| media.id.clone());
    Ok(Detail {
        status: media.status.clone(),
        runtime_minutes: media.runtime_minutes,
        play_id,
        episodes,
        title: title_of(media),
    })
}

/// Whether the key works, for Settings.
#[uniffi::export]
pub fn check_key(key: String) -> Result<(), TvError> {
    let torbox = TorBox::new(&key);
    runtime()
        .block_on(torbox.check_key())
        .map_err(|e| failed(torbox.redact(format!("{e:#}"))))
}

/// Starts looking for releases of `id` in the background, so the search is already answered (core keeps it) when the viewer presses Play.
/// Nothing waits for it and nothing is shown.
#[uniffi::export]
pub fn prepare_stream(id: String) {
    runtime().spawn(async move {
        let _ = stream::plan(&Config::default(), &id).await;
    });
}

/// One copy TorBox holds, as the player's panel lists it.
#[derive(uniffi::Record)]
pub struct Source {
    /// Pass this to `find_stream_of` to play exactly this copy.
    pub hash: String,
    /// "1080p  ·  HEVC  ·  4.2 GB", and "No sound on this TV" for a copy whose sound the TV cannot decode.
    pub label: String,
}

/// A link to play `id` (a film or one episode): finds the releases, asks TorBox which it holds, and
/// picks the file. Only releases TorBox already holds are used. `skip` lists releases that did not
/// play before, so "try again" gives another copy.
#[uniffi::export]
pub fn find_stream(id: String, key: String, skip: Vec<String>) -> Result<Stream, TvError> {
    link(id, key, skip, None)
}

/// A link to play exactly the copy `hash` (one of `sources`).
#[uniffi::export]
pub fn find_stream_of(id: String, key: String, hash: String) -> Result<Stream, TvError> {
    link(id, key, Vec::new(), Some(hash))
}

/// The copies TorBox holds for `id` that have a file to play, best first (the one `find_stream` would pick comes first).
#[uniffi::export]
pub fn sources(id: String, key: String) -> Result<Vec<Source>, TvError> {
    let torbox = TorBox::new(&key);
    let found = runtime().block_on(async {
        tokio::time::timeout(FIND_FOR, async {
            let found = candidates(&id, &[], None, &torbox).await?;
            let mut seen = std::collections::HashMap::<String, usize>::new();
            Ok::<_, String>(
                found
                    .releases
                    .iter()
                    .filter(|release| found.has_file(release))
                    .take(MOST_SOURCES)
                    .map(|release| {
                        // Two copies that look alike get a number, so each line is told apart.
                        let label = source_label(release);
                        let count = seen.entry(label.clone()).or_insert(0);
                        *count += 1;
                        Source {
                            hash: release.info_hash.clone(),
                            label: if *count == 1 {
                                label
                            } else {
                                format!("{label}  ·  {count}")
                            },
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .await
        .map_err(|_| "Took too long to look for copies.".to_string())?
    });
    found.map_err(|reason| failed(torbox.redact(reason)))
}

fn link(
    id: String,
    key: String,
    skip: Vec<String>,
    only: Option<String>,
) -> Result<Stream, TvError> {
    let torbox = TorBox::new(&key);
    let found = runtime().block_on(async {
        tokio::time::timeout(FIND_FOR, find(&id, &skip, only.as_deref(), &torbox))
            .await
            .map_err(|_| "Took too long to find a playable copy.".to_string())?
    });
    found.map_err(|reason| failed(torbox.redact(reason)))
}

/// The releases for a title in the order to try them, and which of them TorBox holds.
struct Candidates {
    plan: stream::Plan,
    releases: Vec<Release>,
    cached: std::collections::HashMap<String, Option<Vec<VideoFile>>>,
}

impl Candidates {
    /// TorBox holds it, and (when it said which files it has) there is a file for this film or episode in it.
    fn has_file(&self, release: &Release) -> bool {
        match self.cached.get(&release.info_hash) {
            None => false,
            Some(None) => true,
            Some(Some(files)) => {
                let media = &self.plan.detail.media;
                let year = media.year.and_then(|year| u32::try_from(year).ok());
                pick_for_identity(
                    files,
                    release,
                    self.plan.playback_episode(),
                    self.plan.parsed.season,
                    &media.title,
                    &media.title_variants,
                    year,
                )
                .is_some()
            }
        }
    }
}

/// `skip` leaves releases out; `only` keeps just that one.
async fn candidates(
    id: &str,
    skip: &[String],
    only: Option<&str>,
    torbox: &TorBox,
) -> Result<Candidates, String> {
    let started = std::time::Instant::now();
    let plan = stream::plan(&Config::default(), id).await?;
    note(
        started,
        &format!(
            "plan: {} playable, {} others",
            plan.ranked.len(),
            plan.extra.len()
        ),
    );
    // The TV decodes H.264 and HEVC in hardware, so the releases the other apps hold back are used too, after the rest.
    // Those with DTS or TrueHD sound go last: the TV has no decoder for them, and they would play in silence.
    let mut releases: Vec<_> = plan
        .ranked
        .iter()
        .chain(plan.extra.iter())
        .filter(|release| !skip.contains(&release.info_hash))
        .filter(|release| only.is_none_or(|hash| release.info_hash == hash))
        .take(40)
        .cloned()
        .collect();
    releases.sort_by_key(|release| has_sound_the_tv_cannot_play(&release.title));
    if releases.is_empty() {
        return Err("No other release found for this title.".into());
    }
    let cached = torbox
        .cached(&releases)
        .await
        .map_err(|e| format!("{e:#}"))?;
    note(
        started,
        &format!("checkcached: {} of {} held", cached.len(), releases.len()),
    );
    Ok(Candidates {
        plan,
        releases,
        cached,
    })
}

async fn find(
    id: &str,
    skip: &[String],
    only: Option<&str>,
    torbox: &TorBox,
) -> Result<Stream, String> {
    let started = std::time::Instant::now();
    let found = candidates(id, skip, only, torbox).await?;
    let media = &found.plan.detail.media;
    let year = media.year.and_then(|year| u32::try_from(year).ok());
    let (episode, season) = (found.plan.playback_episode(), found.plan.parsed.season);

    for release in found
        .releases
        .iter()
        .filter(|r| found.has_file(r))
        .take(MOST_TRIES)
    {
        let Ok(Some(entry)) = torbox.entry_for(release).await else {
            continue;
        };
        note(started, "account entry found");
        let files: Vec<_> = entry.files.iter().map(|(_, file)| file.clone()).collect();
        let Some(picked) = pick_for_identity(
            &files,
            release,
            episode,
            season,
            &media.title,
            &media.title_variants,
            year,
        ) else {
            continue;
        };
        let (file_id, _) = &entry.files[picked.index];
        let url = torbox
            .link(entry.id, *file_id)
            .await
            .map_err(|e| format!("{e:#}"))?;
        note(started, "link ready");
        return Ok(Stream {
            url,
            hash: release.info_hash.clone(),
        });
    }
    if found.cached.is_empty() {
        Err("TorBox does not hold a copy of this yet.".into())
    } else {
        Err("TorBox holds copies, but none has a file that can be played.".into())
    }
}

/// "1080p  ·  HEVC  ·  4.2 GB": what tells copies apart for a viewer.
fn source_label(release: &Release) -> String {
    let mut parts = Vec::new();
    if let Some(height) = parse_resolution(&release.title) {
        parts.push(format!("{height}p"));
    }
    match parse_codec(&release.title) {
        Some(Codec::Hevc) => parts.push("HEVC".to_string()),
        Some(Codec::H264) => parts.push("H.264".to_string()),
        Some(Codec::Av1) => parts.push("AV1".to_string()),
        None => {}
    }
    if let Some(bytes) = release.size_bytes {
        parts.push(format!("{:.1} GB", bytes as f64 / 1e9));
    }
    if has_sound_the_tv_cannot_play(&release.title) {
        parts.push("No sound on this TV".to_string());
    }
    if parts.is_empty() {
        release.title.chars().take(40).collect()
    } else {
        parts.join("  ·  ")
    }
}

/// DTS and TrueHD need a decoder the TV does not have; a release with only those has no sound.
fn has_sound_the_tv_cannot_play(title: &str) -> bool {
    let title = title.to_ascii_lowercase();
    title.contains("dts") || title.contains("truehd")
}

/// The subtitles from outside the video for a film or an episode. Empty when there are none or the sources cannot be reached.
#[uniffi::export]
pub fn subtitles(id: String) -> Vec<Subtitle> {
    runtime().block_on(async {
        let Some(parsed) = media_id::parse(&id) else {
            return Vec::new();
        };
        let Some(detail) = providers::detail(&parsed.key()).await else {
            return Vec::new();
        };
        let query = subtitles::query_for(&detail, &parsed);
        subtitles::find(&query, SUBTITLES_FOR)
            .await
            .into_iter()
            .map(|found| Subtitle {
                language: found.language,
                label: found.label,
                url: found.url,
            })
            .collect()
    })
}

/// One subtitle file as WebVTT, which the player reads.
#[uniffi::export]
pub fn subtitle_file(url: String) -> Result<String, TvError> {
    runtime().block_on(subtitles::fetch(&url)).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dts_and_truehd_releases_are_marked() {
        assert!(has_sound_the_tv_cannot_play(
            "Film 2010 1080p BluRay DTS-HD MA 5.1 x264"
        ));
        assert!(has_sound_the_tv_cannot_play(
            "Film.2010.2160p.REMUX.TrueHD.Atmos.7.1"
        ));
        assert!(!has_sound_the_tv_cannot_play(
            "Film 2010 1080p WEB-DL DDP5.1 H.264"
        ));
        assert!(!has_sound_the_tv_cannot_play(
            "Show S01E01 1080p WEB-DL AAC2.0"
        ));
    }

    fn release(title: &str, size_bytes: Option<u64>) -> Release {
        Release {
            title: title.to_string(),
            info_hash: "0".repeat(40),
            file_idx: None,
            magnet: String::new(),
            size_bytes,
            seeders: None,
            source: String::new(),
            sources: Vec::new(),
        }
    }

    #[test]
    fn a_source_is_named_by_quality_codec_and_size() {
        assert_eq!(
            source_label(&release(
                "Film 2010 1080p WEB-DL x265 AAC",
                Some(4_200_000_000)
            )),
            "1080p  ·  HEVC  ·  4.2 GB"
        );
        assert_eq!(
            source_label(&release("Film 2010 2160p REMUX TrueHD", None)),
            "2160p  ·  No sound on this TV"
        );
        assert_eq!(source_label(&release("Film", None)), "Film");
    }
}
