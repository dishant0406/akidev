//! The Swift-facing face of the manga engine: one `Engine` object holding the sources, the library
//! database and the settings a challenge hands over. Every type here mirrors one in `akidev-manga`;
//! uniffi cannot derive across crates, so they are converted at this edge and nowhere else.

use akidev_manga::catalogue::{Catalogue, Info, MANGADEX_ID};
use akidev_manga::fetch::Challenge;
use akidev_manga::mangadex::MangaDex;
use akidev_manga::model as m;
use akidev_manga::source::Source;
use akidev_manga::store::{Progress, Store};
use akidev_manga::theme::Themes;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

#[derive(Debug, uniffi::Error)]
pub enum MangaError {
    /// The site wants a check first (Cloudflare, a captcha): open `url` in a web view, let the person
    /// pass it, then hand the cookies and the web view's user agent to `set_cookies`/`set_user_agent`.
    Challenge { url: String, status: u16 },
    Failed { message: String },
}

impl std::fmt::Display for MangaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MangaError::Challenge { url, .. } => write!(f, "{url} wants a check before it answers"),
            MangaError::Failed { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MangaError {}

impl From<anyhow::Error> for MangaError {
    fn from(error: anyhow::Error) -> MangaError {
        match error.downcast_ref::<Challenge>() {
            Some(challenge) => MangaError::Challenge { url: challenge.url.clone(), status: challenge.status },
            None => MangaError::Failed { message: format!("{error:#}") },
        }
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
    pub chapter: String,
    pub at_ms: i64,
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
        SourceInfo { id: info.id, name: info.name, languages: info.languages, nsfw: info.nsfw, version: info.version, can_popular: info.can_popular, can_latest: info.can_latest, can_search: info.can_search }
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

#[derive(uniffi::Object)]
pub struct Engine {
    dir: PathBuf,
    store: Store,
    themes: RwLock<Themes>,
    sources: RwLock<BTreeMap<String, Arc<Catalogue>>>,
}

impl Engine {
    fn catalogue(&self, id: &str) -> Result<Arc<Catalogue>> {
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

    fn build(&self, document: serde_json::Value) -> anyhow::Result<Catalogue> {
        let def = self.themes.read().unwrap().resolve(document)?;
        Ok(Catalogue::Definition(Box::new(Source::live(def)?)))
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
        engine.sources.write().unwrap().insert(MANGADEX_ID.into(), Arc::new(Catalogue::MangaDex(Box::new(MangaDex::live()))));
        for (name, theme) in engine.read_dir_json("themes") {
            engine.themes.write().unwrap().insert(&name, theme);
        }
        for (name, document) in engine.read_dir_json("sources") {
            // A saved source that no longer loads (its theme changed) is left out, not a reason to fail.
            match engine.build(document) {
                Ok(catalogue) => {
                    engine.sources.write().unwrap().insert(catalogue.info().id, Arc::new(catalogue));
                }
                Err(error) => eprintln!("akimanga: source {name} did not load: {error:#}"),
            }
        }
        Ok(Arc::new(engine))
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // --- sources -------------------------------------------------------------------------------

    pub fn sources(&self) -> Vec<SourceInfo> {
        self.sources.read().unwrap().values().map(|catalogue| catalogue.info().into()).collect()
    }

    /// Installs a theme (a base definition for sites on the same CMS), kept across launches.
    pub fn install_theme(&self, name: String, json: String) -> Result<()> {
        if !plain_name(&name) {
            return Err(failed("a theme's name is lowercase letters, digits and dashes"));
        }
        let document: serde_json::Value = serde_json::from_str(&json).map_err(|error| failed(format!("the theme is not JSON: {error}")))?;
        self.themes.write().unwrap().insert(&name, document);
        std::fs::create_dir_all(self.dir.join("themes")).and_then(|()| std::fs::write(self.dir.join("themes").join(format!("{name}.json")), json)).map_err(|error| failed(error.to_string()))?;
        Ok(())
    }

    /// Installs a source from its definition, checking it first. A source with the same id is replaced
    /// if the new one's version is not lower.
    pub fn install_source(&self, json: String) -> Result<SourceInfo> {
        let document: serde_json::Value = serde_json::from_str(&json).map_err(|error| failed(format!("the definition is not JSON: {error}")))?;
        let catalogue = self.build(document)?;
        let info = catalogue.info();
        if !plain_name(&info.id) || info.id == MANGADEX_ID {
            return Err(failed(format!("{} is not an id a source may have", info.id)));
        }
        if let Some(have) = self.sources.read().unwrap().get(&info.id) {
            if have.info().version > info.version {
                return Err(failed(format!("{} {} is installed; this is the older {}", info.name, have.info().version, info.version)));
            }
        }
        std::fs::create_dir_all(self.dir.join("sources")).and_then(|()| std::fs::write(self.dir.join("sources").join(format!("{}.json", info.id)), json)).map_err(|error| failed(error.to_string()))?;
        self.sources.write().unwrap().insert(info.id.clone(), Arc::new(catalogue));
        Ok(info.into())
    }

    pub fn remove_source(&self, id: String) -> Result<()> {
        if id == MANGADEX_ID || !plain_name(&id) {
            return Err(failed("that source cannot be removed"));
        }
        self.sources.write().unwrap().remove(&id);
        let _ = std::fs::remove_file(self.dir.join("sources").join(format!("{id}.json")));
        Ok(())
    }

    /// After a check was passed in a web view: the cookies and the user agent the site now expects.
    pub fn set_cookies(&self, source: String, cookies: Vec<Cookie>) -> Result<()> {
        self.catalogue(&source)?.fetcher().set_cookies(cookies.into_iter().map(|cookie| (cookie.name, cookie.value)));
        Ok(())
    }

    pub fn set_user_agent(&self, source: String, user_agent: String) -> Result<()> {
        if !matches!(*self.catalogue(&source)?, Catalogue::MangaDex(_)) {
            self.catalogue(&source)?.fetcher().set_user_agent(&user_agent);
        }
        Ok(())
    }

    // --- asking a source -----------------------------------------------------------------------

    pub async fn popular(&self, source: String, page: u32) -> Result<CardPage> {
        Ok(self.catalogue(&source)?.popular(page).await?.into())
    }

    pub async fn latest(&self, source: String, page: u32) -> Result<CardPage> {
        Ok(self.catalogue(&source)?.latest(page).await?.into())
    }

    pub async fn search(&self, source: String, term: String, page: u32) -> Result<CardPage> {
        Ok(self.catalogue(&source)?.search(&term, page, &BTreeMap::new()).await?.into())
    }

    /// The series' page. Also remembers it, so it can be saved to the library.
    pub async fn details(&self, source: String, id: String) -> Result<Detail> {
        let detail = self.catalogue(&source)?.details(&id).await?;
        self.store.upsert_series(&source, &id, &detail.title, detail.cover.as_deref(), detail.description.as_deref(), detail.status)?;
        Ok(detail.into())
    }

    /// Asks the site for the chapters, keeps them, and returns them with how far each was read.
    pub async fn chapters(&self, source: String, id: String) -> Result<ChapterList> {
        let fetched = self.catalogue(&source)?.chapters(&id).await?;
        let synced = self.store.sync_chapters(&source, &id, &fetched)?;
        Ok(ChapterList { entries: self.entries(&source, &id)?, new_ids: synced.new })
    }

    pub async fn pages(&self, source: String, series: String, chapter: String) -> Result<PageList> {
        Ok(self.catalogue(&source)?.pages(&series, &chapter).await?.into())
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
            .map(|item| LibraryEntry { source: item.source, id: item.id, title: item.title, cover: item.cover, unread: item.unread, total: item.total, last_read_ms: item.last_read_ms })
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

    pub fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>> {
        Ok(self.store.history(limit)?.into_iter().map(|(source, series, chapter, at_ms)| HistoryEntry { source, series, chapter, at_ms }).collect())
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
