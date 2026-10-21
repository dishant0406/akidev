//! The Swift-facing face of the manga engine: one `Engine` object holding the sources, the library
//! database and the settings a challenge hands over. Every type here mirrors one in `akidev-manga`;
//! uniffi cannot derive across crates, so they are converted at this edge and nowhere else.

use akidev_manga::fetch::{Challenge, Unreachable};
use akidev_manga::model as m;
use akidev_manga::repo;
use akidev_manga::source::{Info, Source};
use akidev_manga::store::{Export, Progress, Store};
use akidev_manga::{downloads, local};
use akidev_manga::theme::Themes;
use anyhow::Context;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

#[derive(Debug, uniffi::Error)]
pub enum MangaError {
    /// The site wants a check first (Cloudflare, a captcha). There is no browser in the app to pass it in, so
    /// the screen says the site is not supported from here.
    Challenge { url: String, status: u16 },
    /// The site could not be reached at all (no network, a blocked address).
    Offline { url: String, reason: String },
    Failed { message: String },
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
            return MangaError::Challenge { url: challenge.url.clone(), status: challenge.status };
        }
        if let Some(down) = error.downcast_ref::<Unreachable>() {
            return MangaError::Offline { url: down.url.clone(), reason: down.reason.clone() };
        }
        MangaError::Failed { message: format!("{error:#}") }
    }
}

type Result<T> = std::result::Result<T, MangaError>;

fn failed(message: impl Into<String>) -> MangaError {
    MangaError::Failed { message: message.into() }
}

// ---------------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
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
        Card { id: card.id, title: card.title, cover: card.cover }
    }
}

impl From<m::Page<m::Card>> for CardPage {
    fn from(page: m::Page<m::Card>) -> CardPage {
        CardPage { items: page.items.into_iter().map(Card::from).collect(), has_next: page.has_next }
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
        Detail { title: detail.title, cover: detail.cover, description: detail.description, authors: detail.authors, tags: detail.tags, status: detail.status.into() }
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
        PageList { images: list.images, reading: list.reading.into(), headers: list.headers.into_iter().map(|(name, value)| Header { name, value }).collect() }
    }
}

impl From<Info> for SourceInfo {
    fn from(info: Info) -> SourceInfo {
        SourceInfo { id: info.id, name: info.name, languages: info.languages, nsfw: info.nsfw, version: info.version, can_popular: info.can_popular, can_latest: info.can_latest, can_search: info.can_search, has_settings: info.has_settings }
    }
}

// ---------------------------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------------------------

/// A name that is safe as a file name: lowercase letters, digits and dashes.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
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
}

impl Engine {
    fn source(&self, id: &str) -> Result<Arc<Source>> {
        self.sources.read().unwrap().get(id).cloned().ok_or_else(|| failed(format!("there is no source called {id}")))
    }

    fn read_dir_json(&self, name: &str) -> Vec<(String, serde_json::Value)> {
        let Ok(entries) = std::fs::read_dir(self.dir.join(name)) else { return Vec::new() };
        let mut found: Vec<(String, serde_json::Value)> = entries
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension()? == "json").then_some(())?;
                let stem = path.file_stem()?.to_str()?.to_string();
                Some((stem, serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?))
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
        std::fs::create_dir_all(self.dir.join(folder)).and_then(|()| std::fs::write(self.dir.join(folder).join(format!("{name}.json")), text)).map_err(|error| failed(error.to_string()))
    }

    fn installed_version(&self, id: &str) -> Option<u64> {
        self.sources.read().unwrap().get(id).map(|source| source.info().version)
    }

    fn view(&self, base: &str, name: &str, signed: bool, index: &repo::Index) -> RepoView {
        let sources = index
            .sources
            .iter()
            .map(|offered| {
                let state = if offered.min_app.as_deref().is_some_and(|wanted| !repo::app_at_least(APP_VERSION, wanted)) {
                    SourceState::NeedsNewerApp
                } else {
                    match self.installed_version(&offered.id) {
                        None => SourceState::NotInstalled,
                        Some(have) if have < offered.version => SourceState::UpdateAvailable,
                        Some(_) => SourceState::Installed,
                    }
                };
                RepoSource { id: offered.id.clone(), name: offered.name.clone(), version: offered.version, languages: offered.lang.clone(), nsfw: offered.nsfw, state }
            })
            .collect();
        RepoView { info: RepoInfo { base: base.to_string(), name: name.to_string(), signed }, sources }
    }

    /// Fetches what a repository offers and installs one source (with the themes it extends).
    async fn install_one(&self, base: &str, index: &repo::Index, source_id: &str) -> anyhow::Result<SourceInfo> {
        let offered = index.sources.iter().find(|offered| offered.id == source_id).with_context(|| format!("the repository has no source called {source_id}"))?;
        if let Some(wanted) = &offered.min_app {
            anyhow::ensure!(repo::app_at_least(APP_VERSION, wanted), "{} needs Aki Manga {wanted} or newer (this is {APP_VERSION})", offered.name);
        }
        let mut themes_changed = false;
        for theme_id in &offered.requires {
            let theme = index.themes.iter().find(|theme| &theme.id == theme_id).with_context(|| format!("the repository has no theme {theme_id}"))?;
            let text = repo::fetch_file(base, &theme.path, &theme.sha256).await?;
            serde_json::from_str::<serde_json::Value>(&text).with_context(|| format!("theme {theme_id} is not JSON"))?;
            let path = self.dir.join("themes").join(format!("{theme_id}.json"));
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                self.save("themes", theme_id, &text).map_err(|error| anyhow::anyhow!("{error}"))?;
                themes_changed = true;
            }
        }
        if themes_changed {
            self.reload();
        }
        let text = repo::fetch_file(base, &offered.path, &offered.sha256).await?;
        let info = self.install_text(&text).map_err(|error| anyhow::anyhow!("{error}"))?;
        anyhow::ensure!(info.id == offered.id && info.version == offered.version, "the file for {} is not the source the index lists (it is {} {})", offered.id, info.id, info.version);
        self.store.set_origin(&info.id, Some(base), info.version)?;
        Ok(info)
    }

    /// Installs a source from its document, checking it first.
    fn install_text(&self, json: &str) -> Result<SourceInfo> {
        let document: serde_json::Value = serde_json::from_str(json).map_err(|error| failed(format!("the definition is not JSON: {error}")))?;
        let source = self.build(document)?;
        let info = source.info();
        if !plain_name(&info.id) {
            return Err(failed(format!("{} is not an id a source may have", info.id)));
        }
        if let Some(have) = self.installed_version(&info.id)
            && have > info.version
        {
            return Err(failed(format!("{} {have} is installed; this is the older {}", info.name, info.version)));
        }
        self.save("sources", &info.id, json)?;
        self.sources.write().unwrap().insert(info.id.clone(), Arc::new(source));
        Ok(info.into())
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Engine {
    /// Opens (or creates) the library in `data_dir` and loads the themes and sources installed there.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Engine>> {
        let dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&dir).map_err(|error| failed(format!("cannot use {}: {error}", dir.display())))?;
        let store = Store::open(&dir.join("manga.db"))?;
        let engine = Engine { dir, store, themes: RwLock::default(), sources: RwLock::default() };
        engine.reload();
        Ok(Arc::new(engine))
    }

    pub fn version(&self) -> String {
        APP_VERSION.to_string()
    }

    // --- sources -------------------------------------------------------------------------------

    pub fn sources(&self) -> Vec<SourceInfo> {
        self.sources.read().unwrap().values().map(|source| source.info().into()).collect()
    }

    /// Installs a theme (a base definition for sites on the same CMS), kept across launches. Repositories
    /// install their themes themselves; this is for a definition written by hand.
    pub fn install_theme(&self, name: String, json: String) -> Result<()> {
        if !plain_name(&name) {
            return Err(failed("a theme's name is lowercase letters, digits and dashes"));
        }
        serde_json::from_str::<serde_json::Value>(&json).map_err(|error| failed(format!("the theme is not JSON: {error}")))?;
        self.save("themes", &name, &json)?;
        self.reload();
        Ok(())
    }

    /// Installs a source from its definition, checking it first. A source with the same id is replaced
    /// if the new one's version is not lower.
    pub fn install_source(&self, json: String) -> Result<SourceInfo> {
        let info = self.install_text(&json)?;
        self.store.set_origin(&info.id, None, info.version)?;
        Ok(info)
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
                options: setting.options.into_iter().map(|(value, name)| SettingOption { value, name }).collect(),
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
        self.store.add_repo(&base, &fetched.index.name, fetched.index.key.as_deref(), fetched.signed, now_ms())?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    /// What a repository offers, without keeping it: the "add this repository?" sheet.
    pub async fn preview_repo(&self, link: String) -> Result<RepoView> {
        let base = repo::base_of(&link)?;
        let fetched = repo::fetch_index(&base).await?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    pub fn repos(&self) -> Result<Vec<RepoInfo>> {
        Ok(self.store.repos()?.into_iter().map(|(base, name, signed)| RepoInfo { base, name, signed }).collect())
    }

    /// Removes a repository. Sources installed from it stay, but no longer update.
    pub fn remove_repo(&self, base: String) -> Result<()> {
        Ok(self.store.remove_repo(&base)?)
    }

    /// What a repository offers now, and which of it is installed.
    pub async fn repo_sources(&self, base: String) -> Result<RepoView> {
        let fetched = repo::fetch_index(&base).await?;
        self.store.add_repo(&base, &fetched.index.name, fetched.index.key.as_deref(), fetched.signed, now_ms())?;
        Ok(self.view(&base, &fetched.index.name, fetched.signed, &fetched.index))
    }

    /// Installs (or updates) one source from a repository, with the themes it extends. Every file is
    /// checked against the hash the index gives.
    pub async fn install_from_repo(&self, base: String, source: String) -> Result<SourceInfo> {
        let fetched = repo::fetch_index(&base).await?;
        self.store.add_repo(&base, &fetched.index.name, fetched.index.key.as_deref(), fetched.signed, now_ms())?;
        Ok(self.install_one(&base, &fetched.index, &source).await?)
    }

    /// Updates every installed source whose repository offers a newer version. Returns the ids updated.
    /// A repository that cannot be reached is skipped, not a failure.
    pub async fn update_sources(&self) -> Result<Vec<String>> {
        let mut updated = Vec::new();
        for (base, _, _) in self.store.repos()? {
            let Ok(fetched) = repo::fetch_index(&base).await else { continue };
            for offered in &fetched.index.sources {
                let Some((Some(origin), version)) = self.store.origin(&offered.id)? else { continue };
                if origin == base && version < offered.version && self.install_one(&base, &fetched.index, &offered.id).await.is_ok() {
                    updated.push(offered.id.clone());
                }
            }
        }
        Ok(updated)
    }

    /// After the person has a clearance cookie for a site (pasted in, say), or a login: cookies a
    /// source sends. Nothing in the app gets them from a browser.
    pub fn set_cookies(&self, source: String, cookies: Vec<Cookie>) -> Result<()> {
        self.source(&source)?.fetcher().set_cookies(cookies.into_iter().map(|cookie| (cookie.name, cookie.value)));
        Ok(())
    }

    // --- asking a source -----------------------------------------------------------------------

    pub async fn popular(&self, source: String, page: u32) -> Result<CardPage> {
        Ok(self.source(&source)?.popular(page).await?.into())
    }

    pub async fn latest(&self, source: String, page: u32) -> Result<CardPage> {
        Ok(self.source(&source)?.latest(page).await?.into())
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
                options: filter.options.iter().map(|option| SettingOption { value: option.value.clone(), name: option.name.clone() }).collect(),
                default: filter.default.clone().unwrap_or_default(),
            })
            .collect())
    }

    /// A search; `choices` are the filters the person set (the rest take their defaults).
    pub async fn search(&self, source: String, term: String, page: u32, choices: Vec<Choice>) -> Result<CardPage> {
        let chosen = choices.into_iter().map(|choice| (choice.id, choice.value)).collect();
        Ok(self.source(&source)?.search(&term, page, &chosen).await?.into())
    }

    /// The series' page. Also remembers it, so it can be saved to the library.
    pub async fn details(&self, source: String, id: String) -> Result<Detail> {
        if source == DEVICE {
            return Err(failed("a comic on the device has no page to fetch"));
        }
        let detail = self.source(&source)?.details(&id).await?;
        self.store.upsert_series(&source, &id, &detail.title, detail.cover.as_deref(), detail.description.as_deref(), detail.status)?;
        Ok(detail.into())
    }

    /// Asks the site for the chapters, keeps them, and returns them with how far each was read.
    pub async fn chapters(&self, source: String, id: String) -> Result<ChapterList> {
        if source == DEVICE {
            return Ok(ChapterList { entries: self.entries(&source, &id)?, new_ids: Vec::new() });
        }
        let fetched = self.source(&source)?.chapters(&id).await?;
        let synced = self.store.sync_chapters(&source, &id, &fetched)?;
        Ok(ChapterList { entries: self.entries(&source, &id)?, new_ids: synced.new })
    }

    pub async fn pages(&self, source: String, series: String, chapter: String) -> Result<PageList> {
        // A chapter on the device (imported, or downloaded) is read from its files.
        let kept = if source == DEVICE { downloads::pages(&self.dir.join("local").join(&series)) } else { downloads::pages(&downloads::folder(&self.dir.join("downloads"), &source, &series, &chapter)) };
        if !kept.is_empty() {
            return Ok(PageList { images: kept.iter().map(|page| file_url(page)).collect(), reading: Reading::Unknown, headers: Vec::new() });
        }
        Ok(self.source(&source)?.pages(&series, &chapter).await?.into())
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
            .map(|item| LibraryEntry { source: item.source, id: item.id, title: item.title, cover: item.cover, unread: item.unread, total: item.total, last_read_ms: item.last_read_ms, saved_ms: item.saved_ms, categories: item.categories })
            .collect())
    }

    pub fn set_progress(&self, source: String, series: String, chapter: String, page: u32, offset: f64, finished: bool) -> Result<()> {
        Ok(self.store.set_progress(&source, &series, &chapter, Progress { page, offset, finished }, now_ms())?)
    }

    pub fn mark(&self, source: String, series: String, chapters: Vec<String>, read: bool) -> Result<()> {
        let ids: Vec<&str> = chapters.iter().map(String::as_str).collect();
        Ok(self.store.mark(&source, &series, &ids, read, now_ms())?)
    }

    pub fn resume(&self, source: String, series: String) -> Result<Option<Resume>> {
        Ok(self.store.resume(&source, &series)?.map(|resume| Resume { chapter: resume.chapter, page: resume.page, offset: resume.offset }))
    }

    // --- categories ----------------------------------------------------------------------------

    pub fn categories(&self) -> Result<Vec<CategoryInfo>> {
        Ok(self.store.categories()?.into_iter().map(|category| CategoryInfo { id: category.id, name: category.name, count: category.count }).collect())
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

    pub fn delete_category(&self, id: i64) -> Result<()> {
        Ok(self.store.delete_category(id)?)
    }

    pub fn set_category(&self, source: String, series: String, category: i64, member: bool) -> Result<()> {
        Ok(self.store.put_in_category(&source, &series, category, member)?)
    }

    // --- moving and removing -------------------------------------------------------------------

    /// Moves a series' library entry, categories and read chapters to the same series on another source.
    /// The other series must have been opened once (so its chapters are known). Returns the chapters carried over.
    pub fn migrate(&self, source: String, series: String, to_source: String, to_series: String) -> Result<u32> {
        Ok(self.store.migrate(&source, &series, &to_source, &to_series, now_ms())?)
    }

    /// Forgets a series entirely: library entry, read state and the downloads it had.
    pub fn forget_series(&self, source: String, series: String) -> Result<()> {
        for row in self.store.downloads()?.into_iter().filter(|row| row.source == source && row.series == series) {
            let _ = std::fs::remove_dir_all(downloads::folder(&self.dir.join("downloads"), &row.source, &row.series, &row.chapter));
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
                None => std::fs::read_to_string(self.dir.join("sources").join(format!("{id}.json"))).ok().and_then(|text| serde_json::from_str(&text).ok()),
                Some(_) => None,
            };
            sources.push(BackupSource { settings: self.store.settings(&id)?, id, repo, document });
        }
        let backup = Backup { version: 1, library: self.store.export()?, repos: self.store.repos()?.into_iter().map(|(base, _, _)| base).collect(), sources };
        serde_json::to_string(&backup).map_err(|error| failed(error.to_string()))
    }

    /// Puts a backup back: the library at once, then the repositories and sources as far as they can be
    /// reached. What could not come back is reported, not hidden.
    pub async fn restore(&self, text: String) -> Result<RestoreReport> {
        let backup: Backup = serde_json::from_str(&text).map_err(|error| failed(format!("that is not a backup: {error}")))?;
        if backup.version != 1 {
            return Err(failed(format!("this backup is version {}; this app reads version 1", backup.version)));
        }
        self.store.import(&backup.library)?;
        for base in &backup.repos {
            let _ = self.add_repo(base.clone()).await;
        }
        let mut report = RestoreReport { series: backup.library.series.len() as u32, sources_installed: Vec::new(), sources_missing: Vec::new() };
        for source in &backup.sources {
            let installed = self.installed_version(&source.id).is_some();
            let outcome = match (&source.document, &source.repo) {
                _ if installed => Ok(()),
                (Some(document), _) => self.install_text(&document.to_string()).map(|info| self.store.set_origin(&info.id, None, info.version)).map(|_| ()),
                (None, Some(base)) => match repo::fetch_index(base).await {
                    Ok(fetched) => self.install_one(base, &fetched.index, &source.id).await.map(|_| ()).map_err(MangaError::from),
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
        self.store.upsert_series(DEVICE, &imported.id, &imported.title, imported.pages.first().map(|page| file_url(page)).as_deref(), None, m::Status::Unknown)?;
        let chapter = m::Chapter { id: "1".into(), number: Some(1.0), title: Some(imported.title.clone()), date_ms: None, scanlator: None };
        self.store.sync_chapters(DEVICE, &imported.id, &[chapter])?;
        self.store.add_to_library(DEVICE, &imported.id, now_ms())?;
        Ok(imported.id)
    }

    // --- downloads -----------------------------------------------------------------------------

    /// Keeps a chapter on the device. Returns when it is done; a failed one is recorded as such.
    pub async fn download_chapter(&self, source: String, series: String, chapter: String) -> Result<()> {
        let mut row = akidev_manga::store::DownloadRow { source: source.clone(), series: series.clone(), chapter: chapter.clone(), state: "active".into(), pages: 0, bytes: 0 };
        self.store.set_download(&row)?;
        let outcome = async {
            let list = self.source(&source)?.pages(&series, &chapter).await?;
            let folder = downloads::folder(&self.dir.join("downloads"), &source, &series, &chapter);
            let client = reqwest::Client::new();
            let headers: Vec<(String, String)> = list.headers.into_iter().collect();
            let bytes = downloads::fetch(&client, &list.images, &headers, &folder, |done| {
                row.pages = done;
                let _ = self.store.set_download(&row);
            })
            .await?;
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

    pub fn downloads(&self) -> Result<Vec<DownloadInfo>> {
        Ok(self.store.downloads()?.into_iter().map(|row| DownloadInfo { source: row.source, series: row.series, chapter: row.chapter, state: row.state, pages: row.pages, bytes: row.bytes }).collect())
    }

    pub fn delete_download(&self, source: String, series: String, chapter: String) -> Result<()> {
        let _ = std::fs::remove_dir_all(downloads::folder(&self.dir.join("downloads"), &source, &series, &chapter));
        Ok(self.store.delete_download(&source, &series, &chapter)?)
    }

    pub fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>> {
        Ok(self
            .store
            .history(limit)?
            .into_iter()
            .map(|item| HistoryEntry {
                source: item.source,
                series: item.series,
                title: item.title,
                cover: item.cover,
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
