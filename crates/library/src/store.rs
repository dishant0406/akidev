//! What is in the library: the saved items, where each one's file is, how far it got, and the
//! collections the user made. One SQLite file, `library.db`, next to the saved files.

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

/// Where an item is in its life. Text in the database, so a later version can read an older file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Waiting for its turn.
    Queued,
    /// Being fetched.
    Active,
    /// The user stopped it.
    Paused,
    /// Fetched, being locked into its saved file.
    Sealing,
    /// Saved and playable.
    Ready,
    /// Gave up; `note` says why.
    Failed,
}

impl State {
    fn as_text(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Active => "active",
            State::Paused => "paused",
            State::Sealing => "sealing",
            State::Ready => "ready",
            State::Failed => "failed",
        }
    }

    fn from_text(text: &str) -> State {
        match text {
            "active" => State::Active,
            "paused" => State::Paused,
            "sealing" => State::Sealing,
            "ready" => State::Ready,
            "failed" => State::Failed,
            _ => State::Queued,
        }
    }
}

/// Film, episode or track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Video,
    Music,
}

impl Kind {
    fn as_text(self) -> &'static str {
        match self {
            Kind::Video => "video",
            Kind::Music => "music",
        }
    }

    fn from_text(text: &str) -> Kind {
        if text == "music" {
            Kind::Music
        } else {
            Kind::Video
        }
    }
}

/// One saved (or being saved) film, episode or track.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What names it in the library: the media id, with the release after a `#` for a video.
    pub id: String,
    pub kind: Kind,
    /// The media id (`series-tt0903747-s1e5`, a YouTube video id for music).
    pub media_id: String,
    /// The release's info hash, for a video.
    pub release: Option<String>,
    /// The file within the torrent, once it has been chosen.
    pub file_index: Option<u32>,
    pub magnet: Option<String>,
    pub title: String,
    /// "Season 1, episode 5", or the artist for a track.
    pub subtitle: String,
    pub poster: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub state: State,
    pub note: Option<String>,
    /// The size of the saved media, once known.
    pub bytes: u64,
    /// The saved file, relative to the library folder, once there is one.
    pub file: Option<String>,
    pub added: i64,
}

impl Item {
    /// The id a video is saved under.
    pub fn video_id(media_id: &str, release: &str) -> String {
        format!("{media_id}#{release}")
    }

    /// A new, queued item with nothing known beyond what it is.
    pub fn new(id: String, kind: Kind, media_id: String, title: String) -> Item {
        Item {
            id,
            kind,
            media_id,
            release: None,
            file_index: None,
            magnet: None,
            title,
            subtitle: String::new(),
            poster: None,
            season: None,
            episode: None,
            state: State::Queued,
            note: None,
            bytes: 0,
            file: None,
            added: now(),
        }
    }
}

/// A group of saved items the user made.
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub items: Vec<String>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

const COLUMNS: &str = "id, kind, media_id, release, file_index, magnet, title, subtitle, poster, \
    season, episode, state, note, bytes, file, added";

fn item_from(row: &rusqlite::Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: row.get(0)?,
        kind: Kind::from_text(&row.get::<_, String>(1)?),
        media_id: row.get(2)?,
        release: row.get(3)?,
        file_index: row.get(4)?,
        magnet: row.get(5)?,
        title: row.get(6)?,
        subtitle: row.get(7)?,
        poster: row.get(8)?,
        season: row.get(9)?,
        episode: row.get(10)?,
        state: State::from_text(&row.get::<_, String>(11)?),
        note: row.get(12)?,
        bytes: row.get::<_, i64>(13)? as u64,
        file: row.get(14)?,
        added: row.get(15)?,
    })
}

/// A subtitle to save with an item: where it is, and what language.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleRef {
    pub lang: String,
    pub language: String,
    pub url: String,
}

/// A subtitle saved with an item, as a file under the library folder.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSubtitle {
    pub lang: String,
    pub language: String,
    /// Relative to the library folder.
    pub file: String,
}

pub struct Store {
    db: Mutex<Connection>,
}

/// The database's version. A later change to the tables raises it and adds a step to `migrate`.
const VERSION: i64 = 1;

impl Store {
    /// Opens `library.db` in `dir`, making it if it is not there. A file that is not a database
    /// is moved aside (as `library.db.damaged`) and a new one is started: the saved files are
    /// still there, only the list is lost, and the list can be rebuilt from them.
    pub fn open(dir: &Path) -> anyhow::Result<Store> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("library.db");
        let db = match open_checked(&path) {
            Ok(db) => db,
            // A list from a newer Akidev is not damaged, and is left as it is for that version.
            Err(error) if error.to_string().contains("newer Akidev") => return Err(error),
            Err(error) => {
                tracing::warn!("[library] the list is damaged, starting a new one: {error:#}");
                let _ = std::fs::rename(&path, dir.join("library.db.damaged"));
                // What belonged to the damaged file must not be read as part of the new one.
                let _ = std::fs::remove_file(dir.join("library.db-wal"));
                let _ = std::fs::remove_file(dir.join("library.db-shm"));
                open_checked(&path)?
            }
        };
        Ok(Store { db: Mutex::new(db) })
    }

    #[cfg(test)]
    fn in_memory() -> Store {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        Store { db: Mutex::new(db) }
    }

    /// Adds an item, or leaves the one that is already there as it is. True when it was added.
    pub fn add(&self, item: &Item) -> anyhow::Result<bool> {
        let db = self.db.lock().unwrap();
        let added = db.execute(
            &format!("INSERT OR IGNORE INTO items ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"),
            params![
                item.id,
                item.kind.as_text(),
                item.media_id,
                item.release,
                item.file_index,
                item.magnet,
                item.title,
                item.subtitle,
                item.poster,
                item.season,
                item.episode,
                item.state.as_text(),
                item.note,
                item.bytes as i64,
                item.file,
                item.added,
            ],
        )?;
        Ok(added == 1)
    }

    pub fn item(&self, id: &str) -> anyhow::Result<Option<Item>> {
        let db = self.db.lock().unwrap();
        Ok(db
            .query_row(
                &format!("SELECT {COLUMNS} FROM items WHERE id = ?1"),
                [id],
                item_from,
            )
            .optional()?)
    }

    /// Every item, newest first.
    pub fn items(&self) -> anyhow::Result<Vec<Item>> {
        let db = self.db.lock().unwrap();
        let mut query = db.prepare(&format!(
            "SELECT {COLUMNS} FROM items ORDER BY added DESC, id"
        ))?;
        let items = query
            .query_map([], item_from)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// The saved copy of a video, if there is one.
    pub fn ready_video(
        &self,
        media_id: &str,
        release: Option<&str>,
    ) -> anyhow::Result<Option<Item>> {
        let db = self.db.lock().unwrap();
        let (sql, release) = match release {
            Some(release) => (
                format!(
                    "SELECT {COLUMNS} FROM items WHERE kind = 'video' AND media_id = ?1 AND release = ?2 AND state = 'ready'"
                ),
                release.to_string(),
            ),
            None => (
                format!(
                    "SELECT {COLUMNS} FROM items WHERE kind = 'video' AND media_id = ?1 AND ?2 = '' AND state = 'ready' ORDER BY added DESC"
                ),
                String::new(),
            ),
        };
        Ok(db
            .query_row(&sql, params![media_id, release], item_from)
            .optional()?)
    }

    /// The saved copy of a track, if there is one.
    pub fn ready_track(&self, media_id: &str) -> anyhow::Result<Option<Item>> {
        let db = self.db.lock().unwrap();
        Ok(db
            .query_row(
                &format!("SELECT {COLUMNS} FROM items WHERE kind = 'music' AND media_id = ?1 AND state = 'ready'"),
                [media_id],
                item_from,
            )
            .optional()?)
    }

    pub fn set_state(&self, id: &str, state: State, note: Option<&str>) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE items SET state = ?2, note = ?3 WHERE id = ?1",
            params![id, state.as_text(), note],
        )?;
        Ok(())
    }

    /// Records which file of the torrent was chosen.
    pub fn set_file_index(&self, id: &str, file_index: u32, bytes: u64) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE items SET file_index = ?2, bytes = ?3 WHERE id = ?1",
            params![id, file_index, bytes as i64],
        )?;
        Ok(())
    }

    /// Records the saved file, and that the item is ready.
    pub fn set_saved(&self, id: &str, file: &str, bytes: u64) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE items SET state = 'ready', note = NULL, file = ?2, bytes = ?3 WHERE id = ?1",
            params![id, file, bytes as i64],
        )?;
        Ok(())
    }

    /// Where the viewer stopped.
    /// Removes an item and its place in every collection.
    pub fn remove(&self, id: &str) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM collection_items WHERE item_id = ?1", [id])?;
        db.execute("DELETE FROM subtitles WHERE item_id = ?1", [id])?;
        db.execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Notes the subtitles to fetch for an item. Ones already noted are left as they are.
    pub fn add_subtitles(&self, id: &str, subtitles: &[SubtitleRef]) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        for subtitle in subtitles {
            db.execute(
                "INSERT OR IGNORE INTO subtitles (item_id, lang, language, url) VALUES (?1, ?2, ?3, ?4)",
                params![id, subtitle.lang, subtitle.language, subtitle.url],
            )?;
        }
        Ok(())
    }

    /// The subtitles noted for an item that have not been fetched yet.
    pub fn pending_subtitles(&self, id: &str) -> anyhow::Result<Vec<SubtitleRef>> {
        let db = self.db.lock().unwrap();
        let mut query = db.prepare(
            "SELECT lang, language, url FROM subtitles WHERE item_id = ?1 AND file IS NULL ORDER BY rowid",
        )?;
        let found = query
            .query_map([id], |row| {
                Ok(SubtitleRef {
                    lang: row.get(0)?,
                    language: row.get(1)?,
                    url: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(found)
    }

    /// Records the file a subtitle was saved to.
    pub fn set_subtitle_file(&self, id: &str, url: &str, file: &str) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE subtitles SET file = ?3 WHERE item_id = ?1 AND url = ?2",
            params![id, url, file],
        )?;
        Ok(())
    }

    /// The subtitles saved with an item.
    pub fn saved_subtitles(&self, id: &str) -> anyhow::Result<Vec<SavedSubtitle>> {
        let db = self.db.lock().unwrap();
        let mut query = db.prepare(
            "SELECT lang, language, file FROM subtitles WHERE item_id = ?1 AND file IS NOT NULL ORDER BY rowid",
        )?;
        let found = query
            .query_map([id], |row| {
                Ok(SavedSubtitle {
                    lang: row.get(0)?,
                    language: row.get(1)?,
                    file: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(found)
    }

    /// Makes a collection. A name already in use gives back that collection.
    pub fn add_collection(&self, name: &str) -> anyhow::Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR IGNORE INTO collections (name, created) VALUES (?1, ?2)",
            params![name, now()],
        )?;
        Ok(db.query_row(
            "SELECT id FROM collections WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?)
    }

    /// Deletes a collection, not the items in it.
    pub fn remove_collection(&self, id: i64) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1",
            [id],
        )?;
        db.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn add_to_collection(&self, collection: i64, item: &str) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR IGNORE INTO collection_items (collection_id, item_id, added) VALUES (?1, ?2, ?3)",
            params![collection, item, now()],
        )?;
        Ok(())
    }

    pub fn remove_from_collection(&self, collection: i64, item: &str) -> anyhow::Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND item_id = ?2",
            params![collection, item],
        )?;
        Ok(())
    }

    /// The collections, with the ids of the items in each in the order they were added.
    pub fn collections(&self) -> anyhow::Result<Vec<Collection>> {
        let db = self.db.lock().unwrap();
        let mut query = db.prepare("SELECT id, name FROM collections ORDER BY created, id")?;
        let mut collections = query
            .query_map([], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    items: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut members = db.prepare(
            "SELECT item_id FROM collection_items WHERE collection_id = ?1 ORDER BY added, item_id",
        )?;
        for collection in &mut collections {
            collection.items = members
                .query_map([collection.id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(collections)
    }
}

fn open_checked(path: &Path) -> anyhow::Result<Connection> {
    let db = Connection::open(path)?;
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.pragma_update(None, "foreign_keys", "ON")?;
    let check: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    anyhow::ensure!(check == "ok", "integrity check: {check}");
    migrate(&db)?;
    Ok(db)
}

fn migrate(db: &Connection) -> anyhow::Result<()> {
    let found: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    anyhow::ensure!(found <= VERSION, "the library is from a newer Akidev");
    if found < 1 {
        db.execute_batch(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY, kind TEXT NOT NULL, media_id TEXT NOT NULL, release TEXT,
                file_index INTEGER, magnet TEXT, title TEXT NOT NULL, subtitle TEXT NOT NULL,
                poster TEXT, season INTEGER, episode INTEGER, state TEXT NOT NULL, note TEXT,
                bytes INTEGER NOT NULL, file TEXT, added INTEGER NOT NULL
            );
            CREATE TABLE collections (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created INTEGER NOT NULL
            );
            CREATE TABLE subtitles (
                item_id TEXT NOT NULL, lang TEXT NOT NULL, language TEXT NOT NULL,
                url TEXT NOT NULL, file TEXT, PRIMARY KEY (item_id, url)
            );
            CREATE TABLE collection_items (
                collection_id INTEGER NOT NULL, item_id TEXT NOT NULL, added INTEGER NOT NULL,
                PRIMARY KEY (collection_id, item_id)
            );",
        )?;
    }
    db.pragma_update(None, "user_version", VERSION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(media: &str, release: &str) -> Item {
        let mut item = Item::new(
            Item::video_id(media, release),
            Kind::Video,
            media.into(),
            "Show".into(),
        );
        item.release = Some(release.into());
        item
    }

    #[test]
    fn an_item_is_added_once_and_read_back() {
        let store = Store::in_memory();
        let mut item = video("series-tt1-s1e1", "abc");
        item.poster = Some("https://x/p.jpg".into());
        item.season = Some(1);
        assert!(store.add(&item).unwrap());
        // A second add of the same item changes nothing, so a double press starts one download.
        let mut again = item.clone();
        again.title = "Other".into();
        assert!(!store.add(&again).unwrap());
        assert_eq!(store.item(&item.id).unwrap().unwrap(), item);
        assert_eq!(store.items().unwrap().len(), 1);
        assert!(store.item("missing").unwrap().is_none());
    }

    #[test]
    fn progress_through_the_states_is_kept() {
        let store = Store::in_memory();
        let item = video("series-tt1-s1e1", "abc");
        store.add(&item).unwrap();
        store
            .set_state(&item.id, State::Active, Some("Looking for peers"))
            .unwrap();
        store.set_file_index(&item.id, 3, 1000).unwrap();
        let read = store.item(&item.id).unwrap().unwrap();
        assert_eq!(
            (read.state, read.file_index, read.bytes),
            (State::Active, Some(3), 1000)
        );
        assert!(
            store
                .ready_video("series-tt1-s1e1", Some("abc"))
                .unwrap()
                .is_none()
        );
        store.set_saved(&item.id, "ab/abc.akc", 1000).unwrap();
        let read = store.item(&item.id).unwrap().unwrap();
        assert_eq!(
            (read.state, read.note, read.file.as_deref()),
            (State::Ready, None, Some("ab/abc.akc"))
        );
        // Found by what the stream would play, and by the film alone.
        assert!(
            store
                .ready_video("series-tt1-s1e1", Some("abc"))
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .ready_video("series-tt1-s1e1", None)
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .ready_video("series-tt1-s1e1", Some("other"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn collections_hold_items_without_owning_them() {
        let store = Store::in_memory();
        let (a, b) = (video("m1", "a"), video("m2", "b"));
        store.add(&a).unwrap();
        store.add(&b).unwrap();
        let watch = store.add_collection("Watch later").unwrap();
        // The same name is the same collection.
        assert_eq!(store.add_collection("Watch later").unwrap(), watch);
        let kids = store.add_collection("Kids").unwrap();
        for (collection, item) in [(watch, &a), (watch, &b), (kids, &a), (kids, &a)] {
            store.add_to_collection(collection, &item.id).unwrap();
        }
        let all = store.collections().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].items, [a.id.clone(), b.id.clone()]);
        assert_eq!(all[1].items, std::slice::from_ref(&a.id));

        store.remove_from_collection(watch, &b.id).unwrap();
        // Deleting a collection keeps its items.
        store.remove_collection(kids).unwrap();
        assert_eq!(store.items().unwrap().len(), 2);
        assert_eq!(store.collections().unwrap().len(), 1);
        // Removing an item takes it out of the collections that held it.
        store.remove(&a.id).unwrap();
        assert!(store.collections().unwrap()[0].items.is_empty());
    }

    #[test]
    fn a_damaged_list_is_set_aside_and_a_new_one_started() {
        let dir = std::env::temp_dir().join(format!("akidev-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("library.db"),
            b"this is not a database at all, it is long enough to be read as a header",
        )
        .unwrap();
        let store = Store::open(&dir).unwrap();
        assert!(store.items().unwrap().is_empty());
        assert!(dir.join("library.db.damaged").exists());
        store.add(&video("m", "r")).unwrap();
        drop(store);
        // Opened again, the list is what was saved.
        assert_eq!(Store::open(&dir).unwrap().items().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn subtitles_are_noted_fetched_and_removed_with_their_item() {
        let store = Store::in_memory();
        let item = video("m", "r");
        store.add(&item).unwrap();
        let english = SubtitleRef {
            lang: "en".into(),
            language: "English".into(),
            url: "https://x/en.srt".into(),
        };
        let french = SubtitleRef {
            lang: "fr".into(),
            language: "French".into(),
            url: "https://x/fr.srt".into(),
        };
        store
            .add_subtitles(&item.id, &[english.clone(), french.clone()])
            .unwrap();
        // Noting one again changes nothing.
        store
            .add_subtitles(&item.id, std::slice::from_ref(&english))
            .unwrap();
        assert_eq!(
            store.pending_subtitles(&item.id).unwrap(),
            [english.clone(), french]
        );
        assert!(store.saved_subtitles(&item.id).unwrap().is_empty());
        store
            .set_subtitle_file(&item.id, &english.url, "subtitles/ab/0.vtt")
            .unwrap();
        assert_eq!(store.pending_subtitles(&item.id).unwrap().len(), 1);
        let saved = store.saved_subtitles(&item.id).unwrap();
        assert_eq!(
            (saved[0].lang.as_str(), saved[0].file.as_str()),
            ("en", "subtitles/ab/0.vtt")
        );
        store.remove(&item.id).unwrap();
        assert!(store.pending_subtitles(&item.id).unwrap().is_empty());
        assert!(store.saved_subtitles(&item.id).unwrap().is_empty());
    }
}
