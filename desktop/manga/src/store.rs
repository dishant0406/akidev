//! What the person keeps: the library, the chapters seen, how far each was read, categories.
//! One SQLite file on the phone. Everything the sources return is a cache of what a site said;
//! progress and the library are the only things here that cannot be fetched again.

use crate::model::{Chapter, Status};
use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_VERSION: i32 = 2;

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
    anyhow::ensure!(version <= SCHEMA_VERSION, "this library was made by a newer app (schema {version})");
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
    Ok(())
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Store> {
        let db = Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
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
    pub fn upsert_series(&self, source: &str, id: &str, title: &str, cover: Option<&str>, description: Option<&str>, status: Status) -> anyhow::Result<()> {
        let status = serde_json::to_value(status)?.as_str().unwrap_or("unknown").to_string();
        self.db().execute(
            "INSERT INTO series (source, id, title, cover, description, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source, id) DO UPDATE SET title = ?3, cover = coalesce(?4, cover), description = coalesce(?5, description), status = ?6",
            params![source, id, title, cover, description, status],
        )?;
        Ok(())
    }

    pub fn add_to_library(&self, source: &str, id: &str, now_ms: i64) -> anyhow::Result<()> {
        let changed = self.db().execute("UPDATE series SET saved_at = coalesce(saved_at, ?3) WHERE source = ?1 AND id = ?2", params![source, id, now_ms])?;
        anyhow::ensure!(changed == 1, "that series is not known yet: save its details first");
        Ok(())
    }

    pub fn remove_from_library(&self, source: &str, id: &str) -> anyhow::Result<()> {
        self.db().execute("UPDATE series SET saved_at = NULL WHERE source = ?1 AND id = ?2", params![source, id])?;
        Ok(())
    }

    pub fn in_library(&self, source: &str, id: &str) -> anyhow::Result<bool> {
        Ok(self.db().query_row("SELECT saved_at IS NOT NULL FROM series WHERE source = ?1 AND id = ?2", params![source, id], |row| row.get(0)).optional()?.unwrap_or(false))
    }

    /// Replaces the chapter list with what the site says now (newest first) and tells which are new.
    pub fn sync_chapters(&self, source: &str, series: &str, chapters: &[Chapter]) -> anyhow::Result<Synced> {
        let mut db = self.db();
        let tx = db.transaction()?;
        let known: Vec<String> = {
            let mut statement = tx.prepare("SELECT id FROM chapter WHERE source = ?1 AND series = ?2")?;
            statement.query_map(params![source, series], |row| row.get(0))?.collect::<Result<_, _>>()?
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
            tx.execute("DELETE FROM chapter WHERE source = ?1 AND series = ?2 AND id = ?3", params![source, series, gone])?;
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
                chapter: Chapter { id: row.get(0)?, number: row.get(1)?, title: row.get(2)?, date_ms: row.get(3)?, scanlator: row.get(4)? },
                finished,
                page: if finished { None } else { page.filter(|page| *page > 0) },
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_progress(&self, source: &str, series: &str, chapter: &str, progress: Progress, now_ms: i64) -> anyhow::Result<()> {
        self.db().execute(
            "INSERT INTO progress (source, series, chapter, page, offset, finished, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (source, series, chapter) DO UPDATE SET page = ?4, offset = ?5, finished = ?6, updated_at = ?7",
            params![source, series, chapter, progress.page, progress.offset, progress.finished, now_ms],
        )?;
        Ok(())
    }

    /// Marks chapters read (or unread, which forgets their progress).
    pub fn mark(&self, source: &str, series: &str, chapters: &[&str], read: bool, now_ms: i64) -> anyhow::Result<()> {
        for chapter in chapters {
            if read {
                self.set_progress(source, series, chapter, Progress { page: 0, offset: 0.0, finished: true }, now_ms)?;
            } else {
                self.db().execute("DELETE FROM progress WHERE source = ?1 AND series = ?2 AND chapter = ?3", params![source, series, chapter])?;
            }
        }
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
            return Ok(Some(Resume { chapter, page: 0, offset: 0.0 }));
        }
        // Nothing read yet: the oldest chapter, the first to read.
        let first: Option<String> = db.query_row("SELECT id FROM chapter WHERE source = ?1 AND series = ?2 ORDER BY position DESC LIMIT 1", params![source, series], |row| row.get(0)).optional()?;
        Ok(first.filter(|_| !self.has_progress(&db, source, series)).map(|chapter| Resume { chapter, page: 0, offset: 0.0 }))
    }

    fn has_progress(&self, db: &Connection, source: &str, series: &str) -> bool {
        db.query_row("SELECT 1 FROM progress WHERE source = ?1 AND series = ?2 LIMIT 1", params![source, series], |_| Ok(())).optional().ok().flatten().is_some()
    }

    pub fn library(&self) -> anyhow::Result<Vec<LibraryItem>> {
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT s.source, s.id, s.title, s.cover,
                (SELECT count(*) FROM chapter c WHERE c.source = s.source AND c.series = s.id),
                (SELECT count(*) FROM chapter c WHERE c.source = s.source AND c.series = s.id
                    AND NOT EXISTS (SELECT 1 FROM progress p WHERE p.source = c.source AND p.series = c.series AND p.chapter = c.id AND p.finished = 1)),
                (SELECT max(p.updated_at) FROM progress p WHERE p.source = s.source AND p.series = s.id)
             FROM series s WHERE s.saved_at IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(LibraryItem { source: row.get(0)?, id: row.get(1)?, title: row.get(2)?, cover: row.get(3)?, total: row.get(4)?, unread: row.get(5)?, last_read_ms: row.get(6)? })
        })?;
        let mut items: Vec<LibraryItem> = rows.collect::<Result<_, _>>()?;
        // Most recently read first, then by title.
        items.sort_by(|a, b| b.last_read_ms.cmp(&a.last_read_ms).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        Ok(items)
    }

    /// The series read most recently, newest first.
    pub fn history(&self, limit: u32) -> anyhow::Result<Vec<(String, String, String, i64)>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT source, series, chapter, updated_at FROM progress ORDER BY updated_at DESC LIMIT ?1")?;
        let rows = statement.query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // --- repositories and what came from them ---------------------------------------------------

    /// Remembers a repository. The first key seen for it is kept, and a later index naming a different key
    /// is refused: a signed repository cannot change hands unnoticed.
    pub fn add_repo(&self, base: &str, name: &str, key: Option<&str>, signed: bool, now_ms: i64) -> anyhow::Result<()> {
        let db = self.db();
        let pinned: Option<Option<String>> = db.query_row("SELECT key FROM repo WHERE base = ?1", params![base], |row| row.get(0)).optional()?;
        if let Some(Some(pinned)) = &pinned {
            anyhow::ensure!(key == Some(pinned.as_str()), "this repository was added with a different key: it may have changed hands, so it is refused (remove it first to trust the new one)");
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
        let mut statement = db.prepare("SELECT base, name, signed FROM repo ORDER BY added_at, base")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_repo(&self, base: &str) -> anyhow::Result<()> {
        let db = self.db();
        db.execute("DELETE FROM repo WHERE base = ?1", params![base])?;
        db.execute("UPDATE installed SET repo = NULL WHERE repo = ?1", params![base])?;
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
        Ok(self.db().query_row("SELECT repo, version FROM installed WHERE source = ?1", params![source], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64))).optional()?)
    }

    pub fn forget_source(&self, source: &str) -> anyhow::Result<()> {
        let db = self.db();
        db.execute("DELETE FROM installed WHERE source = ?1", params![source])?;
        db.execute("DELETE FROM setting WHERE source = ?1", params![source])?;
        Ok(())
    }

    pub fn set_setting(&self, source: &str, key: &str, value: &str) -> anyhow::Result<()> {
        self.db().execute("INSERT INTO setting (source, key, value) VALUES (?1, ?2, ?3) ON CONFLICT (source, key) DO UPDATE SET value = ?3", params![source, key, value])?;
        Ok(())
    }

    pub fn settings(&self, source: &str) -> anyhow::Result<Vec<(String, String)>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT key, value FROM setting WHERE source = ?1 ORDER BY key")?;
        let rows = statement.query_map(params![source], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn add_category(&self, name: &str) -> anyhow::Result<i64> {
        let db = self.db();
        db.execute("INSERT INTO category (name, position) VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM category))", params![name])?;
        Ok(db.last_insert_rowid())
    }

    pub fn put_in_category(&self, source: &str, series: &str, category: i64, member: bool) -> anyhow::Result<()> {
        let db = self.db();
        if member {
            db.execute("INSERT OR IGNORE INTO series_category (source, series, category) VALUES (?1, ?2, ?3)", params![source, series, category])?;
        } else {
            db.execute("DELETE FROM series_category WHERE source = ?1 AND series = ?2 AND category = ?3", params![source, series, category])?;
        }
        Ok(())
    }

    pub fn in_category(&self, category: i64) -> anyhow::Result<Vec<(String, String)>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT source, series FROM series_category WHERE category = ?1 ORDER BY source, series")?;
        let rows = statement.query_map(params![category], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: &str, number: f64) -> Chapter {
        Chapter { id: id.into(), number: Some(number), title: None, date_ms: None, scanlator: None }
    }

    fn store_with_series() -> Store {
        let store = Store::in_memory().unwrap();
        store.upsert_series("s", "a", "Alpha", Some("c.jpg"), None, Status::Ongoing).unwrap();
        store.add_to_library("s", "a", 100).unwrap();
        store
    }

    #[test]
    fn the_first_sync_has_no_news_and_a_later_one_names_the_new_chapters() {
        let store = store_with_series();
        let first = store.sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)]).unwrap();
        assert_eq!(first, Synced { new: vec![], first: true });
        let later = store.sync_chapters("s", "a", &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)]).unwrap();
        assert_eq!(later, Synced { new: vec!["c3".into()], first: false });
        let rows = store.chapters("s", "a").unwrap();
        assert_eq!(rows.iter().map(|row| row.chapter.id.as_str()).collect::<Vec<_>>(), ["c3", "c2", "c1"]);
    }

    #[test]
    fn unread_counts_follow_progress_and_continue_goes_to_the_right_chapter() {
        let store = store_with_series();
        store.sync_chapters("s", "a", &[chapter("c3", 3.0), chapter("c2", 2.0), chapter("c1", 1.0)]).unwrap();
        assert_eq!(store.library().unwrap()[0].unread, 3);
        // Nothing read: start at the oldest chapter.
        assert_eq!(store.resume("s", "a").unwrap().unwrap().chapter, "c1");
        store.set_progress("s", "a", "c1", Progress { page: 0, offset: 0.0, finished: true }, 200).unwrap();
        // Finished chapter 1: continue with 2.
        assert_eq!(store.resume("s", "a").unwrap().unwrap().chapter, "c2");
        store.set_progress("s", "a", "c2", Progress { page: 7, offset: 0.4, finished: false }, 300).unwrap();
        let resume = store.resume("s", "a").unwrap().unwrap();
        assert_eq!((resume.chapter.as_str(), resume.page, resume.offset), ("c2", 7, 0.4));
        let item = &store.library().unwrap()[0];
        assert_eq!((item.unread, item.total, item.last_read_ms), (2, 3, Some(300)));
        let rows = store.chapters("s", "a").unwrap();
        assert_eq!(rows[1].page, Some(7));
        assert!(rows[2].finished);
    }

    #[test]
    fn everything_read_leaves_nothing_to_continue() {
        let store = store_with_series();
        store.sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)]).unwrap();
        store.mark("s", "a", &["c1", "c2"], true, 1).unwrap();
        assert_eq!(store.resume("s", "a").unwrap(), None);
        assert_eq!(store.library().unwrap()[0].unread, 0);
        store.mark("s", "a", &["c2"], false, 2).unwrap();
        assert_eq!(store.library().unwrap()[0].unread, 1);
    }

    #[test]
    fn progress_survives_a_chapter_leaving_the_list_and_library_membership_is_separate() {
        let store = store_with_series();
        store.sync_chapters("s", "a", &[chapter("c2", 2.0), chapter("c1", 1.0)]).unwrap();
        store.set_progress("s", "a", "c1", Progress { page: 3, offset: 0.0, finished: false }, 5).unwrap();
        store.sync_chapters("s", "a", &[chapter("c2", 2.0)]).unwrap();
        assert_eq!(store.history(10).unwrap().len(), 1);
        store.remove_from_library("s", "a").unwrap();
        assert!(!store.in_library("s", "a").unwrap());
        assert!(store.library().unwrap().is_empty());
        // Saving a series again does not forget when it was first saved.
        store.upsert_series("s", "a", "Alpha 2", None, None, Status::Completed).unwrap();
        assert!(store.add_to_library("s", "zzz", 1).is_err());
    }

    #[test]
    fn a_repository_keeps_the_first_key_it_was_added_with() {
        let store = Store::in_memory().unwrap();
        store.add_repo("https://a/", "A", Some("key1"), true, 1).unwrap();
        store.add_repo("https://a/", "A renamed", Some("key1"), true, 2).unwrap();
        assert!(store.add_repo("https://a/", "A", Some("key2"), true, 3).is_err());
        assert!(store.add_repo("https://a/", "A", None, false, 3).is_err());
        assert_eq!(store.repos().unwrap(), vec![("https://a/".to_string(), "A renamed".to_string(), true)]);
        // An unsigned repository can later be signed; and removing a repository forgets its key.
        store.add_repo("https://b/", "B", None, false, 4).unwrap();
        store.add_repo("https://b/", "B", Some("kb"), true, 5).unwrap();
        store.remove_repo("https://a/").unwrap();
        store.add_repo("https://a/", "A", Some("key2"), true, 6).unwrap();
    }

    #[test]
    fn what_was_installed_and_the_settings_chosen_are_remembered_per_source() {
        let store = Store::in_memory().unwrap();
        store.add_repo("https://a/", "A", None, false, 1).unwrap();
        store.set_origin("x", Some("https://a/"), 3).unwrap();
        store.set_origin("x", Some("https://a/"), 4).unwrap();
        assert_eq!(store.origin("x").unwrap(), Some((Some("https://a/".into()), 4)));
        store.set_setting("x", "lang", "fr").unwrap();
        store.set_setting("x", "lang", "es").unwrap();
        assert_eq!(store.settings("x").unwrap(), vec![("lang".to_string(), "es".to_string())]);
        store.remove_repo("https://a/").unwrap();
        assert_eq!(store.origin("x").unwrap(), Some((None, 4)));
        store.forget_source("x").unwrap();
        assert_eq!(store.origin("x").unwrap(), None);
        assert!(store.settings("x").unwrap().is_empty());
    }

    #[test]
    fn categories_hold_series_and_a_file_opens_again() {
        let dir = std::env::temp_dir().join(format!("akidev-manga-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manga.db");
        {
            let store = Store::open(&path).unwrap();
            store.upsert_series("s", "a", "Alpha", None, None, Status::Unknown).unwrap();
            let reading = store.add_category("Reading").unwrap();
            store.put_in_category("s", "a", reading, true).unwrap();
            store.put_in_category("s", "a", reading, true).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.in_category(1).unwrap(), vec![("s".to_string(), "a".to_string())]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
