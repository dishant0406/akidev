//! The Swift-facing face of the manga engine: one `Engine` object holding the sources, the library
//! database and the settings a challenge hands over. Every type here mirrors one in `akidev-manga`;
//! uniffi cannot derive across crates, so they are converted at this edge and nowhere else.

use akidev_manga::fetch::{Challenge, Unreachable};
use akidev_manga::model as m;
use akidev_manga::repo;
use akidev_manga::source::{Info, Source};
use akidev_manga::store::{BookMarkRow, BookRow, Export, Progress, Store};
use akidev_manga::theme::Themes;
use akidev_manga::{downloads, local};
use anyhow::Context;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

#[derive(Debug, uniffi::Error)]
pub enum MangaError {
    /// The site wants a check first (Cloudflare, a captcha). There is no browser in the app to pass it in, so
    /// the screen says the site is not supported from here.
    Challenge {
        url: String,
        status: u16,
    },
    /// The site could not be reached at all (no network, a blocked address).
    Offline {
        url: String,
        reason: String,
    },
    Failed {
        message: String,
    },
}

impl std::fmt::Display for MangaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MangaError::Challenge { url, .. } => write!(f, "{url} wants a check before it answers"),
            MangaError::Offline { url, reason } => write!(f, "cannot reach {url}: {reason}"),
            MangaError::Failed { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MangaError {}

impl From<anyhow::Error> for MangaError {
    fn from(error: anyhow::Error) -> MangaError {
        if let Some(challenge) = error.downcast_ref::<Challenge>() {
            return MangaError::Challenge {
                url: challenge.url.clone(),
                status: challenge.status,
            };
        }
        if let Some(down) = error.downcast_ref::<Unreachable>() {
            return MangaError::Offline {
                url: down.url.clone(),
                reason: down.reason.clone(),
            };
        }
        MangaError::Failed {
            message: format!("{error:#}"),
        }
    }
}

type Result<T> = std::result::Result<T, MangaError>;

fn failed(message: impl Into<String>) -> MangaError {
    MangaError::Failed {
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub chapters: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CardPage {
    pub items: Vec<Card>,
    pub has_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Status {
    Ongoing,
    Completed,
    Hiatus,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Detail {
    pub title: String,
    pub cover: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub status: Status,
}

/// A chapter and where the person is in it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ChapterEntry {
    pub id: String,
    pub number: Option<f64>,
    pub title: Option<String>,
    pub date_ms: Option<i64>,
    pub scanlator: Option<String>,
    pub finished: bool,
    /// The page the person is on, in a chapter not yet finished.
    pub page: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ChapterList {
    /// Newest first.
    pub entries: Vec<ChapterEntry>,
    /// Chapters that appeared since the last time this series was fetched (none the first time).
    pub new_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Reading {
    LongStrip,
    Paged,
    PagedRtl,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PageList {
    pub images: Vec<String>,
    pub reading: Reading,
    /// Headers every image request must carry.
    pub headers: Vec<Header>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub nsfw: bool,
    pub version: u64,
    pub can_popular: bool,
    pub can_latest: bool,
    pub can_search: bool,
    pub has_settings: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SettingOption {
    pub value: String,
    pub name: String,
}

/// A choice a source offers (a language, a content rating) and what is chosen now.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SettingInfo {
    pub id: String,
    pub name: String,
    pub options: Vec<SettingOption>,
    pub value: String,
}

/// A repository the person added.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RepoInfo {
    /// The folder the repository is read from (its identity).
    pub base: String,
    pub name: String,
    /// Whether the repository signs its index: a signed one cannot change hands unnoticed.
    pub signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SourceState {
    NotInstalled,
    Installed,
    UpdateAvailable,
    /// Needs a newer app than this one.
    NeedsNewerApp,
}

/// A source a repository offers.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RepoSource {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub languages: Vec<String>,
    pub nsfw: bool,
    pub state: SourceState,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RepoView {
    pub info: RepoInfo,
    pub sources: Vec<RepoSource>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LibraryEntry {
    pub source: String,
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub unread: u32,
    pub total: u32,
    pub last_read_ms: Option<i64>,
    pub saved_ms: i64,
    /// Ids of the categories it is in.
    pub categories: Vec<i64>,
    /// When its chapters were last asked of the site.
    pub checked_ms: Option<i64>,
    /// A finished series: worth checking rarely.
    pub completed: bool,
}

/// A saved series with its newest chapter.
#[derive(Debug, Clone, uniffi::Record)]
pub struct UpdateEntry {
    pub source: String,
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub unread: u32,
    pub latest_number: Option<f64>,
    pub latest_title: Option<String>,
    pub latest_date_ms: Option<i64>,
}

/// A page the person marked.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Bookmark {
    pub chapter: String,
    pub page: u32,
    /// How far down the page, 0 to 1 of its height.
    pub offset: f64,
    pub created_ms: i64,
    pub chapter_number: Option<f64>,
    pub chapter_title: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CategoryInfo {
    pub id: i64,
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FilterKind {
    Select,
    Text,
}

/// A way to narrow a search, with its choices.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FilterInfo {
    pub id: String,
    pub name: String,
    pub kind: FilterKind,
    pub options: Vec<SettingOption>,
    pub default: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Choice {
    pub id: String,
    pub value: String,
}

/// What a restore did.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RestoreReport {
    pub series: u32,
    pub sources_installed: Vec<String>,
    /// Sources the backup used that could not be brought back (the repository is gone, or it is a hand-made one).
    pub sources_missing: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DownloadInfo {
    pub source: String,
    pub series: String,
    pub chapter: String,
    /// `queued`, `active`, `done` or `failed`.
    pub state: String,
    pub pages: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Resume {
    pub chapter: String,
    pub page: u32,
    pub offset: f64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HistoryEntry {
    pub source: String,
    pub series: String,
    pub title: String,
    pub cover: Option<String>,
    pub chapter: String,
    pub chapter_number: Option<f64>,
    pub chapter_title: Option<String>,
    pub at_ms: i64,
    pub finished: bool,
}

impl From<m::Card> for Card {
    fn from(card: m::Card) -> Card {
        Card {
            id: card.id,
            title: card.title,
            cover: card.cover,
            chapters: card.chapters,
        }
    }
}

impl From<m::Page<m::Card>> for CardPage {
    fn from(page: m::Page<m::Card>) -> CardPage {
        CardPage {
            items: page.items.into_iter().map(Card::from).collect(),
            has_next: page.has_next,
        }
    }
}

impl From<m::Status> for Status {
    fn from(status: m::Status) -> Status {
        match status {
            m::Status::Ongoing => Status::Ongoing,
            m::Status::Completed => Status::Completed,
            m::Status::Hiatus => Status::Hiatus,
            m::Status::Cancelled => Status::Cancelled,
            m::Status::Unknown => Status::Unknown,
        }
    }
}

impl From<m::Detail> for Detail {
    fn from(detail: m::Detail) -> Detail {
        Detail {
            title: detail.title,
            cover: detail.cover,
            description: detail.description,
            authors: detail.authors,
            tags: detail.tags,
            status: detail.status.into(),
        }
    }
}

impl From<m::Reading> for Reading {
    fn from(reading: m::Reading) -> Reading {
        match reading {
            m::Reading::LongStrip => Reading::LongStrip,
            m::Reading::Paged => Reading::Paged,
            m::Reading::PagedRtl => Reading::PagedRtl,
            m::Reading::Unknown => Reading::Unknown,
        }
    }
}

impl From<m::PageList> for PageList {
    fn from(list: m::PageList) -> PageList {
        PageList {
            images: list.images,
            reading: list.reading.into(),
            headers: list
                .headers
                .into_iter()
                .map(|(name, value)| Header { name, value })
                .collect(),
        }
    }
}

impl From<Info> for SourceInfo {
    fn from(info: Info) -> SourceInfo {
        SourceInfo {
            id: info.id,
            name: info.name,
            languages: info.languages,
            nsfw: info.nsfw,
            version: info.version,
            can_popular: info.can_popular,
            can_latest: info.can_latest,
            can_search: info.can_search,
            has_settings: info.has_settings,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------------------------

/// A name that is safe as a file name: lowercase letters, digits and dashes.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A cover the person chose is stored as `custom/<number>.jpg`; the picture is `covers/<number>.jpg` in the data folder.
const CUSTOM: &str = "custom/";
/// A chosen cover is a small JPEG (the app makes one under a megabyte at most); far more than this is not a cover.
const MAX_COVER: usize = 2 * 1024 * 1024;

fn custom_name_is_sound(name: &str) -> bool {
    name.strip_prefix(CUSTOM)
        .and_then(|rest| rest.strip_suffix(".jpg"))
        .is_some_and(|number| {
            !number.is_empty() && number.len() <= 20 && number.bytes().all(|b| b.is_ascii_digit())
        })
}

/// Page lists fetched in the last few minutes, newest last: opening a chapter just seen, or one fetched ahead while the series
/// page was on screen, does not ask the site again. A list is dropped after ten minutes (some sites sign their picture
/// addresses for a short while) and the oldest goes when there are more than a couple of dozen.
#[derive(Default)]
struct PageCache {
    lists: Vec<(String, Instant, PageList)>,
}

impl PageCache {
    const LIFETIME: Duration = Duration::from_secs(600);
    const MOST: usize = 24;

    fn get(&self, key: &str, now: Instant) -> Option<PageList> {
        self.lists
            .iter()
            .find(|(name, at, _)| name == key && now.duration_since(*at) < Self::LIFETIME)
            .map(|(_, _, list)| list.clone())
    }

    fn put(&mut self, key: String, now: Instant, list: PageList) {
        self.lists.retain(|(name, _, _)| *name != key);
        self.lists.push((key, now, list));
        if self.lists.len() > Self::MOST {
            self.lists.remove(0);
        }
    }
}

fn now_ms() -> i64 {
    akidev_core::calendar::now_ms()
}

/// The source id comic archives from the device are filed under; it has no definition.
const DEVICE: &str = "device";

fn file_url(path: &std::path::Path) -> String {
    format!("file://{}", path.display())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Backup {
    version: u32,
    library: Export,
    /// The pictures chosen as covers, by name, as base64: a few tens of kilobytes each.
    #[serde(default)]
    covers: BTreeMap<String, String>,
    repos: Vec<String>,
    sources: Vec<BackupSource>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BackupSource {
    id: String,
    /// The repository it came from; none for one installed by hand.
    repo: Option<String>,
    /// The whole definition, kept only when there is no repository to get it from.
    document: Option<serde_json::Value>,
    settings: Vec<(String, String)>,
}

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The app is a renderer: it has no source of its own. Everything it reads comes from a definition the
/// person installed, usually from a repository they added.
#[derive(uniffi::Object)]
pub struct Engine {
    dir: PathBuf,
    store: Store,
    themes: RwLock<Themes>,
    sources: RwLock<BTreeMap<String, Arc<Source>>>,
    /// Chapters being downloaded now, as `source/series/chapter`, so one is never fetched twice at once.
    downloading: std::sync::Mutex<std::collections::HashSet<String>>,
    /// Page lists asked of a site in the last few minutes.
    page_lists: std::sync::Mutex<PageCache>,
    /// One turn at a time per chapter: a second ask for a chapter being fetched waits for that fetch and reuses its answer.
    page_turns: std::sync::Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Voice models being downloaded now.
    installing: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl Engine {
    fn keep_first_page(&self, source: &str, kind: &str, page: u32, found: &m::Page<m::Card>) {
        if page == 1
            && let Ok(body) = serde_json::to_string(found)
        {
            let _ = self.store.keep_listing(source, kind, &body, now_ms());
        }
    }

    fn source(&self, id: &str) -> Result<Arc<Source>> {
        self.sources
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| failed(format!("there is no source called {id}")))
    }

    fn read_dir_json(&self, name: &str) -> Vec<(String, serde_json::Value)> {
        let Ok(entries) = std::fs::read_dir(self.dir.join(name)) else {
            return Vec::new();
        };
        let mut found: Vec<(String, serde_json::Value)> = entries
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension()? == "json").then_some(())?;
                let stem = path.file_stem()?.to_str()?.to_string();
                Some((
                    stem,
                    serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?,
                ))
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    }

    /// A source from its document, with the person's settings put back.
    fn build(&self, document: serde_json::Value) -> anyhow::Result<Source> {
        let def = self.themes.read().unwrap().resolve(document)?;
        let source = Source::live(def)?;
        for (key, value) in self.store.settings(&source.info().id)? {
            // A choice the source no longer offers is dropped quietly.
            let _ = source.choose(&key, &value);
        }
        Ok(source)
    }

    /// Reads every installed theme and source again (after a theme changed).
    fn reload(&self) {
        let mut themes = Themes::default();
        for (name, theme) in self.read_dir_json("themes") {
            themes.insert(&name, theme);
        }
        *self.themes.write().unwrap() = themes;
        let mut sources = BTreeMap::new();
        for (name, document) in self.read_dir_json("sources") {
            // A saved source that no longer loads (its theme changed) is left out, not a reason to fail.
            match self.build(document) {
                Ok(source) => {
                    sources.insert(source.info().id, Arc::new(source));
                }
                Err(error) => eprintln!("akimanga: source {name} did not load: {error:#}"),
            }
        }
        *self.sources.write().unwrap() = sources;
    }

    fn save(&self, folder: &str, name: &str, text: &str) -> Result<()> {
        std::fs::create_dir_all(self.dir.join(folder))
            .and_then(|()| std::fs::write(self.dir.join(folder).join(format!("{name}.json")), text))
            .map_err(|error| failed(error.to_string()))
    }

    fn installed_version(&self, id: &str) -> Option<u64> {
        self.sources
            .read()
            .unwrap()
            .get(id)
            .map(|source| source.info().version)
    }

    fn view(&self, base: &str, name: &str, signed: bool, index: &repo::Index) -> RepoView {
        let sources = index
            .sources
            .iter()
            .map(|offered| {
                let state = if offered
                    .min_app
                    .as_deref()
                    .is_some_and(|wanted| !repo::app_at_least(APP_VERSION, wanted))
                {
                    SourceState::NeedsNewerApp
                } else {
                    match self.installed_version(&offered.id) {
                        None => SourceState::NotInstalled,
                        Some(have) if have < offered.version => SourceState::UpdateAvailable,
                        Some(_) => SourceState::Installed,
                    }
                };
                RepoSource {
                    id: offered.id.clone(),
                    name: offered.name.clone(),
                    version: offered.version,
                    languages: offered.lang.clone(),
                    nsfw: offered.nsfw,
                    state,
                }
            })
            .collect();
        RepoView {
            info: RepoInfo {
                base: base.to_string(),
                name: name.to_string(),
                signed,
            },
            sources,
        }
    }

    /// Fetches what a repository offers and installs one source (with the themes it extends).
    async fn install_one(
        &self,
        base: &str,
        index: &repo::Index,
        source_id: &str,
    ) -> anyhow::Result<SourceInfo> {
        let offered = index
            .sources
            .iter()
            .find(|offered| offered.id == source_id)
            .with_context(|| format!("the repository has no source called {source_id}"))?;
        if let Some(wanted) = &offered.min_app {
            anyhow::ensure!(
                repo::app_at_least(APP_VERSION, wanted),
                "{} needs Akidev {wanted} or newer (this is {APP_VERSION})",
                offered.name
            );
        }
        let mut themes_changed = false;
        for theme_id in &offered.requires {
            anyhow::ensure!(
                plain_name(theme_id),
                "{theme_id} is not a name a theme may have"
            );
            let theme = index
                .themes
                .iter()
                .find(|theme| &theme.id == theme_id)
                .with_context(|| format!("the repository has no theme {theme_id}"))?;
            let text = repo::fetch_file(base, &theme.path, &theme.sha256).await?;
            serde_json::from_str::<serde_json::Value>(&text)
                .with_context(|| format!("theme {theme_id} is not JSON"))?;
            let path = self.dir.join("themes").join(format!("{theme_id}.json"));
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                self.save("themes", theme_id, &text)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
                themes_changed = true;
            }
        }
        if themes_changed {
            self.reload();
        }
        let text = repo::fetch_file(base, &offered.path, &offered.sha256).await?;
        let info = self
            .install_document(&text, false)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        anyhow::ensure!(
            info.id == offered.id && info.version == offered.version,
            "the file for {} is not the source the index lists (it is {} {})",
            offered.id,
            info.id,
            info.version
        );
        self.store.set_origin(&info.id, Some(base), info.version)?;
        Ok(info)
    }

    /// The source a document makes, and the text it is kept as. One pasted on its own has the themes it
    /// extends merged in and is kept that way, so it stands alone; one from a repository keeps what it was
    /// given, so it follows its theme's updates.
    fn prepared(&self, json: &str, pasted: bool) -> Result<(Source, String)> {
        let given: serde_json::Value = serde_json::from_str(json)
            .map_err(|error| failed(format!("the definition is not JSON: {error}")))?;
        if !pasted {
            let source = self
                .build(given)
                .map_err(|error| failed(format!("{error:#}")))?;
            return Ok((source, json.to_string()));
        }
        let merged = self
            .themes
            .read()
            .unwrap()
            .resolve_document(given)
            .map_err(|error| failed(format!("{error:#}")))?;
        let text =
            serde_json::to_string_pretty(&merged).map_err(|error| failed(error.to_string()))?;
        let source = self
            .build(merged)
            .map_err(|error| failed(format!("{error:#}")))?;
        Ok((source, text))
    }

    /// The source a document makes, checked as if it were about to be kept: a preview and an install refuse
    /// the same things, so the sheet never offers an install that would then fail.
    fn checked(&self, json: &str, pasted: bool) -> Result<(Source, String)> {
        let (source, text) = self.prepared(json, pasted)?;
        let info = source.info();
        if !plain_name(&info.id) {
            return Err(failed(format!(
                "{} is not an id a source may have",
                info.id
            )));
        }
        if let Some(have) = self.installed_version(&info.id)
            && have > info.version
        {
            return Err(failed(format!(
                "{} {have} is installed; this is the older {}",
                info.name, info.version
            )));
        }
        Ok((source, text))
    }

    /// Installs a source from its document, checking it first.
    fn install_document(&self, json: &str, pasted: bool) -> Result<SourceInfo> {
        let (source, text) = self.checked(json, pasted)?;
        let info = source.info();
        self.save("sources", &info.id, &text)?;
        self.sources
            .write()
            .unwrap()
            .insert(info.id.clone(), Arc::new(source));
        Ok(info.into())
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Engine {
    /// Opens (or creates) the library in `data_dir` and loads the themes and sources installed there.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Engine>> {
        let dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&dir)
            .map_err(|error| failed(format!("cannot use {}: {error}", dir.display())))?;
        let store = Store::open(&dir.join("manga.db"))?;
        store.fail_interrupted_downloads()?;
        downloads::clear_unfinished(&dir.join("downloads"));
        akidev_manga::voice::clear_unfinished(&dir.join("voices"));
        let engine = Engine {
            dir,
            store,
            themes: RwLock::default(),
            sources: RwLock::default(),
            downloading: Default::default(),
            page_lists: Default::default(),
            page_turns: Default::default(),
            installing: Default::default(),
        };
        engine.reload();
        Ok(Arc::new(engine))
    }

    pub fn version(&self) -> String {
        APP_VERSION.to_string()
    }

    // --- sources -------------------------------------------------------------------------------

    pub fn sources(&self) -> Vec<SourceInfo> {
        self.sources
            .read()
            .unwrap()
            .values()
            .map(|source| source.info().into())
            .collect()
    }

    /// Installs a theme (a base definition for sites on the same CMS), kept across launches. Repositories
    /// install their themes themselves; this is for a definition written by hand.
    pub fn install_theme(&self, name: String, json: String) -> Result<()> {
        if !plain_name(&name) {
            return Err(failed(
                "a theme's name is lowercase letters, digits and dashes",
            ));
        }
        serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|error| failed(format!("the theme is not JSON: {error}")))?;
        self.save("themes", &name, &json)?;
        self.reload();
        Ok(())
    }

    /// Installs a source from its definition, checking it first. A source with the same id is replaced
    /// if the new one's version is not lower. The themes it extends are merged into it and it is kept that
    /// way, so one pasted on its own works without the repository that has the theme.
    pub fn install_source(&self, json: String) -> Result<SourceInfo> {
        let info = self.install_document(&json, true)?;
        self.store.set_origin(&info.id, None, info.version)?;
        Ok(info)
    }

    /// What a definition would install, without installing it: the "install this source?" sheet. Checks the
    /// whole definition, so a broken one says what is wrong before anything is kept.
    pub fn preview_source(&self, json: String) -> Result<SourceInfo> {
        Ok(self.checked(&json, true)?.0.info().into())
    }

    /// The repository an installed source came from, if any. Pasting over one of these replaces it with a
    /// standalone copy that no longer updates, which the sheet warns about first.
    pub fn installed_origin(&self, id: String) -> Option<String> {
        self.store
            .origin(&id)
            .ok()
            .flatten()
            .and_then(|(origin, _)| origin)
    }

    pub fn remove_source(&self, id: String) -> Result<()> {
        if !plain_name(&id) {
            return Err(failed("that is not a source"));
        }
        self.sources.write().unwrap().remove(&id);
        let _ = std::fs::remove_file(self.dir.join("sources").join(format!("{id}.json")));
        self.store.forget_source(&id)?;
        Ok(())
    }

    /// The choices a source offers (language, content) and what is chosen.
    pub fn source_settings(&self, source: String) -> Result<Vec<SettingInfo>> {
        Ok(self
            .source(&source)?
            .settings()
            .into_iter()
            .map(|setting| SettingInfo {
                id: setting.id,
                name: setting.name,
                options: setting
                    .options
                    .into_iter()
                    .map(|(value, name)| SettingOption { value, name })
                    .collect(),
                value: setting.value,
            })
            .collect())
    }

    pub fn set_setting(&self, source: String, id: String, value: String) -> Result<()> {
        self.source(&source)?.choose(&id, &value)?;
        self.store.set_setting(&source, &id, &value)?;
        Ok(())
    }

    // --- repositories --------------------------------------------------------------------------

    /// Adds a repository by its address (a GitHub link, or any https folder with an index.json), and
    /// returns what it offers. Nothing is installed.
    pub async fn add_repo(&self, link: String) -> Result<RepoView> {
        let base = repo::base_of(&link)?;
        let fetched = repo::fetch_index(&base).await?;
        self.store.add_repo(
            &base,
            &fetched.index.name,
            fetched.index.key.as_deref(),
            fetched.signed,
            now_ms(),
        )?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    /// What a repository offers, without keeping it: the "add this repository?" sheet.
    pub async fn preview_repo(&self, link: String) -> Result<RepoView> {
        let base = repo::base_of(&link)?;
        let fetched = repo::fetch_index(&base).await?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    pub fn repos(&self) -> Result<Vec<RepoInfo>> {
        Ok(self
            .store
            .repos()?
            .into_iter()
            .map(|(base, name, signed)| RepoInfo { base, name, signed })
            .collect())
    }

    /// Removes a repository. Sources installed from it stay, but no longer update.
    pub fn remove_repo(&self, base: String) -> Result<()> {
        Ok(self.store.remove_repo(&base)?)
    }

    /// What a repository offers now, and which of it is installed.
    pub async fn repo_sources(&self, base: String) -> Result<RepoView> {
        let fetched = repo::fetch_index(&base).await?;
        self.store.add_repo(
            &base,
            &fetched.index.name,
            fetched.index.key.as_deref(),
            fetched.signed,
            now_ms(),
        )?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    /// Installs (or updates) one source from a repository, with the themes it extends. Every file is
    /// checked against the hash the index gives.
    pub async fn install_from_repo(&self, base: String, source: String) -> Result<SourceInfo> {
        let fetched = repo::fetch_index(&base).await?;
        self.store.add_repo(
            &base,
            &fetched.index.name,
            fetched.index.key.as_deref(),
            fetched.signed,
            now_ms(),
        )?;
        Ok(self.install_one(&base, &fetched.index, &source).await?)
    }

    /// Updates every installed source whose repository offers a newer version. Returns the ids updated.
    /// A repository that cannot be reached is skipped, not a failure.
    pub async fn update_sources(&self) -> Result<Vec<String>> {
        let mut updated = Vec::new();
        for (base, _, _) in self.store.repos()? {
            let Ok(fetched) = repo::fetch_index(&base).await else {
                continue;
            };
            for offered in &fetched.index.sources {
                let Some((Some(origin), version)) = self.store.origin(&offered.id)? else {
                    continue;
                };
                if origin == base
                    && version < offered.version
                    && self
                        .install_one(&base, &fetched.index, &offered.id)
                        .await
                        .is_ok()
                {
                    updated.push(offered.id.clone());
                }
            }
        }
        Ok(updated)
    }

    /// After the person has a clearance cookie for a site (pasted in, say), or a login: cookies a
    /// source sends. Nothing in the app gets them from a browser.
    pub fn set_cookies(&self, source: String, cookies: Vec<Cookie>) -> Result<()> {
        self.source(&source)?.fetcher().set_cookies(
            cookies
                .into_iter()
                .map(|cookie| (cookie.name, cookie.value)),
        );
        Ok(())
    }

    // --- asking a source -----------------------------------------------------------------------

    pub async fn popular(&self, source: String, page: u32) -> Result<CardPage> {
        let found = self.source(&source)?.popular(page).await?;
        self.keep_first_page(&source, "popular", page, &found);
        Ok(found.into())
    }

    pub async fn latest(&self, source: String, page: u32) -> Result<CardPage> {
        let found = self.source(&source)?.latest(page).await?;
        self.keep_first_page(&source, "latest", page, &found);
        Ok(found.into())
    }

    /// What `popular` or `latest` gave last time for page one, with no network: shown at once while the fresh page loads.
    pub fn remembered_listing(&self, source: String, latest: bool) -> Option<CardPage> {
        let body = self
            .store
            .listing(&source, if latest { "latest" } else { "popular" })
            .ok()??;
        serde_json::from_str::<m::Page<m::Card>>(&body)
            .ok()
            .map(CardPage::from)
    }

    /// The ways a source lets a search be narrowed.
    pub fn source_filters(&self, source: String) -> Result<Vec<FilterInfo>> {
        Ok(self
            .source(&source)?
            .filters()
            .iter()
            .map(|filter| FilterInfo {
                id: filter.id.clone(),
                name: filter.name.clone(),
                kind: match filter.kind {
                    akidev_manga::def::FilterKind::Select => FilterKind::Select,
                    akidev_manga::def::FilterKind::Text => FilterKind::Text,
                },
                options: filter
                    .options
                    .iter()
                    .map(|option| SettingOption {
                        value: option.value.clone(),
                        name: option.name.clone(),
                    })
                    .collect(),
                default: filter.default.clone().unwrap_or_default(),
            })
            .collect())
    }

    /// A search; `choices` are the filters the person set (the rest take their defaults).
    pub async fn search(
        &self,
        source: String,
        term: String,
        page: u32,
        choices: Vec<Choice>,
    ) -> Result<CardPage> {
        let chosen = choices
            .into_iter()
            .map(|choice| (choice.id, choice.value))
            .collect();
        Ok(self
            .source(&source)?
            .search(&term, page, &chosen)
            .await?
            .into())
    }

    /// The series' page. Also remembers it, so it can be saved to the library.
    pub async fn details(&self, source: String, id: String) -> Result<Detail> {
        if source == DEVICE {
            return Err(failed("a comic on the device has no page to fetch"));
        }
        let detail = self.source(&source)?.details(&id).await?;
        self.store.keep_detail(&source, &id, &detail, now_ms())?;
        self.store.upsert_series(
            &source,
            &id,
            &detail.title,
            detail.cover.as_deref(),
            detail.description.as_deref(),
            detail.status,
        )?;
        Ok(detail.into())
    }

    /// Asks the site for the chapters, keeps them, and returns them with how far each was read.
    pub async fn chapters(&self, source: String, id: String) -> Result<ChapterList> {
        if source == DEVICE {
            return Ok(ChapterList {
                entries: self.entries(&source, &id)?,
                new_ids: Vec::new(),
            });
        }
        let fetched = self.source(&source)?.chapters(&id).await?;
        let synced = self.store.sync_chapters(&source, &id, &fetched)?;
        self.store.set_checked(&source, &id, now_ms())?;
        Ok(ChapterList {
            entries: self.entries(&source, &id)?,
            new_ids: synced.new,
        })
    }

    /// What the site last said about a series, to show before asking it again.
    pub fn cached_details(&self, source: String, id: String) -> Result<Option<Detail>> {
        Ok(self.store.detail(&source, &id)?.map(Detail::from))
    }

    pub async fn pages(&self, source: String, series: String, chapter: String) -> Result<PageList> {
        // A chapter on the device (imported, or downloaded in full) is read from its files.
        if source == DEVICE && plain_name(&series) {
            let kept = downloads::pages(&self.dir.join("local").join(&series));
            if !kept.is_empty() {
                return Ok(PageList {
                    images: kept.iter().map(|page| file_url(page)).collect(),
                    reading: Reading::Unknown,
                    headers: Vec::new(),
                });
            }
        } else if source != DEVICE && self.store.is_downloaded(&source, &series, &chapter)? {
            let folder = downloads::folder(&self.dir.join("downloads"), &source, &series, &chapter);
            let kept = downloads::pages(&folder);
            if !kept.is_empty() {
                let reading = match std::fs::read_to_string(folder.join("reading.txt"))
                    .unwrap_or_default()
                    .trim()
                {
                    "long_strip" => Reading::LongStrip,
                    "paged" => Reading::Paged,
                    "paged_rtl" => Reading::PagedRtl,
                    _ => Reading::Unknown,
                };
                return Ok(PageList {
                    images: kept.iter().map(|page| file_url(page)).collect(),
                    reading,
                    headers: Vec::new(),
                });
            }
        }
        let site = self.source(&source)?;
        let key = format!("{source}/{series}/{chapter}");
        let turn = self
            .page_turns
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_default()
            .clone();
        let result = {
            let _held = turn.lock().await;
            // Asked while another ask for it was running: that one has put its answer here by now.
            let cached = self.page_lists.lock().unwrap().get(&key, Instant::now());
            match cached {
                Some(list) => Ok(list),
                None => {
                    let fetched = site.pages(&series, &chapter).await;
                    fetched.map(PageList::from).inspect(|list| {
                        self.page_lists.lock().unwrap().put(
                            key.clone(),
                            Instant::now(),
                            list.clone(),
                        );
                    })
                }
            }
        };
        // The map's own handle and this one: nobody else is waiting, so the turn can go.
        let mut turns = self.page_turns.lock().unwrap();
        if turns
            .get(&key)
            .is_some_and(|held| Arc::strong_count(held) <= 2)
        {
            turns.remove(&key);
        }
        Ok(result?)
    }

    // --- the library ---------------------------------------------------------------------------

    /// The chapters last fetched, with progress; no network.
    pub fn cached_chapters(&self, source: String, series: String) -> Result<Vec<ChapterEntry>> {
        self.entries(&source, &series)
    }

    pub async fn set_in_library(&self, source: String, id: String, saved: bool) -> Result<()> {
        if !saved {
            return Ok(self.store.remove_from_library(&source, &id)?);
        }
        if self.store.add_to_library(&source, &id, now_ms()).is_err() {
            self.details(source.clone(), id.clone()).await?;
            self.store.add_to_library(&source, &id, now_ms())?;
        }
        Ok(())
    }

    pub fn in_library(&self, source: String, id: String) -> Result<bool> {
        Ok(self.store.in_library(&source, &id)?)
    }

    pub fn library(&self) -> Result<Vec<LibraryEntry>> {
        Ok(self
            .store
            .library()?
            .into_iter()
            .map(|item| LibraryEntry {
                source: item.source,
                id: item.id,
                title: item.title,
                cover: self.shown_cover(item.cover),
                unread: item.unread,
                total: item.total,
                last_read_ms: item.last_read_ms,
                saved_ms: item.saved_ms,
                categories: item.categories,
                checked_ms: item.checked_ms,
                completed: item.completed,
            })
            .collect())
    }

    /// Every saved series with its newest chapter, in one read: the Updates screen.
    pub fn updates(&self) -> Result<Vec<UpdateEntry>> {
        Ok(self
            .store
            .updates()?
            .into_iter()
            .map(|item| UpdateEntry {
                source: item.source,
                id: item.id,
                title: item.title,
                cover: self.shown_cover(item.cover),
                unread: item.unread,
                latest_number: item.latest_number,
                latest_title: item.latest_title,
                latest_date_ms: item.latest_date_ms,
            })
            .collect())
    }

    /// When a series' chapters were last asked of its site, to skip asking again at once.
    pub fn chapters_checked_ms(&self, source: String, series: String) -> Result<Option<i64>> {
        Ok(self.store.checked(&source, &series)?)
    }

    /// Notes an attempt to ask a site for a series' chapters that failed, so the next refresh waits instead of asking again at once.
    pub fn note_failed_check(&self, source: String, series: String) -> Result<()> {
        Ok(self.store.set_checked(&source, &series, now_ms())?)
    }

    pub fn set_progress(
        &self,
        source: String,
        series: String,
        chapter: String,
        page: u32,
        offset: f64,
        finished: bool,
    ) -> Result<()> {
        Ok(self.store.set_progress(
            &source,
            &series,
            &chapter,
            Progress {
                page,
                offset,
                finished,
            },
            now_ms(),
        )?)
    }

    // --- covers --------------------------------------------------------------------------------

    /// Makes a picture (a small JPEG) the series' cover, in place of the one the source gives, until it is cleared.
    pub fn set_custom_cover(&self, source: String, series: String, jpeg: Vec<u8>) -> Result<()> {
        if jpeg.len() > MAX_COVER || !jpeg.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Err(failed("that is not a picture the app can use as a cover"));
        }
        std::fs::create_dir_all(self.dir.join("covers"))
            .map_err(|error| failed(error.to_string()))?;
        // The time in milliseconds, or the next number free: two covers set in one millisecond must not share a file.
        let mut number = now_ms();
        while self.cover_file(&format!("{CUSTOM}{number}.jpg")).exists() {
            number += 1;
        }
        let name = format!("{CUSTOM}{number}.jpg");
        std::fs::write(self.cover_file(&name), &jpeg).map_err(|error| failed(error.to_string()))?;
        match self.store.set_custom_cover(&source, &series, Some(&name)) {
            Ok(old) => {
                if let Some(old) = old.filter(|old| *old != name) {
                    let _ = std::fs::remove_file(self.cover_file(&old));
                }
                Ok(())
            }
            Err(error) => {
                let _ = std::fs::remove_file(self.cover_file(&name));
                Err(error.into())
            }
        }
    }

    /// Goes back to the cover the source gives.
    pub fn clear_custom_cover(&self, source: String, series: String) -> Result<()> {
        if let Some(old) = self.store.set_custom_cover(&source, &series, None)? {
            let _ = std::fs::remove_file(self.cover_file(&old));
        }
        Ok(())
    }

    /// The address of the cover the person chose for a series, if they did.
    pub fn custom_cover(&self, source: String, series: String) -> Result<Option<String>> {
        Ok(self.shown_cover(self.store.custom_cover(&source, &series)?))
    }

    /// Marks a page; marking it again moves the mark within the page.
    pub fn add_bookmark(
        &self,
        source: String,
        series: String,
        chapter: String,
        page: u32,
        offset: f64,
    ) -> Result<()> {
        Ok(self
            .store
            .add_bookmark(&source, &series, &chapter, page, offset, now_ms())?)
    }

    pub fn remove_bookmark(
        &self,
        source: String,
        series: String,
        chapter: String,
        page: u32,
    ) -> Result<()> {
        Ok(self
            .store
            .remove_bookmark(&source, &series, &chapter, page)?)
    }

    /// The pages marked in a series, newest first.
    pub fn bookmarks(&self, source: String, series: String) -> Result<Vec<Bookmark>> {
        Ok(self
            .store
            .bookmarks(&source, &series)?
            .into_iter()
            .map(|row| Bookmark {
                chapter: row.chapter,
                page: row.page,
                offset: row.offset,
                created_ms: row.created_at,
                chapter_number: row.chapter_number,
                chapter_title: row.chapter_title,
            })
            .collect())
    }

    pub fn mark(
        &self,
        source: String,
        series: String,
        chapters: Vec<String>,
        read: bool,
    ) -> Result<()> {
        let ids: Vec<&str> = chapters.iter().map(String::as_str).collect();
        Ok(self.store.mark(&source, &series, &ids, read, now_ms())?)
    }

    pub fn resume(&self, source: String, series: String) -> Result<Option<Resume>> {
        Ok(self.store.resume(&source, &series)?.map(|resume| Resume {
            chapter: resume.chapter,
            page: resume.page,
            offset: resume.offset,
        }))
    }

    // --- categories ----------------------------------------------------------------------------

    pub fn categories(&self) -> Result<Vec<CategoryInfo>> {
        Ok(self
            .store
            .categories()?
            .into_iter()
            .map(|category| CategoryInfo {
                id: category.id,
                name: category.name,
                count: category.count,
            })
            .collect())
    }

    pub fn add_category(&self, name: String) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(failed("a category needs a name"));
        }
        Ok(self.store.add_category(name)?)
    }

    pub fn rename_category(&self, id: i64, name: String) -> Result<()> {
        Ok(self.store.rename_category(id, name.trim())?)
    }

    /// Puts the categories in the order given (see `Store::reorder_categories`).
    pub fn reorder_categories(&self, ids: Vec<i64>) -> Result<()> {
        Ok(self.store.reorder_categories(&ids)?)
    }

    pub fn delete_category(&self, id: i64) -> Result<()> {
        Ok(self.store.delete_category(id)?)
    }

    pub fn set_category(
        &self,
        source: String,
        series: String,
        category: i64,
        member: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .put_in_category(&source, &series, category, member)?)
    }

    // --- moving and removing -------------------------------------------------------------------

    /// Moves a series' library entry, categories and read chapters to the same series on another source.
    /// The other series must have been opened once (so its chapters are known). Returns the chapters carried over.
    pub fn migrate(
        &self,
        source: String,
        series: String,
        to_source: String,
        to_series: String,
    ) -> Result<u32> {
        Ok(self
            .store
            .migrate(&source, &series, &to_source, &to_series, now_ms())?)
    }

    /// Forgets a series entirely: library entry, read state and the downloads it had.
    pub fn forget_series(&self, source: String, series: String) -> Result<()> {
        for row in self
            .store
            .downloads()?
            .into_iter()
            .filter(|row| row.source == source && row.series == series)
        {
            let _ = std::fs::remove_dir_all(downloads::folder(
                &self.dir.join("downloads"),
                &row.source,
                &row.series,
                &row.chapter,
            ));
        }
        if let Some(name) = self.store.custom_cover(&source, &series)? {
            let _ = std::fs::remove_file(self.cover_file(&name));
        }
        if source == DEVICE && plain_name(&series) {
            let _ = std::fs::remove_dir_all(self.dir.join("local").join(&series));
        }
        Ok(self.store.delete_series(&source, &series)?)
    }

    // --- backup --------------------------------------------------------------------------------

    /// Everything that is the person's, as one text: library, read state, categories, the repositories and
    /// the sources installed (by repository where there is one, whole where there is not), and source settings.
    pub fn backup(&self) -> Result<String> {
        let mut sources = Vec::new();
        for (id, repo) in self.store.origins()? {
            let document = match &repo {
                None => {
                    std::fs::read_to_string(self.dir.join("sources").join(format!("{id}.json")))
                        .ok()
                        .and_then(|text| serde_json::from_str(&text).ok())
                }
                Some(_) => None,
            };
            sources.push(BackupSource {
                settings: self.store.settings(&id)?,
                id,
                repo,
                document,
            });
        }
        let library = self.store.export()?;
        let mut covers = BTreeMap::new();
        for name in library
            .series
            .iter()
            .filter_map(|series| series.custom_cover.as_deref())
        {
            if let Ok(bytes) = std::fs::read(self.cover_file(name)) {
                covers.insert(name.to_string(), BASE64.encode(bytes));
            }
        }
        let backup = Backup {
            version: 1,
            library,
            covers,
            repos: self
                .store
                .repos()?
                .into_iter()
                .map(|(base, _, _)| base)
                .collect(),
            sources,
        };
        serde_json::to_string(&backup).map_err(|error| failed(error.to_string()))
    }

    /// Puts a backup back: the library at once, then the repositories and sources as far as they can be
    /// reached. What could not come back is reported, not hidden.
    pub async fn restore(&self, text: String) -> Result<RestoreReport> {
        let backup: Backup = serde_json::from_str(&text)
            .map_err(|error| failed(format!("that is not a backup: {error}")))?;
        if backup.version != 1 {
            return Err(failed(format!(
                "this backup is version {}; this app reads version 1",
                backup.version
            )));
        }
        let library = backup.library;
        self.store.import(&library)?;
        // Then the pictures the library now names: one for a series that already had a cover of its own is not wanted, and
        // one that cannot be read or written leaves that series with the source's cover.
        for series in &library.series {
            let Some(name) = series.custom_cover.as_deref() else {
                continue;
            };
            if self
                .store
                .custom_cover(&series.source, &series.id)?
                .as_deref()
                != Some(name)
            {
                continue;
            }
            let bytes = custom_name_is_sound(name)
                .then(|| backup.covers.get(name))
                .flatten()
                .and_then(|text| BASE64.decode(text).ok())
                .filter(|bytes| bytes.len() <= MAX_COVER && bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
            let written = bytes.is_some_and(|bytes| {
                std::fs::create_dir_all(self.dir.join("covers")).is_ok()
                    && std::fs::write(self.cover_file(name), bytes).is_ok()
            });
            if !written {
                self.store
                    .set_custom_cover(&series.source, &series.id, None)?;
            }
        }
        for base in &backup.repos {
            let _ = self.add_repo(base.clone()).await;
        }
        let mut report = RestoreReport {
            series: library.series.len() as u32,
            sources_installed: Vec::new(),
            sources_missing: Vec::new(),
        };
        for source in &backup.sources {
            let installed = self.installed_version(&source.id).is_some();
            let outcome = match (&source.document, &source.repo) {
                _ if installed => Ok(()),
                (Some(document), _) => self
                    .install_document(&document.to_string(), false)
                    .map(|info| self.store.set_origin(&info.id, None, info.version))
                    .map(|_| ()),
                (None, Some(base)) => match repo::fetch_index(base).await {
                    Ok(fetched) => self
                        .install_one(base, &fetched.index, &source.id)
                        .await
                        .map(|_| ())
                        .map_err(MangaError::from),
                    Err(error) => Err(error.into()),
                },
                (None, None) => Err(failed("no way to get it back")),
            };
            match outcome {
                Ok(()) => {
                    for (key, value) in &source.settings {
                        let _ = self.set_setting(source.id.clone(), key.clone(), value.clone());
                    }
                    report.sources_installed.push(source.id.clone());
                }
                Err(_) => report.sources_missing.push(source.id.clone()),
            }
        }
        Ok(report)
    }

    // --- comics on the device ------------------------------------------------------------------

    /// Adds a comic archive (CBZ) from the device as a series of the `device` source and saves it to the library.
    /// Returns the series id.
    pub fn import_file(&self, path: String) -> Result<String> {
        let imported = local::import(std::path::Path::new(&path), &self.dir.join("local"))?;
        self.store.upsert_series(
            DEVICE,
            &imported.id,
            &imported.title,
            imported.pages.first().map(|page| file_url(page)).as_deref(),
            None,
            m::Status::Unknown,
        )?;
        let chapter = m::Chapter {
            id: "1".into(),
            number: Some(1.0),
            title: Some(imported.title.clone()),
            date_ms: None,
            scanlator: None,
        };
        self.store.sync_chapters(DEVICE, &imported.id, &[chapter])?;
        self.store.add_to_library(DEVICE, &imported.id, now_ms())?;
        Ok(imported.id)
    }

    // --- downloads -----------------------------------------------------------------------------

    /// Keeps a chapter on the device. Returns when it is done; a failed one is recorded as such.
    pub async fn download_chapter(
        &self,
        source: String,
        series: String,
        chapter: String,
    ) -> Result<()> {
        let key = format!("{source}/{series}/{chapter}");
        if !self.downloading.lock().unwrap().insert(key.clone()) {
            return Ok(());
        }
        let result = self.download_one(&source, &series, &chapter).await;
        self.downloading.lock().unwrap().remove(&key);
        result
    }

    pub fn downloads(&self) -> Result<Vec<DownloadInfo>> {
        Ok(self
            .store
            .downloads()?
            .into_iter()
            .map(|row| DownloadInfo {
                source: row.source,
                series: row.series,
                chapter: row.chapter,
                state: row.state,
                pages: row.pages,
                bytes: row.bytes,
            })
            .collect())
    }

    pub fn delete_download(&self, source: String, series: String, chapter: String) -> Result<()> {
        let _ = std::fs::remove_dir_all(downloads::folder(
            &self.dir.join("downloads"),
            &source,
            &series,
            &chapter,
        ));
        Ok(self.store.delete_download(&source, &series, &chapter)?)
    }
}

impl Engine {
    fn cover_file(&self, name: &str) -> PathBuf {
        self.dir
            .join("covers")
            .join(name.trim_start_matches(CUSTOM))
    }

    /// What a listing's `cover` means on screen: an address, or the name of a chosen picture, which becomes a file address
    /// (nothing, if the picture is gone).
    fn shown_cover(&self, cover: Option<String>) -> Option<String> {
        let cover = cover?;
        if !custom_name_is_sound(&cover) {
            return Some(cover);
        }
        // A picture that is gone (a restore that lacked it, iOS clearing storage) is no cover, not a broken one.
        let file = self.cover_file(&cover);
        file.exists().then(|| file_url(&file))
    }

    async fn download_one(&self, source: &str, series: &str, chapter: &str) -> Result<()> {
        let (source, series, chapter) =
            (source.to_string(), series.to_string(), chapter.to_string());
        let mut row = akidev_manga::store::DownloadRow {
            source: source.clone(),
            series: series.clone(),
            chapter: chapter.clone(),
            state: "active".into(),
            pages: 0,
            bytes: 0,
        };
        self.store.set_download(&row)?;
        let outcome = async {
            let list = self.source(&source)?.pages(&series, &chapter).await?;
            let folder = downloads::folder(&self.dir.join("downloads"), &source, &series, &chapter);
            let client = akidev_manga::http::client_builder()
                .timeout(std::time::Duration::from_secs(60))
                .connect_timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            let headers: Vec<(String, String)> = list.headers.into_iter().collect();
            let bytes = downloads::fetch(&client, &list.images, &headers, &folder, |done| {
                row.pages = done;
                let _ = self.store.set_download(&row);
            })
            .await?;
            let reading = match list.reading {
                m::Reading::LongStrip => "long_strip",
                m::Reading::Paged => "paged",
                m::Reading::PagedRtl => "paged_rtl",
                m::Reading::Unknown => "unknown",
            };
            std::fs::write(folder.join("reading.txt"), reading)?;
            anyhow::Ok(bytes)
        }
        .await;
        match outcome {
            Ok(bytes) => {
                row.state = "done".into();
                row.bytes = bytes;
                self.store.set_download(&row)?;
                Ok(())
            }
            Err(error) => {
                row.state = "failed".into();
                self.store.set_download(&row)?;
                Err(error.into())
            }
        }
    }
}

#[uniffi::export]
impl Engine {
    pub fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>> {
        Ok(self
            .store
            .history(limit)?
            .into_iter()
            .map(|item| HistoryEntry {
                source: item.source,
                series: item.series,
                title: item.title,
                cover: self.shown_cover(item.cover),
                chapter: item.chapter,
                chapter_number: item.chapter_number,
                chapter_title: item.chapter_title,
                at_ms: item.at_ms,
                finished: item.finished,
            })
            .collect())
    }
}

impl Engine {
    fn entries(&self, source: &str, series: &str) -> Result<Vec<ChapterEntry>> {
        Ok(self
            .store
            .chapters(source, series)?
            .into_iter()
            .map(|row| ChapterEntry {
                id: row.chapter.id,
                number: row.chapter.number,
                title: row.chapter.title,
                date_ms: row.chapter.date_ms,
                scanlator: row.chapter.scanlator,
                finished: row.finished,
                page: row.page,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(first: &str) -> PageList {
        PageList {
            images: vec![first.to_string()],
            reading: Reading::Unknown,
            headers: Vec::new(),
        }
    }

    #[test]
    fn a_page_list_is_reused_for_ten_minutes_and_the_oldest_goes_first() {
        let start = Instant::now();
        let mut cache = PageCache::default();
        assert!(cache.get("a", start).is_none());
        cache.put("a".into(), start, list("one"));
        assert_eq!(cache.get("a", start).unwrap().images, ["one"]);
        assert!(cache.get("a", start + Duration::from_secs(599)).is_some());
        assert!(cache.get("a", start + Duration::from_secs(601)).is_none());
        // Putting the same chapter again replaces it instead of keeping two.
        cache.put("a".into(), start, list("two"));
        assert_eq!(cache.get("a", start).unwrap().images, ["two"]);
        assert_eq!(cache.lists.len(), 1);
        // Beyond the most it keeps, the oldest is dropped.
        for n in 0..PageCache::MOST {
            cache.put(format!("k{n}"), start, list("x"));
        }
        assert_eq!(cache.lists.len(), PageCache::MOST);
        assert!(cache.get("a", start).is_none());
        assert!(cache.get("k0", start).is_some());
    }
}

// ---------------------------------------------------------------------------------------------
// Books
// ---------------------------------------------------------------------------------------------

/// A book on the shelf.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookInfo {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub rtl: bool,
    pub bytes: u64,
    pub cover: Option<String>,
    pub added_ms: i64,
    pub opened_ms: Option<i64>,
    /// Where the person is: the chapter, the place in it (see `BookBlock.start`), and how far through the book.
    pub chapter: u32,
    pub offset: u32,
    pub progress: f64,
    pub finished: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookEntry {
    pub title: String,
    pub depth: u32,
    pub chapter: u32,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookChapter {
    pub blocks: Vec<BookBlock>,
    pub anchors: Vec<BookAnchor>,
    pub fonts: Vec<BookFace>,
    pub vertical: bool,
}

/// A font the book carries: the name its styles use, where its file is (ask for it with `resource`), and its style.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookFace {
    pub family: String,
    pub path: String,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookAnchor {
    pub id: String,
    pub offset: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookBlock {
    pub start: u32,
    pub kind: BookKind,
    pub heading: u8,
    pub align: BookAlign,
    pub left: f32,
    pub first_line: f32,
    pub space_before: f32,
    pub page_break_before: bool,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BookKind {
    Text { runs: Vec<BookRun> },
    Image { src: String, ink: bool },
    Rule,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum BookAlign {
    Start,
    Center,
    End,
    Justify,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub small_caps: bool,
    pub mono: bool,
    pub shift: i8,
    pub size: f32,
    pub family: Option<String>,
    pub link: Option<String>,
}

/// What a chapter is, so the reader can open a book at its start and fold the rest away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BookSection {
    Cover,
    Contents,
    Front,
    Body,
    Back,
}

/// A place a search found.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookHit {
    pub chapter: u32,
    pub offset: u32,
    pub length: u32,
    pub snippet: String,
    /// The words are near the ones asked for, not the same.
    pub similar: bool,
}

/// A highlight, with the reader's note if there is one.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookHighlight {
    pub id: i64,
    pub chapter: u32,
    pub start: u32,
    pub end: u32,
    pub colour: u8,
    pub note: Option<String>,
    pub quote: String,
}

impl From<BookMarkRow> for BookHighlight {
    fn from(row: BookMarkRow) -> BookHighlight {
        BookHighlight {
            id: row.id,
            chapter: row.chapter,
            start: row.start,
            end: row.end,
            colour: row.colour,
            note: row.note,
            quote: row.quote,
        }
    }
}

/// A catalogue of books the person added.
#[derive(Debug, Clone, uniffi::Record)]
pub struct CatalogueInfo {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CatalogueFeed {
    pub title: String,
    pub items: Vec<CatalogueItem>,
    pub next: Option<String>,
    pub search: Option<String>,
}

/// A book to get or a shelf to open in a catalogue.
#[derive(Debug, Clone, uniffi::Record)]
pub struct CatalogueItem {
    pub title: String,
    pub author: Option<String>,
    pub cover: Option<String>,
    pub open: Option<String>,
    pub get: Option<String>,
}

impl From<akidev_manga::book::opds::Feed> for CatalogueFeed {
    fn from(feed: akidev_manga::book::opds::Feed) -> CatalogueFeed {
        CatalogueFeed {
            title: feed.title,
            items: feed
                .items
                .into_iter()
                .map(|item| CatalogueItem {
                    title: item.title,
                    author: item.author,
                    cover: item.cover,
                    open: item.open,
                    get: item.get,
                })
                .collect(),
            next: feed.next,
            search: feed.search,
        }
    }
}

/// Where a link inside a book goes.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookTarget {
    pub chapter: u32,
    pub anchor: Option<String>,
}

impl From<akidev_manga::book::Chapter> for BookChapter {
    fn from(chapter: akidev_manga::book::Chapter) -> BookChapter {
        use akidev_manga::book as b;
        BookChapter {
            vertical: chapter.vertical,
            fonts: chapter
                .fonts
                .into_iter()
                .map(|face| BookFace {
                    family: face.family,
                    path: face.path,
                    bold: face.bold,
                    italic: face.italic,
                })
                .collect(),
            anchors: chapter
                .anchors
                .into_iter()
                .map(|anchor| BookAnchor {
                    id: anchor.id,
                    offset: anchor.offset,
                })
                .collect(),
            blocks: chapter
                .blocks
                .into_iter()
                .map(|block| BookBlock {
                    start: block.start,
                    kind: match block.kind {
                        b::Kind::Text(runs) => BookKind::Text {
                            runs: runs
                                .into_iter()
                                .map(|run| BookRun {
                                    text: run.text,
                                    bold: run.look.bold,
                                    italic: run.look.italic,
                                    underline: run.look.underline,
                                    strike: run.look.strike,
                                    small_caps: run.look.small_caps,
                                    mono: run.look.mono,
                                    shift: run.look.shift,
                                    size: run.look.size,
                                    family: run.look.family,
                                    link: run.look.link,
                                })
                                .collect(),
                        },
                        b::Kind::Image { src, ink } => BookKind::Image { src, ink },
                        b::Kind::Rule => BookKind::Rule,
                    },
                    heading: block.heading,
                    align: match block.align {
                        b::Align::Start => BookAlign::Start,
                        b::Align::Center => BookAlign::Center,
                        b::Align::End => BookAlign::End,
                        b::Align::Justify => BookAlign::Justify,
                    },
                    left: block.left,
                    first_line: block.first_line,
                    space_before: block.space_before,
                    page_break_before: block.page_break_before,
                })
                .collect(),
        }
    }
}

/// An open book: its chapters are read one at a time, when asked for.
#[derive(uniffi::Object)]
pub struct BookReader {
    book: std::sync::Mutex<Box<dyn akidev_manga::book::BookFile>>,
}

#[uniffi::export]
impl BookReader {
    pub fn chapter_count(&self) -> u32 {
        self.book.lock().unwrap().chapters() as u32
    }

    /// What each chapter is.
    pub fn sections(&self) -> Vec<BookSection> {
        use akidev_manga::book::Section;
        self.book
            .lock()
            .unwrap()
            .sections()
            .iter()
            .map(|section| match section {
                Section::Cover => BookSection::Cover,
                Section::Contents => BookSection::Contents,
                Section::Front => BookSection::Front,
                Section::Body => BookSection::Body,
                Section::Back => BookSection::Back,
            })
            .collect()
    }

    /// The chapter the book itself starts at.
    pub fn start_chapter(&self) -> u32 {
        self.book.lock().unwrap().start() as u32
    }

    /// The size of each chapter's file: how much of the book each is, for "how far through".
    pub fn chapter_sizes(&self) -> Vec<u64> {
        self.book.lock().unwrap().sizes().to_vec()
    }

    pub fn contents(&self) -> Vec<BookEntry> {
        self.book
            .lock()
            .unwrap()
            .contents()
            .iter()
            .map(|entry| BookEntry {
                title: entry.title.clone(),
                depth: entry.depth,
                chapter: entry.chapter,
                anchor: entry.anchor.clone(),
            })
            .collect()
    }

    /// One chapter as blocks. Reading and styling it takes a moment: call it off the main thread.
    pub fn chapter(&self, index: u32) -> Result<BookChapter> {
        Ok(self.book.lock().unwrap().chapter(index as usize)?.into())
    }

    /// The bytes of a picture inside the book, by the path a block gives.
    pub fn resource(&self, path: String) -> Result<Vec<u8>> {
        Ok(self.book.lock().unwrap().resource(&path)?)
    }

    /// The text of the footnote a link points at, when it points at one.
    pub fn note(&self, link: String) -> Option<String> {
        self.book.lock().unwrap().note(&link)
    }

    /// Which chapter a link in the text goes to, when it goes into the book.
    pub fn target(&self, link: String) -> Option<BookTarget> {
        let (chapter, anchor) = self.book.lock().unwrap().target(&link)?;
        Some(BookTarget {
            chapter: chapter as u32,
            anchor,
        })
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Engine {
    /// Adds a catalogue of books by its address, after reading it once to see that it is one.
    pub async fn add_catalogue(&self, url: String) -> Result<CatalogueInfo> {
        let url = url.trim().to_string();
        let url = if url.contains("://") {
            url
        } else {
            format!("https://{url}")
        };
        let feed = akidev_manga::book::opds::fetch(&url).await?;
        let name = if feed.title.is_empty() {
            url.clone()
        } else {
            feed.title
        };
        self.store.add_catalogue(&url, &name, now_ms())?;
        Ok(CatalogueInfo { url, name })
    }

    /// One page of a catalogue.
    pub async fn catalogue_feed(&self, url: String) -> Result<CatalogueFeed> {
        Ok(akidev_manga::book::opds::fetch(&url).await?.into())
    }

    /// The results of a search of a catalogue, from what its `search` said.
    pub async fn catalogue_search(&self, search: String, term: String) -> Result<CatalogueFeed> {
        let address = akidev_manga::book::opds::search_address(&search, &term).await?;
        Ok(akidev_manga::book::opds::fetch(&address).await?.into())
    }

    /// Downloads a book from a catalogue and puts it on the shelf.
    pub async fn get_book(&self, url: String) -> Result<BookInfo> {
        let folder = self.dir.join(format!("get-{}", now_ms()));
        let result = async {
            let file = akidev_manga::book::opds::download(&url, &folder).await?;
            self.import_book(file.to_string_lossy().into_owned())
        }
        .await;
        let _ = std::fs::remove_dir_all(&folder);
        result
    }
}

#[uniffi::export]
impl Engine {
    pub fn catalogues(&self) -> Result<Vec<CatalogueInfo>> {
        Ok(self
            .store
            .catalogues()?
            .into_iter()
            .map(|(url, name)| CatalogueInfo { url, name })
            .collect())
    }

    pub fn remove_catalogue(&self, url: String) -> Result<()> {
        Ok(self.store.remove_catalogue(&url)?)
    }

    /// Adds a book from the device to the shelf. A book already on the shelf is found, not added twice.
    pub fn import_book(&self, path: String) -> Result<BookInfo> {
        let imported = akidev_manga::book::import(
            std::path::Path::new(&path),
            &self.dir.join("books"),
            &self.dir.join("covers"),
        )?;
        self.store.add_book(&BookRow {
            id: imported.id.clone(),
            title: imported.meta.title,
            authors: imported.meta.authors.join("\n"),
            language: imported.meta.language,
            rtl: imported.meta.rtl,
            file: imported.file,
            bytes: imported.bytes as i64,
            cover: imported.cover,
            added_at: now_ms(),
            opened_at: None,
            chapter: 0,
            offset: 0,
            progress: 0.0,
            finished: false,
        })?;
        self.book_info(imported.id)
    }

    /// The shelf, the book read last first.
    pub fn books(&self) -> Result<Vec<BookInfo>> {
        Ok(self
            .store
            .books()?
            .into_iter()
            .map(|row| self.shown_book(row))
            .collect())
    }

    pub fn book_info(&self, id: String) -> Result<BookInfo> {
        let row = self
            .store
            .book(&id)?
            .ok_or_else(|| failed("that book is not on the shelf"))?;
        Ok(self.shown_book(row))
    }

    /// Opens a book to read. Reads its contents list and nothing else.
    pub fn open_book(&self, id: String) -> Result<Arc<BookReader>> {
        let row = self
            .store
            .book(&id)?
            .ok_or_else(|| failed("that book is not on the shelf"))?;
        let book = akidev_manga::book::open(&self.dir.join("books").join(&row.file))?;
        Ok(Arc::new(BookReader {
            book: std::sync::Mutex::new(book),
        }))
    }

    /// Remembers where the person is in a book.
    pub fn save_book_position(
        &self,
        id: String,
        chapter: u32,
        offset: u32,
        progress: f64,
        finished: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .set_book_progress(&id, chapter, offset, progress, finished, now_ms())?)
    }

    /// Every place in a book where the words are written (or, if nowhere, words near them), in reading order. It reads the book
    /// on its own, so it never holds up the page being read; call it off the main thread.
    pub fn search_book(&self, id: String, query: String) -> Result<Vec<BookHit>> {
        let row = self
            .store
            .book(&id)?
            .ok_or_else(|| failed("that book is not on the shelf"))?;
        let hits =
            akidev_manga::book::search(&self.dir.join("books").join(&row.file), &query, 300)?;
        Ok(hits
            .into_iter()
            .map(|hit| BookHit {
                chapter: hit.chapter,
                offset: hit.offset,
                length: hit.length,
                snippet: hit.snippet,
                similar: hit.similar,
            })
            .collect())
    }

    /// Keeps a highlight: the chapter, where it starts and ends, a colour (0 to 3) and the words themselves.
    pub fn add_book_mark(
        &self,
        book: String,
        chapter: u32,
        start: u32,
        end: u32,
        colour: u8,
        quote: String,
    ) -> Result<i64> {
        Ok(self.store.add_book_mark(&BookMarkRow {
            id: 0,
            book,
            chapter,
            start,
            end,
            colour,
            note: None,
            quote,
            created_at: now_ms(),
        })?)
    }

    /// A book's highlights in reading order.
    pub fn book_marks(&self, book: String) -> Result<Vec<BookHighlight>> {
        Ok(self
            .store
            .book_marks(&book)?
            .into_iter()
            .map(BookHighlight::from)
            .collect())
    }

    /// Changes a highlight's colour and note (an empty note is none).
    pub fn set_book_mark(&self, id: i64, colour: u8, note: Option<String>) -> Result<()> {
        Ok(self.store.set_book_mark(id, colour, note.as_deref())?)
    }

    pub fn remove_book_mark(&self, id: i64) -> Result<()> {
        Ok(self.store.remove_book_mark(id)?)
    }

    /// Takes a book off the shelf and deletes its file.
    pub fn remove_book(&self, id: String) -> Result<()> {
        if let Some(row) = self.store.remove_book(&id)? {
            let _ = std::fs::remove_file(self.dir.join("books").join(&row.file));
            if let Some(cover) = row.cover {
                let _ = std::fs::remove_file(self.dir.join("covers").join(cover));
            }
        }
        Ok(())
    }
}

impl Engine {
    fn shown_book(&self, row: BookRow) -> BookInfo {
        let cover = row
            .cover
            .map(|name| self.dir.join("covers").join(name))
            .filter(|file| file.exists())
            .map(|file| file_url(&file));
        BookInfo {
            id: row.id,
            title: row.title,
            authors: row
                .authors
                .lines()
                .map(str::to_string)
                .filter(|name| !name.is_empty())
                .collect(),
            language: row.language,
            rtl: row.rtl,
            bytes: row.bytes as u64,
            cover,
            added_ms: row.added_at,
            opened_ms: row.opened_at,
            chapter: row.chapter,
            offset: row.offset,
            progress: row.progress,
            finished: row.finished,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading aloud
// ---------------------------------------------------------------------------------------------

/// A voice model: one a repository offers, or one on the device.
#[derive(Debug, Clone, uniffi::Record)]
pub struct VoiceModel {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub bytes: u64,
    pub licence: String,
    /// The licence allows use in a product that earns money.
    pub commercial: bool,
    /// This app can run it: it knows the kind of model and is new enough.
    pub usable: bool,
}

/// Told how far a download has got, in bytes.
#[uniffi::export(with_foreign)]
pub trait VoiceProgress: Send + Sync {
    fn update(&self, done: u64, total: u64);
}

/// One sentence of a chapter to read, and where it is.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BookSpeech {
    pub start: u32,
    pub length: u32,
    pub text: String,
    pub heading: bool,
}

/// A voice model loaded and ready to speak.
#[derive(uniffi::Object)]
pub struct VoiceSpeaker {
    speaker: akidev_manga::voice::Speaker,
}

#[uniffi::export]
impl VoiceSpeaker {
    /// The speech for `text`: mono samples at `sample_rate()`. It takes a moment: call it off the main thread.
    pub fn speak(&self, text: String) -> Result<Vec<f32>> {
        Ok(self.speaker.speak(&text)?)
    }

    pub fn sample_rate(&self) -> u32 {
        self.speaker.sample_rate()
    }
}

#[uniffi::export]
impl BookReader {
    /// The sentences of one chapter, in reading order, for reading aloud.
    pub fn speech(&self, chapter: u32) -> Result<Vec<BookSpeech>> {
        let chapter = self.book.lock().unwrap().chapter(chapter as usize)?;
        Ok(akidev_manga::book::speech::units(&chapter)
            .into_iter()
            .map(|unit| BookSpeech {
                start: unit.start,
                length: unit.length,
                text: unit.text,
                heading: unit.heading,
            })
            .collect())
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Engine {
    /// The voice models a repository offers. Reading the index is also how the app learns the repository can be
    /// reached: an error means it cannot.
    pub async fn voices_offered(&self, link: String) -> Result<Vec<VoiceModel>> {
        let base = repo::base_of(&link)?;
        let index = repo::fetch_index(&base).await?.index;
        Ok(index
            .voices
            .into_iter()
            .map(|offered| VoiceModel {
                usable: akidev_manga::voice::runs(&offered, APP_VERSION),
                id: offered.id,
                name: offered.name,
                languages: offered.languages,
                bytes: offered.bytes,
                licence: offered.licence,
                commercial: offered.commercial,
            })
            .collect())
    }

    /// Downloads a model a repository offers, checks it and keeps it on the device.
    pub async fn install_voice(
        &self,
        link: String,
        id: String,
        progress: Arc<dyn VoiceProgress>,
    ) -> Result<()> {
        let base = repo::base_of(&link)?;
        let offered = repo::fetch_index(&base)
            .await?
            .index
            .voices
            .into_iter()
            .find(|voice| voice.id == id)
            .ok_or_else(|| failed(format!("the repository has no model called {id}")))?;
        if !self.installing.lock().unwrap().insert(id.clone()) {
            return Err(failed("that model is already downloading"));
        }
        let _downloading = Downloading {
            set: &self.installing,
            id,
        };
        akidev_manga::voice::install(
            &self.voices_dir(),
            &base,
            &offered,
            APP_VERSION,
            |done, total| progress.update(done, total),
        )
        .await?;
        Ok(())
    }
}

#[uniffi::export]
impl Engine {
    pub fn voices_installed(&self) -> Vec<VoiceModel> {
        akidev_manga::voice::installed(&self.voices_dir())
            .into_iter()
            .map(|model| VoiceModel {
                bytes: model.def.files.iter().map(|file| file.bytes).sum(),
                id: model.def.id,
                name: model.def.name,
                languages: model.def.languages,
                licence: model.def.licence.name,
                commercial: model.def.licence.commercial,
                usable: true,
            })
            .collect()
    }

    pub fn remove_voice(&self, id: String) -> Result<()> {
        Ok(akidev_manga::voice::remove(&self.voices_dir(), &id)?)
    }

    /// Loads an installed model so it can speak. It takes a moment: call it off the main thread.
    pub fn open_speaker(&self, id: String) -> Result<Arc<VoiceSpeaker>> {
        let model = akidev_manga::voice::installed(&self.voices_dir())
            .into_iter()
            .find(|model| model.def.id == id)
            .ok_or_else(|| failed("that voice model is not on this device"))?;
        let speaker = akidev_manga::voice::Speaker::open(&model.folder, &model.def.lang)?;
        Ok(Arc::new(VoiceSpeaker { speaker }))
    }
}

/// A model being downloaded: it leaves the set when this goes, even if the download is abandoned half way.
struct Downloading<'a> {
    set: &'a std::sync::Mutex<std::collections::HashSet<String>>,
    id: String,
}

impl Drop for Downloading<'_> {
    fn drop(&mut self) {
        self.set.lock().unwrap().remove(&self.id);
    }
}

impl Engine {
    fn voices_dir(&self) -> PathBuf {
        self.dir.join("voices")
    }
}
