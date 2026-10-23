//! What the person keeps: the library, the chapters seen, how far each was read, categories.
//! One SQLite file on the phone. Everything the sources return is a cache of what a site said;
//! progress and the library are the only things here that cannot be fetched again.

use crate::model::{Chapter, Status};
use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_VERSION: i32 = 4;

pub struct Store {
    db: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub unread: u32,
    pub total: u32,
    pub last_read_ms: Option<i64>,
    pub saved_ms: i64,
    pub categories: Vec<i64>,
    /// When its chapters were last asked of the site.
    pub checked_ms: Option<i64>,
    /// A finished series: it is checked for new chapters rarely.
    pub completed: bool,
}

/// A saved series with its newest chapter, for the Updates screen.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub unread: u32,
    pub latest_number: Option<f64>,
    pub latest_title: Option<String>,
    pub latest_date_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub count: u32,
}

/// A chapter kept on the device.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadRow {
    pub source: String,
    pub series: String,
    pub chapter: String,
    /// `queued`, `active`, `done` or `failed`.
    pub state: String,
    pub pages: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChapterRow {
    pub chapter: Chapter,
    pub finished: bool,
    /// The page the person is on in an unfinished chapter.
    pub page: Option<u32>,
}

/// How far a chapter has been read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub page: u32,
    /// How far into that page, 0 to 1 of its height.
    pub offset: f64,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resume {
    pub chapter: String,
    pub page: u32,
    /// How far into that page, 0 to 1 of its height (a long strip restores to the exact spot).
    pub offset: f64,
}

/// A chapter someone read, with the series it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryItem {
    pub source: String,
    pub series: String,
    /// The series' title (its id when it was never fetched).
    pub title: String,
    pub cover: Option<String>,
    pub chapter: String,
    pub chapter_number: Option<f64>,
    pub chapter_title: Option<String>,
    pub at_ms: i64,
    pub finished: bool,
}

/// A page the person marked, with the chapter's number and title for showing it.
#[derive(Debug, Clone, PartialEq)]
pub struct BookmarkRow {
    pub chapter: String,
    pub page: u32,
    /// How far down the page, 0 to 1 of its height (a long-strip page can be many screens tall).
    pub offset: f64,
    pub created_at: i64,
    pub chapter_number: Option<f64>,
    pub chapter_title: Option<String>,
}

/// What a backup holds of the library.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Export {
    pub series: Vec<ExportSeries>,
    pub progress: Vec<ExportProgress>,
    /// Absent from backups made before bookmarks existed.
    #[serde(default)]
    pub bookmarks: Vec<ExportBookmark>,
    /// Category names in order.
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExportSeries {
    pub source: String,
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub description: Option<String>,
    pub status: String,
    /// When it was saved to the library; none for a series only read.
    pub saved_at: Option<i64>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExportProgress {
    pub source: String,
    pub series: String,
    pub chapter: String,
    pub page: u32,
    pub offset: f64,
    pub finished: bool,
    pub updated_at: i64,
    /// Kept so a restore can say which chapter it was.
    pub chapter_number: Option<f64>,
    pub chapter_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExportBookmark {
    pub source: String,
    pub series: String,
    pub chapter: String,
    pub page: u32,
    pub offset: f64,
    pub created_at: i64,
}

/// What `sync_chapters` found.
#[derive(Debug, Clone, PartialEq)]
pub struct Synced {
    /// Chapters the store had not seen. Empty on the first sync of a series: all of them are new then,
    /// and none is news.
    pub new: Vec<String>,
    pub first: bool,
}

fn migrate(db: &Connection) -> anyhow::Result<()> {
    let version: i32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    anyhow::ensure!(
        version <= SCHEMA_VERSION,
        "this library was made by a newer app (schema {version})"
    );
    if version < 1 {
        db.execute_batch(
            "CREATE TABLE series (
                source TEXT NOT NULL, id TEXT NOT NULL, title TEXT NOT NULL, cover TEXT, description TEXT,
                status TEXT NOT NULL DEFAULT 'unknown',
                saved_at INTEGER,
                PRIMARY KEY (source, id));
             CREATE TABLE chapter (
                source TEXT NOT NULL, series TEXT NOT NULL, id TEXT NOT NULL,
                number REAL, title TEXT, date_ms INTEGER, scanlator TEXT,
                position INTEGER NOT NULL,
                PRIMARY KEY (source, series, id));
             CREATE INDEX chapter_by_series ON chapter (source, series, position);
             CREATE TABLE progress (
                source TEXT NOT NULL, series TEXT NOT NULL, chapter TEXT NOT NULL,
                page INTEGER NOT NULL DEFAULT 0, offset REAL NOT NULL DEFAULT 0,
                finished INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL,
                PRIMARY KEY (source, series, chapter));
             CREATE INDEX progress_by_time ON progress (updated_at);
             CREATE TABLE category (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL);
             CREATE TABLE series_category (
                source TEXT NOT NULL, series TEXT NOT NULL, category INTEGER NOT NULL REFERENCES category(id) ON DELETE CASCADE,
                PRIMARY KEY (source, series, category));
             CREATE TABLE download (
                source TEXT NOT NULL, series TEXT NOT NULL, chapter TEXT NOT NULL,
                state TEXT NOT NULL, pages INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (source, series, chapter));
             CREATE TABLE link (
                source TEXT NOT NULL, series TEXT NOT NULL, other_source TEXT NOT NULL, other_series TEXT NOT NULL,
                PRIMARY KEY (source, series, other_source, other_series));
             PRAGMA user_version = 1;",
        )?;
    }
    if version < 2 {
        db.execute_batch(
            "CREATE TABLE repo (
                base TEXT PRIMARY KEY, name TEXT NOT NULL, key TEXT, signed INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL);
             CREATE TABLE installed (source TEXT PRIMARY KEY, repo TEXT, version INTEGER NOT NULL);
             CREATE TABLE setting (
                source TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
                PRIMARY KEY (source, key));
             PRAGMA user_version = 2;",
        )?;
    }
    if version < 3 {
        db.execute_batch(
            "ALTER TABLE series ADD COLUMN checked_at INTEGER;
             CREATE TABLE listing (source TEXT NOT NULL, kind TEXT NOT NULL, body TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY (source, kind));
             CREATE INDEX series_saved ON series (saved_at) WHERE saved_at IS NOT NULL;
             PRAGMA user_version = 3;",
        )?;
    }
    if version < 4 {
        db.execute_batch(
            "CREATE TABLE bookmark (
                source TEXT NOT NULL, series TEXT NOT NULL, chapter TEXT NOT NULL,
                page INTEGER NOT NULL, offset REAL NOT NULL DEFAULT 0, created_at INTEGER NOT NULL,
                PRIMARY KEY (source, series, chapter, page));
             PRAGMA user_version = 4;",
        )?;
    }
    Ok(())
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Store> {
        let db =
            Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        Store::from(db)
    }

    pub fn in_memory() -> anyhow::Result<Store> {
        Store::from(Connection::open_in_memory()?)
    }

    fn from(db: Connection) -> anyhow::Result<Store> {
        db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        migrate(&db)?;
        Ok(Store { db: Mutex::new(db) })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap()
    }

    /// Remembers what a site said about a series. Keeps the library flag.
    pub fn upsert_series(
        &self,
        source: &str,
        id: &str,
        title: &str,
        cover: Option<&str>,
        description: Option<&str>,
        status: Status,
    ) -> anyhow::Result<()> {
        let status = serde_json::to_value(status)?
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        self.db().execute(
            "INSERT INTO series (source, id, title, cover, description, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source, id) DO UPDATE SET title = ?3, cover = coalesce(?4, cover), description = coalesce(?5, description), status = ?6",
            params![source, id, title, cover, description, status],
        )?;
        Ok(())
    }

    pub fn add_to_library(&self, source: &str, id: &str, now_ms: i64) -> anyhow::Result<()> {
        let changed = self.db().execute(
            "UPDATE series SET saved_at = coalesce(saved_at, ?3) WHERE source = ?1 AND id = ?2",
            params![source, id, now_ms],
        )?;
        anyhow::ensure!(
            changed == 1,
            "that series is not known yet: save its details first"
        );
        Ok(())
    }

    pub fn remove_from_library(&self, source: &str, id: &str) -> anyhow::Result<()> {
        self.db().execute(
            "UPDATE series SET saved_at = NULL WHERE source = ?1 AND id = ?2",
            params![source, id],
        )?;
        Ok(())
    }

    pub fn in_library(&self, source: &str, id: &str) -> anyhow::Result<bool> {
        Ok(self
            .db()
            .query_row(
                "SELECT saved_at IS NOT NULL FROM series WHERE source = ?1 AND id = ?2",
                params![source, id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    /// Replaces the chapter list with what the site says now (newest first) and tells which are new.
    pub fn sync_chapters(
        &self,
        source: &str,
        series: &str,
        chapters: &[Chapter],
    ) -> anyhow::Result<Synced> {
        let mut db = self.db();
        let tx = db.transaction()?;
        let known: Vec<String> = {
            let mut statement =
                tx.prepare("SELECT id FROM chapter WHERE source = ?1 AND series = ?2")?;
            statement
                .query_map(params![source, series], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        let first = known.is_empty();
        let mut new = Vec::new();
        for (position, chapter) in chapters.iter().enumerate() {
            tx.execute(
                "INSERT INTO chapter (source, series, id, number, title, date_ms, scanlator, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (source, series, id) DO UPDATE SET number = ?4, title = ?5, date_ms = ?6, scanlator = ?7, position = ?8",
                params![source, series, chapter.id, chapter.number, chapter.title, chapter.date_ms, chapter.scanlator, position as i64],
            )?;
            if !known.contains(&chapter.id) && !first {
                new.push(chapter.id.clone());
            }
        }
        // A chapter the site no longer lists is gone, unless it was read: progress is kept either way.
        let ids: Vec<&str> = chapters.iter().map(|chapter| chapter.id.as_str()).collect();
        for gone in known.iter().filter(|id| !ids.contains(&id.as_str())) {
            tx.execute(
                "DELETE FROM chapter WHERE source = ?1 AND series = ?2 AND id = ?3",
                params![source, series, gone],
            )?;
        }
        tx.commit()?;
        Ok(Synced { new, first })
    }

    pub fn chapters(&self, source: &str, series: &str) -> anyhow::Result<Vec<ChapterRow>> {
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT c.id, c.number, c.title, c.date_ms, c.scanlator, coalesce(p.finished, 0), p.page
             FROM chapter c LEFT JOIN progress p ON p.source = c.source AND p.series = c.series AND p.chapter = c.id
             WHERE c.source = ?1 AND c.series = ?2 ORDER BY c.position",
        )?;
        let rows = statement.query_map(params![source, series], |row| {
            let finished: bool = row.get(5)?;
            let page: Option<u32> = row.get(6)?;
            Ok(ChapterRow {
                chapter: Chapter {
                    id: row.get(0)?,
                    number: row.get(1)?,
                    title: row.get(2)?,
                    date_ms: row.get(3)?,
                    scanlator: row.get(4)?,
                },
                finished,
                page: if finished {
                    None
                } else {
                    page.filter(|page| *page > 0)
                },
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_progress(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        progress: Progress,
        now_ms: i64,
    ) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO progress (source, series, chapter, page, offset, finished, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (source, series, chapter) DO UPDATE SET page = ?4, offset = ?5, finished = ?6, updated_at = ?7",
            params![source, series, chapter, progress.page, progress.offset, progress.finished, now_ms],
        )?;
        Ok(())
    }

    /// Marks chapters read (or unread, which forgets their progress).
    pub fn mark(
        &self,
        source: &str,
        series: &str,
        chapters: &[&str],
        read: bool,
        now_ms: i64,
    ) -> anyhow::Result<()> {
        let mut db = self.db();
        let tx = db.transaction()?;
        for chapter in chapters {
            if read {
                tx.execute(
                    "INSERT INTO progress (source, series, chapter, page, offset, finished, updated_at) VALUES (?1, ?2, ?3, 0, 0, 1, ?4)
                     ON CONFLICT (source, series, chapter) DO UPDATE SET page = 0, offset = 0, finished = 1, updated_at = ?4",
                    params![source, series, chapter, now_ms],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM progress WHERE source = ?1 AND series = ?2 AND chapter = ?3",
                    params![source, series, chapter],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Where "Continue" goes: the chapter being read, or else the one after the last finished
    /// (chapters are stored newest first, so the next to read is the one before in the list).
    pub fn resume(&self, source: &str, series: &str) -> anyhow::Result<Option<Resume>> {
        let db = self.db();
        let current: Option<Resume> = db
            .query_row(
                "SELECT chapter, page, offset FROM progress WHERE source = ?1 AND series = ?2 AND finished = 0 ORDER BY updated_at DESC LIMIT 1",
                params![source, series],
                |row| Ok(Resume { chapter: row.get(0)?, page: row.get(1)?, offset: row.get(2)? }),
            )
            .optional()?;
        if current.is_some() {
            return Ok(current);
        }
        // The finished chapter that is furthest along (smallest position), and the one before it.
        let next: Option<String> = db
            .query_row(
                "SELECT c.id FROM chapter c
                 WHERE c.source = ?1 AND c.series = ?2
                   AND c.position < (SELECT min(c2.position) FROM chapter c2 JOIN progress p ON p.source = c2.source AND p.series = c2.series AND p.chapter = c2.id
                                     WHERE c2.source = ?1 AND c2.series = ?2 AND p.finished = 1)
                 ORDER BY c.position DESC LIMIT 1",
                params![source, series],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(chapter) = next {
            return Ok(Some(Resume {
                chapter,
                page: 0,
                offset: 0.0,
            }));
        }
        // Nothing read yet: the oldest chapter, the first to read.
        let first: Option<String> = db.query_row("SELECT id FROM chapter WHERE source = ?1 AND series = ?2 ORDER BY position DESC LIMIT 1", params![source, series], |row| row.get(0)).optional()?;
        Ok(first
            .filter(|_| !self.has_progress(&db, source, series))
            .map(|chapter| Resume {
                chapter,
                page: 0,
                offset: 0.0,
            }))
    }

    fn has_progress(&self, db: &Connection, source: &str, series: &str) -> bool {
        db.query_row(
            "SELECT 1 FROM progress WHERE source = ?1 AND series = ?2 LIMIT 1",
            params![source, series],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some()
    }

    /// Counts of chapters and of finished ones for every series, read once (a query per series would grow with the library).
    const COUNTS: &'static str = "
        LEFT JOIN (SELECT source, series, count(*) AS total FROM chapter GROUP BY source, series) c ON c.source = s.source AND c.series = s.id
        LEFT JOIN (SELECT p.source, p.series, count(*) AS done FROM progress p
                   JOIN chapter ch ON ch.source = p.source AND ch.series = p.series AND ch.id = p.chapter
                   WHERE p.finished = 1 GROUP BY p.source, p.series) d ON d.source = s.source AND d.series = s.id";

    pub fn library(&self) -> anyhow::Result<Vec<LibraryItem>> {
        let db = self.db();
        let mut statement = db.prepare(&format!(
            "SELECT s.source, s.id, s.title, s.cover, coalesce(c.total, 0), coalesce(c.total, 0) - coalesce(d.done, 0),
                (SELECT max(p.updated_at) FROM progress p WHERE p.source = s.source AND p.series = s.id),
                s.saved_at, s.checked_at, s.status IN ('completed', 'cancelled')
             FROM series s {} WHERE s.saved_at IS NOT NULL",
            Store::COUNTS
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(LibraryItem {
                source: row.get(0)?,
                id: row.get(1)?,
                title: row.get(2)?,
                cover: row.get(3)?,
                total: row.get(4)?,
                unread: row.get(5)?,
                last_read_ms: row.get(6)?,
                saved_ms: row.get(7)?,
                categories: Vec::new(),
                checked_ms: row.get(8)?,
                completed: row.get(9)?,
            })
        })?;
        let mut items: Vec<LibraryItem> = rows.collect::<Result<_, _>>()?;
        let mut position = std::collections::HashMap::new();
        for (at, item) in items.iter().enumerate() {
            position.insert((item.source.clone(), item.id.clone()), at);
        }
        let mut member = db.prepare("SELECT source, series, category FROM series_category")?;
        for row in member.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })? {
            let (source, series, category) = row?;
            if let Some(at) = position.get(&(source, series)) {
                items[*at].categories.push(category);
            }
        }
        // Most recently read first, then by title.
        items.sort_by(|a, b| {
            b.last_read_ms
                .cmp(&a.last_read_ms)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(items)
    }

    /// Every saved series with its newest chapter, newest chapter first: the Updates screen, in one query.
    pub fn updates(&self) -> anyhow::Result<Vec<UpdateItem>> {
        let db = self.db();
        let mut statement = db.prepare(&format!(
            "SELECT s.source, s.id, s.title, s.cover, coalesce(c.total, 0) - coalesce(d.done, 0), l.number, l.title, l.date_ms
             FROM series s {} LEFT JOIN chapter l ON l.source = s.source AND l.series = s.id AND l.position = 0
             WHERE s.saved_at IS NOT NULL ORDER BY l.date_ms DESC",
            Store::COUNTS
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(UpdateItem {
                source: row.get(0)?,
                id: row.get(1)?,
                title: row.get(2)?,
                cover: row.get(3)?,
                unread: row.get(4)?,
                latest_number: row.get(5)?,
                latest_title: row.get(6)?,
                latest_date_ms: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Remembers that a series' chapters were just asked of the site.
    pub fn set_checked(&self, source: &str, series: &str, now_ms: i64) -> anyhow::Result<()> {
        self.db().execute(
            "UPDATE series SET checked_at = ?3 WHERE source = ?1 AND id = ?2",
            params![source, series, now_ms],
        )?;
        Ok(())
    }

    pub fn checked(&self, source: &str, series: &str) -> anyhow::Result<Option<i64>> {
        Ok(self
            .db()
            .query_row(
                "SELECT checked_at FROM series WHERE source = ?1 AND id = ?2",
                params![source, series],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// The last first page of a listing (`popular`, `latest`) a source gave, to show at once while a fresh one is fetched.
    pub fn listing(&self, source: &str, kind: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .db()
            .query_row(
                "SELECT body FROM listing WHERE source = ?1 AND kind = ?2",
                params![source, kind],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn keep_listing(
        &self,
        source: &str,
        kind: &str,
        body: &str,
        now_ms: i64,
    ) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO listing (source, kind, body, at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (source, kind) DO UPDATE SET body = ?3, at = ?4",
            params![source, kind, body, now_ms],
        )?;
        Ok(())
    }

    /// What was read most recently, newest first, with what to show for it.
    pub fn history(&self, limit: u32) -> anyhow::Result<Vec<HistoryItem>> {
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT p.source, p.series, coalesce(s.title, p.series), s.cover, p.chapter, c.number, c.title, p.updated_at, p.finished
             FROM progress p
             LEFT JOIN series s ON s.source = p.source AND s.id = p.series
             LEFT JOIN chapter c ON c.source = p.source AND c.series = p.series AND c.id = p.chapter
             ORDER BY p.updated_at DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(HistoryItem {
                source: row.get(0)?,
                series: row.get(1)?,
                title: row.get(2)?,
                cover: row.get(3)?,
                chapter: row.get(4)?,
                chapter_number: row.get(5)?,
                chapter_title: row.get(6)?,
                at_ms: row.get(7)?,
                finished: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // --- repositories and what came from them ---------------------------------------------------

    /// Remembers a repository. The first key seen for it is kept, and a later index naming a different key
    /// is refused: a signed repository cannot change hands unnoticed.
    pub fn add_repo(
        &self,
        base: &str,
        name: &str,
        key: Option<&str>,
        signed: bool,
        now_ms: i64,
    ) -> anyhow::Result<()> {
        let db = self.db();
        let pinned: Option<Option<String>> = db
            .query_row(
                "SELECT key FROM repo WHERE base = ?1",
                params![base],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(Some(pinned)) = &pinned {
            anyhow::ensure!(
                key == Some(pinned.as_str()),
                "this repository was added with a different key: it may have changed hands, so it is refused (remove it first to trust the new one)"
            );
        }
        db.execute(
            "INSERT INTO repo (base, name, key, signed, added_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (base) DO UPDATE SET name = ?2, key = coalesce(key, ?3), signed = ?4",
            params![base, name, key, signed, now_ms],
        )?;
        Ok(())
    }

    /// (base, name, signed), oldest first.
    pub fn repos(&self) -> anyhow::Result<Vec<(String, String, bool)>> {
        let db = self.db();
        let mut statement =
            db.prepare("SELECT base, name, signed FROM repo ORDER BY added_at, base")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_repo(&self, base: &str) -> anyhow::Result<()> {
        let db = self.db();
        db.execute("DELETE FROM repo WHERE base = ?1", params![base])?;
        db.execute(
            "UPDATE installed SET repo = NULL WHERE repo = ?1",
            params![base],
        )?;
        Ok(())
    }

    /// Where an installed source came from and which version, for updates.
    pub fn set_origin(&self, source: &str, repo: Option<&str>, version: u64) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO installed (source, repo, version) VALUES (?1, ?2, ?3)
             ON CONFLICT (source) DO UPDATE SET repo = ?2, version = ?3",
            params![source, repo, version as i64],
        )?;
        Ok(())
    }

    pub fn origin(&self, source: &str) -> anyhow::Result<Option<(Option<String>, u64)>> {
        Ok(self
            .db()
            .query_row(
                "SELECT repo, version FROM installed WHERE source = ?1",
                params![source],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64)),
            )
            .optional()?)
    }

    /// Every installed source with where it came from, for a backup.
    pub fn origins(&self) -> anyhow::Result<Vec<(String, Option<String>)>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT source, repo FROM installed ORDER BY source")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn forget_source(&self, source: &str) -> anyhow::Result<()> {
        let db = self.db();
        db.execute("DELETE FROM installed WHERE source = ?1", params![source])?;
        db.execute("DELETE FROM setting WHERE source = ?1", params![source])?;
        db.execute("DELETE FROM listing WHERE source = ?1", params![source])?;
        Ok(())
    }

    pub fn set_setting(&self, source: &str, key: &str, value: &str) -> anyhow::Result<()> {
        self.db().execute("INSERT INTO setting (source, key, value) VALUES (?1, ?2, ?3) ON CONFLICT (source, key) DO UPDATE SET value = ?3", params![source, key, value])?;
        Ok(())
    }

    pub fn settings(&self, source: &str) -> anyhow::Result<Vec<(String, String)>> {
        let db = self.db();
        let mut statement =
            db.prepare("SELECT key, value FROM setting WHERE source = ?1 ORDER BY key")?;
        let rows = statement.query_map(params![source], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn add_category(&self, name: &str) -> anyhow::Result<i64> {
        let db = self.db();
        db.execute("INSERT INTO category (name, position) VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM category))", params![name])?;
        Ok(db.last_insert_rowid())
    }

    pub fn rename_category(&self, id: i64, name: &str) -> anyhow::Result<()> {
        self.db().execute(
            "UPDATE category SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    /// Deletes a category; the series in it stay in the library.
    pub fn delete_category(&self, id: i64) -> anyhow::Result<()> {
        self.db()
            .execute("DELETE FROM category WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// The categories in order, with how many series each holds.
    pub fn categories(&self) -> anyhow::Result<Vec<Category>> {
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT c.id, c.name, (SELECT count(*) FROM series_category m JOIN series s ON s.source = m.source AND s.id = m.series
                                    WHERE m.category = c.id AND s.saved_at IS NOT NULL)
             FROM category c ORDER BY c.position, c.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn put_in_category(
        &self,
        source: &str,
        series: &str,
        category: i64,
        member: bool,
    ) -> anyhow::Result<()> {
        let db = self.db();
        if member {
            db.execute("INSERT OR IGNORE INTO series_category (source, series, category) VALUES (?1, ?2, ?3)", params![source, series, category])?;
        } else {
            db.execute(
                "DELETE FROM series_category WHERE source = ?1 AND series = ?2 AND category = ?3",
                params![source, series, category],
            )?;
        }
        Ok(())
    }

    // --- backup ---------------------------------------------------------------------------------

    /// Everything the person did that cannot be fetched again: saved series, read state, categories.
    pub fn export(&self) -> anyhow::Result<Export> {
        let db = self.db();
        let mut series = Vec::new();
        let mut statement = db.prepare("SELECT source, id, title, cover, description, status, saved_at FROM series WHERE saved_at IS NOT NULL OR EXISTS (SELECT 1 FROM progress p WHERE p.source = series.source AND p.series = series.id)")?;
        for row in statement.query_map([], |row| {
            Ok(ExportSeries {
                source: row.get(0)?,
                id: row.get(1)?,
                title: row.get(2)?,
                cover: row.get(3)?,
                description: row.get(4)?,
                status: row.get(5)?,
                saved_at: row.get(6)?,
                categories: Vec::new(),
            })
        })? {
            series.push(row?);
        }
        let mut statement = db.prepare("SELECT m.source, m.series, c.name FROM series_category m JOIN category c ON c.id = m.category")?;
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (source, id, name) = row?;
            if let Some(found) = series
                .iter_mut()
                .find(|found| found.source == source && found.id == id)
            {
                found.categories.push(name);
            }
        }
        let mut progress = Vec::new();
        let mut statement = db.prepare(
            "SELECT p.source, p.series, p.chapter, p.page, p.offset, p.finished, p.updated_at, c.number, c.title
             FROM progress p LEFT JOIN chapter c ON c.source = p.source AND c.series = p.series AND c.id = p.chapter",
        )?;
        for row in statement.query_map([], |row| {
            Ok(ExportProgress {
                source: row.get(0)?,
                series: row.get(1)?,
                chapter: row.get(2)?,
                page: row.get(3)?,
                offset: row.get(4)?,
                finished: row.get(5)?,
                updated_at: row.get(6)?,
                chapter_number: row.get(7)?,
                chapter_title: row.get(8)?,
            })
        })? {
            progress.push(row?);
        }
        let mut bookmarks = Vec::new();
        let mut statement =
            db.prepare("SELECT source, series, chapter, page, offset, created_at FROM bookmark")?;
        for row in statement.query_map([], |row| {
            Ok(ExportBookmark {
                source: row.get(0)?,
                series: row.get(1)?,
                chapter: row.get(2)?,
                page: row.get(3)?,
                offset: row.get(4)?,
                created_at: row.get(5)?,
            })
        })? {
            bookmarks.push(row?);
        }
        let mut statement = db.prepare("SELECT name FROM category ORDER BY position, id")?;
        let categories = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(Export {
            series,
            progress,
            bookmarks,
            categories,
        })
    }

    /// Puts an export back. A series already here keeps its newer read state; categories are matched by name.
    pub fn import(&self, export: &Export) -> anyhow::Result<()> {
        let sound = |text: &str| !text.is_empty() && text.len() <= 512;
        anyhow::ensure!(
            export
                .series
                .iter()
                .all(|series| sound(&series.source) && sound(&series.id))
                && export
                    .progress
                    .iter()
                    .all(|row| sound(&row.source) && sound(&row.series) && sound(&row.chapter))
                && export
                    .bookmarks
                    .iter()
                    .all(|row| sound(&row.source) && sound(&row.series) && sound(&row.chapter)),
            "the backup holds a name that cannot be right"
        );
        let mut db = self.db();
        let tx = db.transaction()?;
        for name in &export.categories {
            tx.execute("INSERT OR IGNORE INTO category (name, position) VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM category))", params![name])?;
        }
        for series in &export.series {
            tx.execute(
                "INSERT INTO series (source, id, title, cover, description, status, saved_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (source, id) DO UPDATE SET title = ?3, cover = coalesce(?4, cover), description = coalesce(?5, description),
                                                        saved_at = coalesce(saved_at, ?7)",
                params![series.source, series.id, series.title, series.cover, series.description, series.status, series.saved_at],
            )?;
            for name in &series.categories {
                tx.execute(
                    "INSERT OR IGNORE INTO series_category (source, series, category) SELECT ?1, ?2, id FROM category WHERE name = ?3",
                    params![series.source, series.id, name],
                )?;
            }
        }
        for row in &export.progress {
            // The chapter row may not be here yet (it is fetched again later); progress is keyed by the chapter's id.
            tx.execute(
                "INSERT INTO progress (source, series, chapter, page, offset, finished, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (source, series, chapter) DO UPDATE SET page = excluded.page, offset = excluded.offset, finished = excluded.finished, updated_at = excluded.updated_at
                 WHERE excluded.updated_at > progress.updated_at",
                params![row.source, row.series, row.chapter, row.page, row.offset, row.finished, row.updated_at],
            )?;
        }
        for row in &export.bookmarks {
            // A bookmark already here is kept as it is.
            tx.execute(
                "INSERT OR IGNORE INTO bookmark (source, series, chapter, page, offset, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![row.source, row.series, row.chapter, row.page, row.offset, row.created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- bookmarks -----------------------------------------------------------------------------

    /// Marks a page. Marking it again moves the mark inside the page and keeps when it was first made.
    pub fn add_bookmark(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        page: u32,
        offset: f64,
        now_ms: i64,
    ) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO bookmark (source, series, chapter, page, offset, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source, series, chapter, page) DO UPDATE SET offset = ?5",
            params![source, series, chapter, page, offset, now_ms],
        )?;
        Ok(())
    }

    pub fn remove_bookmark(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        page: u32,
    ) -> anyhow::Result<()> {
        self.db().execute(
            "DELETE FROM bookmark WHERE source = ?1 AND series = ?2 AND chapter = ?3 AND page = ?4",
            params![source, series, chapter, page],
        )?;
        Ok(())
    }

    /// The pages marked in a series, newest mark first.
    pub fn bookmarks(&self, source: &str, series: &str) -> anyhow::Result<Vec<BookmarkRow>> {
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT b.chapter, b.page, b.offset, b.created_at, c.number, c.title
             FROM bookmark b LEFT JOIN chapter c ON c.source = b.source AND c.series = b.series AND c.id = b.chapter
             WHERE b.source = ?1 AND b.series = ?2 ORDER BY b.created_at DESC, b.chapter, b.page",
        )?;
        let rows = statement.query_map(params![source, series], |row| {
            Ok(BookmarkRow {
                chapter: row.get(0)?,
                page: row.get(1)?,
                offset: row.get(2)?,
                created_at: row.get(3)?,
                chapter_number: row.get(4)?,
                chapter_title: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // --- downloads -----------------------------------------------------------------------------

    pub fn set_download(&self, row: &DownloadRow) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO download (source, series, chapter, state, pages, bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source, series, chapter) DO UPDATE SET state = ?4, pages = ?5, bytes = ?6",
            params![row.source, row.series, row.chapter, row.state, row.pages, row.bytes as i64],
        )?;
        Ok(())
    }

    /// Whether a chapter is kept in full (a half-finished one is not).
    pub fn is_downloaded(&self, source: &str, series: &str, chapter: &str) -> anyhow::Result<bool> {
        Ok(self.db().query_row("SELECT EXISTS (SELECT 1 FROM download WHERE source = ?1 AND series = ?2 AND chapter = ?3 AND state = 'done')", params![source, series, chapter], |row| row.get(0))?)
    }

    /// After a launch: a download that was running when the app stopped did not finish.
    pub fn fail_interrupted_downloads(&self) -> anyhow::Result<()> {
        self.db().execute(
            "UPDATE download SET state = 'failed' WHERE state IN ('queued', 'active')",
            [],
        )?;
        Ok(())
    }

    pub fn downloads(&self) -> anyhow::Result<Vec<DownloadRow>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT source, series, chapter, state, pages, bytes FROM download ORDER BY source, series, chapter")?;
        let rows = statement.query_map([], |row| {
            Ok(DownloadRow {
                source: row.get(0)?,
                series: row.get(1)?,
                chapter: row.get(2)?,
                state: row.get(3)?,
                pages: row.get(4)?,
                bytes: row.get::<_, i64>(5)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_download(&self, source: &str, series: &str, chapter: &str) -> anyhow::Result<()> {
        self.db().execute(
            "DELETE FROM download WHERE source = ?1 AND series = ?2 AND chapter = ?3",
            params![source, series, chapter],
        )?;
        Ok(())
    }

    // --- moving and removing series -------------------------------------------------------------

    /// Moves what the person did with a series to the same series on another source: the library entry, its
    /// categories, and the read state of every chapter whose number the other source has too. Returns how
    /// many chapters carried over. Reading history stays under the old series.
    pub fn migrate(
        &self,
        from_source: &str,
        from_series: &str,
        to_source: &str,
        to_series: &str,
        now_ms: i64,
    ) -> anyhow::Result<u32> {
        anyhow::ensure!(
            (from_source, from_series) != (to_source, to_series),
            "a series cannot move onto itself"
        );
        let mut db = self.db();
        let tx = db.transaction()?;
        let target_known: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM series WHERE source = ?1 AND id = ?2)",
            params![to_source, to_series],
            |row| row.get(0),
        )?;
        anyhow::ensure!(
            target_known,
            "open the other series once before moving to it"
        );
        let old: Vec<(String, f64)> = {
            let mut statement = tx.prepare("SELECT id, number FROM chapter WHERE source = ?1 AND series = ?2 AND number IS NOT NULL ORDER BY id")?;
            statement
                .query_map(params![from_source, from_series], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<Result<_, _>>()?
        };
        let new: Vec<(String, f64)> = {
            let mut statement = tx.prepare("SELECT id, number FROM chapter WHERE source = ?1 AND series = ?2 AND number IS NOT NULL")?;
            statement
                .query_map(params![to_source, to_series], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<Result<_, _>>()?
        };
        let mut carried = 0;
        for (old_id, number) in &old {
            let Some((new_id, _)) = new.iter().find(|(_, other)| (other - number).abs() < 1e-6)
            else {
                continue;
            };
            let moved = tx.execute(
                "INSERT INTO progress (source, series, chapter, page, offset, finished, updated_at)
                 SELECT ?3, ?4, ?5, page, offset, finished, ?6 FROM progress WHERE source = ?1 AND series = ?2 AND chapter = ?7
                 ON CONFLICT (source, series, chapter) DO UPDATE SET page = excluded.page, offset = excluded.offset, finished = max(finished, excluded.finished), updated_at = excluded.updated_at",
                params![from_source, from_series, to_source, to_series, new_id, now_ms, old_id],
            )?;
            carried += moved as u32;
            tx.execute(
                "INSERT OR IGNORE INTO bookmark (source, series, chapter, page, offset, created_at)
                 SELECT ?3, ?4, ?5, page, offset, created_at FROM bookmark WHERE source = ?1 AND series = ?2 AND chapter = ?6",
                params![from_source, from_series, to_source, to_series, new_id, old_id],
            )?;
        }
        tx.execute("UPDATE series SET saved_at = coalesce(saved_at, (SELECT saved_at FROM series WHERE source = ?1 AND id = ?2), ?5) WHERE source = ?3 AND id = ?4", params![from_source, from_series, to_source, to_series, now_ms])?;
        tx.execute(
            "INSERT OR IGNORE INTO series_category (source, series, category) SELECT ?3, ?4, category FROM series_category WHERE source = ?1 AND series = ?2",
            params![from_source, from_series, to_source, to_series],
        )?;
        tx.execute(
            "UPDATE series SET saved_at = NULL WHERE source = ?1 AND id = ?2",
            params![from_source, from_series],
        )?;
        tx.commit()?;
        Ok(carried)
    }

    /// Forgets a series: its entry, chapters, progress, bookmarks, categories and downloads.
    pub fn delete_series(&self, source: &str, series: &str) -> anyhow::Result<()> {
        let mut db = self.db();
        let tx = db.transaction()?;
        for table in [
            "series_category",
            "progress",
            "bookmark",
            "download",
            "chapter",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE source = ?1 AND series = ?2"),
                params![source, series],
            )?;
        }
        tx.execute(
            "DELETE FROM series WHERE source = ?1 AND id = ?2",
            params![source, series],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: &str, number: f64) -> Chapter {
        Chapter {
            id: id.into(),
            number: Some(number),
            title: None,
            date_ms: None,
            scanlator: None,
        }
    }

    fn store_with_series() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .upsert_series("s", "a", "Alpha", Some("c.jpg"), None, Status::Ongoing)
            .unwrap();
        store.add_to_library("s", "a", 100).unwrap();
        store
    }

    #[test]
    fn the_first_sync_has_no_news_and_a_later_one_names_the_new_chapters() {
        let store = store_with_series();
        let first = store
            .sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)])
            .unwrap();
        assert_eq!(
            first,
            Synced {
                new: vec![],
                first: true
            }
        );
        let later = store
            .sync_chapters(
                "s",
                "a",
                &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)],
            )
            .unwrap();
        assert_eq!(
            later,
            Synced {
                new: vec!["c3".into()],
                first: false
            }
        );
        let rows = store.chapters("s", "a").unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row.chapter.id.as_str())
                .collect::<Vec<_>>(),
            ["c3", "c2", "c1"]
        );
    }

    #[test]
    fn a_page_is_bookmarked_once_and_the_list_is_newest_first() {
        let store = store_with_series();
        store
            .sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)])
            .unwrap();
        store.add_bookmark("s", "a", "c1", 3, 0.25, 100).unwrap();
        store.add_bookmark("s", "a", "c2", 0, 0.0, 200).unwrap();
        // Bookmarking the same page again moves the mark inside it and keeps when it was first made.
        store.add_bookmark("s", "a", "c1", 3, 0.5, 300).unwrap();
        let marks = store.bookmarks("s", "a").unwrap();
        assert_eq!(
            marks
                .iter()
                .map(|mark| (
                    mark.chapter.as_str(),
                    mark.page,
                    mark.offset,
                    mark.created_at
                ))
                .collect::<Vec<_>>(),
            [("c2", 0, 0.0, 200), ("c1", 3, 0.5, 100)]
        );
        assert_eq!(marks[1].chapter_number, Some(1.0));
        store.remove_bookmark("s", "a", "c2", 0).unwrap();
        assert_eq!(store.bookmarks("s", "a").unwrap().len(), 1);
        assert!(store.bookmarks("s", "other").unwrap().is_empty());
    }

    #[test]
    fn bookmarks_go_into_a_backup_follow_a_move_and_leave_with_the_series() {
        let store = store_with_series();
        store
            .sync_chapters("s", "a", &[chapter("c1", 1.0)])
            .unwrap();
        store.add_bookmark("s", "a", "c1", 2, 0.1, 100).unwrap();

        // A backup carries them, and an older backup without the key still reads.
        let backup = store.export().unwrap();
        let json = serde_json::to_value(&backup).unwrap();
        let mut older = json.clone();
        older.as_object_mut().unwrap().remove("bookmarks");
        assert!(
            serde_json::from_value::<Export>(older)
                .unwrap()
                .bookmarks
                .is_empty()
        );
        let restored = Store::in_memory().unwrap();
        restored
            .import(&serde_json::from_value(json).unwrap())
            .unwrap();
        assert_eq!(restored.bookmarks("s", "a").unwrap().len(), 1);

        // Moving the series to another source carries a bookmark to the chapter with the same number.
        store
            .upsert_series("t", "b", "Alpha", None, None, Status::Ongoing)
            .unwrap();
        store
            .sync_chapters("t", "b", &[chapter("x1", 1.0)])
            .unwrap();
        store.migrate("s", "a", "t", "b", 500).unwrap();
        let moved = store.bookmarks("t", "b").unwrap();
        assert_eq!((moved[0].chapter.as_str(), moved[0].page), ("x1", 2));

        // Forgetting a series forgets its bookmarks.
        store.delete_series("s", "a").unwrap();
        assert!(store.bookmarks("s", "a").unwrap().is_empty());
    }

    #[test]
    fn a_library_from_before_bookmarks_gets_the_table() {
        let store = Store::in_memory().unwrap();
        let db = store.db();
        db.execute_batch("DROP TABLE bookmark; PRAGMA user_version = 3;")
            .unwrap();
        migrate(&db).unwrap();
        db.execute(
            "INSERT INTO bookmark (source, series, chapter, page, offset, created_at) VALUES ('s', 'a', 'c', 0, 0, 1)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn unread_counts_follow_progress_and_continue_goes_to_the_right_chapter() {
        let store = store_with_series();
        store
            .sync_chapters(
                "s",
                "a",
                &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)],
            )
            .unwrap();
        assert_eq!(store.library().unwrap()[0].unread, 3);
        // Nothing read: start at the oldest chapter.
        assert_eq!(store.resume("s", "a").unwrap().unwrap().chapter, "c1");
        store
            .set_progress(
                "s",
                "a",
                "c1",
                Progress {
                    page: 0,
                    offset: 0.0,
                    finished: true,
                },
                200,
            )
            .unwrap();
        // Finished chapter 1: continue with 2.
        assert_eq!(store.resume("s", "a").unwrap().unwrap().chapter, "c2");
        store
            .set_progress(
                "s",
                "a",
                "c2",
                Progress {
                    page: 7,
                    offset: 0.4,
                    finished: false,
                },
                300,
            )
            .unwrap();
        let resume = store.resume("s", "a").unwrap().unwrap();
        assert_eq!(
            (resume.chapter.as_str(), resume.page, resume.offset),
            ("c2", 7, 0.4)
        );
        let item = &store.library().unwrap()[0];
        assert_eq!(
            (item.unread, item.total, item.last_read_ms),
            (2, 3, Some(300))
        );
        let rows = store.chapters("s", "a").unwrap();
        assert_eq!(rows[1].page, Some(7));
        assert!(rows[2].finished);
    }

    #[test]
    fn everything_read_leaves_nothing_to_continue() {
        let store = store_with_series();
        store
            .sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)])
            .unwrap();
        store.mark("s", "a", &["c1", "c2"], true, 1).unwrap();
        assert_eq!(store.resume("s", "a").unwrap(), None);
        assert_eq!(store.library().unwrap()[0].unread, 0);
        store.mark("s", "a", &["c2"], false, 2).unwrap();
        assert_eq!(store.library().unwrap()[0].unread, 1);
    }

    #[test]
    fn progress_survives_a_chapter_leaving_the_list_and_library_membership_is_separate() {
        let store = store_with_series();
        store
            .sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)])
            .unwrap();
        store
            .set_progress(
                "s",
                "a",
                "c1",
                Progress {
                    page: 3,
                    offset: 0.0,
                    finished: false,
                },
                5,
            )
            .unwrap();
        store
            .sync_chapters("s", "a", &[chapter("c2", 2.0)])
            .unwrap();
        let history = store.history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(
            (
                history[0].title.as_str(),
                history[0].cover.as_deref(),
                history[0].chapter.as_str()
            ),
            ("Alpha", Some("c.jpg"), "c1")
        );
        store.remove_from_library("s", "a").unwrap();
        assert!(!store.in_library("s", "a").unwrap());
        assert!(store.library().unwrap().is_empty());
        // Saving a series again does not forget when it was first saved.
        store
            .upsert_series("s", "a", "Alpha 2", None, None, Status::Completed)
            .unwrap();
        assert!(store.add_to_library("s", "zzz", 1).is_err());
    }

    #[test]
    fn a_repository_keeps_the_first_key_it_was_added_with() {
        let store = Store::in_memory().unwrap();
        store
            .add_repo("https://a/", "A", Some("key1"), true, 1)
            .unwrap();
        store
            .add_repo("https://a/", "A renamed", Some("key1"), true, 2)
            .unwrap();
        assert!(
            store
                .add_repo("https://a/", "A", Some("key2"), true, 3)
                .is_err()
        );
        assert!(store.add_repo("https://a/", "A", None, false, 3).is_err());
        assert_eq!(
            store.repos().unwrap(),
            vec![("https://a/".to_string(), "A renamed".to_string(), true)]
        );
        // An unsigned repository can later be signed; and removing a repository forgets its key.
        store.add_repo("https://b/", "B", None, false, 4).unwrap();
        store
            .add_repo("https://b/", "B", Some("kb"), true, 5)
            .unwrap();
        store.remove_repo("https://a/").unwrap();
        store
            .add_repo("https://a/", "A", Some("key2"), true, 6)
            .unwrap();
    }

    #[test]
    fn what_was_installed_and_the_settings_chosen_are_remembered_per_source() {
        let store = Store::in_memory().unwrap();
        store.add_repo("https://a/", "A", None, false, 1).unwrap();
        store.set_origin("x", Some("https://a/"), 3).unwrap();
        store.set_origin("x", Some("https://a/"), 4).unwrap();
        assert_eq!(
            store.origin("x").unwrap(),
            Some((Some("https://a/".into()), 4))
        );
        store.set_setting("x", "lang", "fr").unwrap();
        store.set_setting("x", "lang", "es").unwrap();
        assert_eq!(
            store.settings("x").unwrap(),
            vec![("lang".to_string(), "es".to_string())]
        );
        store.remove_repo("https://a/").unwrap();
        assert_eq!(store.origin("x").unwrap(), Some((None, 4)));
        store.forget_source("x").unwrap();
        assert_eq!(store.origin("x").unwrap(), None);
        assert!(store.settings("x").unwrap().is_empty());
    }

    #[test]
    fn categories_count_their_saved_series_and_show_in_the_library() {
        let store = store_with_series();
        let reading = store.add_category("Reading").unwrap();
        let later = store.add_category("Later").unwrap();
        store.put_in_category("s", "a", reading, true).unwrap();
        let categories = store.categories().unwrap();
        assert_eq!(
            categories
                .iter()
                .map(|c| (c.name.as_str(), c.count))
                .collect::<Vec<_>>(),
            [("Reading", 1), ("Later", 0)]
        );
        assert_eq!(store.library().unwrap()[0].categories, vec![reading]);
        assert_eq!(store.library().unwrap()[0].saved_ms, 100);
        store.rename_category(later, "Someday").unwrap();
        store.delete_category(reading).unwrap();
        assert_eq!(store.categories().unwrap()[0].name, "Someday");
        assert!(
            store.library().unwrap()[0].categories.is_empty(),
            "the series stays in the library"
        );
    }

    #[test]
    fn migrating_carries_the_library_entry_categories_and_read_chapters_by_number() {
        let store = store_with_series();
        let reading = store.add_category("Reading").unwrap();
        store.put_in_category("s", "a", reading, true).unwrap();
        store
            .sync_chapters(
                "s",
                "a",
                &[
                    chapter("old3", 3.0),
                    chapter("old2", 2.0),
                    chapter("old1", 1.0),
                ],
            )
            .unwrap();
        store.mark("s", "a", &["old1", "old2"], true, 5).unwrap();
        store
            .set_progress(
                "s",
                "a",
                "old3",
                Progress {
                    page: 4,
                    offset: 0.25,
                    finished: false,
                },
                6,
            )
            .unwrap();
        store
            .upsert_series(
                "t",
                "b",
                "Alpha (other source)",
                None,
                None,
                Status::Ongoing,
            )
            .unwrap();
        // The other source has chapters 1, 2.5 and 3, under other ids.
        store
            .sync_chapters(
                "t",
                "b",
                &[chapter("n3", 3.0), chapter("n25", 2.5), chapter("n1", 1.0)],
            )
            .unwrap();
        assert_eq!(store.migrate("s", "a", "t", "b", 50).unwrap(), 2);

        let rows = store.chapters("t", "b").unwrap();
        let state = |id: &str| {
            rows.iter()
                .find(|row| row.chapter.id == id)
                .map(|row| (row.finished, row.page))
                .unwrap()
        };
        assert_eq!(state("n1"), (true, None));
        assert_eq!(state("n25"), (false, None), "2.5 has no counterpart");
        assert_eq!(state("n3"), (false, Some(4)));
        assert!(!store.in_library("s", "a").unwrap());
        let library = store.library().unwrap();
        assert_eq!(
            (
                library.len(),
                library[0].id.as_str(),
                library[0].saved_ms,
                library[0].categories.clone()
            ),
            (1, "b", 100, vec![reading])
        );
    }

    #[test]
    fn an_export_restores_into_a_fresh_store_and_never_overwrites_newer_progress() {
        let store = store_with_series();
        let reading = store.add_category("Reading").unwrap();
        store.put_in_category("s", "a", reading, true).unwrap();
        store
            .sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)])
            .unwrap();
        store.mark("s", "a", &["c1"], true, 10).unwrap();
        store
            .set_progress(
                "s",
                "a",
                "c2",
                Progress {
                    page: 3,
                    offset: 0.5,
                    finished: false,
                },
                20,
            )
            .unwrap();
        let export = store.export().unwrap();
        assert_eq!(
            (
                export.series.len(),
                export.progress.len(),
                export.categories.clone()
            ),
            (1, 2, vec!["Reading".to_string()])
        );
        let text = serde_json::to_string(&export).unwrap();

        let fresh = Store::in_memory().unwrap();
        fresh.import(&serde_json::from_str(&text).unwrap()).unwrap();
        let library = fresh.library().unwrap();
        assert_eq!(
            (
                library[0].title.as_str(),
                library[0].saved_ms,
                library[0].categories.len()
            ),
            ("Alpha", 100, 1)
        );
        assert_eq!(fresh.resume("s", "a").unwrap().unwrap().chapter, "c2");
        assert_eq!(fresh.history(10).unwrap().len(), 2);

        // Newer progress already on the device wins over an older backup.
        fresh
            .set_progress(
                "s",
                "a",
                "c2",
                Progress {
                    page: 9,
                    offset: 0.0,
                    finished: false,
                },
                99,
            )
            .unwrap();
        fresh.import(&serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(fresh.resume("s", "a").unwrap().unwrap().page, 9);
    }

    #[test]
    fn updates_show_each_saved_series_with_its_newest_chapter_and_unread_count_in_one_read() {
        let store = store_with_series();
        store
            .sync_chapters(
                "s",
                "a",
                &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)],
            )
            .unwrap();
        store.mark("s", "a", &["c1"], true, 5).unwrap();
        let updates = store.updates().unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(
            (updates[0].unread, updates[0].latest_number),
            (2, Some(3.0))
        );
        let item = &store.library().unwrap()[0];
        assert_eq!(
            (item.total, item.unread, item.checked_ms, item.completed),
            (3, 2, None, false)
        );
        store.set_checked("s", "a", 777).unwrap();
        assert_eq!(store.checked("s", "a").unwrap(), Some(777));
        assert_eq!(store.library().unwrap()[0].checked_ms, Some(777));
    }

    #[test]
    fn the_last_first_page_of_a_listing_is_kept_per_source_and_kind() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.listing("s", "popular").unwrap(), None);
        store.keep_listing("s", "popular", "one", 1).unwrap();
        store.keep_listing("s", "popular", "two", 2).unwrap();
        store.keep_listing("s", "latest", "late", 3).unwrap();
        assert_eq!(
            store.listing("s", "popular").unwrap().as_deref(),
            Some("two")
        );
        assert_eq!(
            store.listing("s", "latest").unwrap().as_deref(),
            Some("late")
        );
        store.forget_source("s").unwrap();
        assert_eq!(store.listing("s", "popular").unwrap(), None);
    }

    #[test]
    fn deleting_a_series_forgets_everything_about_it() {
        let store = store_with_series();
        store
            .sync_chapters("s", "a", &[chapter("c1", 1.0)])
            .unwrap();
        store.mark("s", "a", &["c1"], true, 1).unwrap();
        store
            .set_download(&DownloadRow {
                source: "s".into(),
                series: "a".into(),
                chapter: "c1".into(),
                state: "done".into(),
                pages: 3,
                bytes: 99,
            })
            .unwrap();
        assert_eq!(store.downloads().unwrap()[0].pages, 3);
        store.delete_series("s", "a").unwrap();
        assert!(store.library().unwrap().is_empty());
        assert!(store.chapters("s", "a").unwrap().is_empty());
        assert!(store.history(10).unwrap().is_empty());
        assert!(store.downloads().unwrap().is_empty());
    }

    #[test]
    fn categories_hold_series_and_a_file_opens_again() {
        let dir = std::env::temp_dir().join(format!("akidev-manga-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manga.db");
        {
            let store = Store::open(&path).unwrap();
            store
                .upsert_series("s", "a", "Alpha", None, None, Status::Unknown)
                .unwrap();
            let reading = store.add_category("Reading").unwrap();
            store.put_in_category("s", "a", reading, true).unwrap();
            store.put_in_category("s", "a", reading, true).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.categories().unwrap()[0].count,
            0,
            "an unsaved series does not count on a shelf"
        );
        assert_eq!(store.categories().unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
