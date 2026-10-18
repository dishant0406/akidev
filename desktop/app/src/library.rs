//! The downloaded library as the rest of the app sees it: where it lives, how to look in it, and
//! when its downloader is started.

use akidev_core::music::Music;
use akidev_library::container::{MasterKey, Sealed};
use akidev_library::download::{Library, MasterSource};
use akidev_library::keys;
use akidev_library::store::{Item, SavedSubtitle, State, Store};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// When the saved media ids were last read, and what they were.
type SavedIds = (Instant, HashSet<String>);

/// The list opens in the background when the app starts, the key is fetched when it is first
/// needed, and the downloader (a torrent session with its own threads and sockets) is started only
/// when something is being fetched or the screen is opened. Nothing here blocks the window: while
/// the list is still opening it reads as empty.
#[derive(Clone)]
pub struct LazyLibrary {
    root: PathBuf,
    upload_bps: Option<u32>,
    store: Arc<OnceLock<Result<Arc<Store>, String>>>,
    master: Arc<OnceLock<MasterKey>>,
    downloader: Arc<tokio::sync::OnceCell<Result<Arc<Library>, String>>>,
    /// Finds the audio of a track, set once the music side has started.
    music: Arc<OnceLock<Arc<Music>>>,
    /// The media ids with a saved copy, read at most once a second.
    saved: Arc<Mutex<Option<SavedIds>>>,
}

impl LazyLibrary {
    /// Opens the library in `root` on a thread of its own. What was left unfinished carries on, and
    /// the key is fetched now if there is anything saved, so a permission dialog from the system
    /// comes at launch and not in the middle of pressing play.
    pub fn new(root: PathBuf, upload_bps: Option<u32>) -> LazyLibrary {
        let library = LazyLibrary {
            root,
            upload_bps,
            store: Default::default(),
            master: Default::default(),
            downloader: Default::default(),
            music: Default::default(),
            saved: Default::default(),
        };
        let opening = library.clone();
        std::thread::spawn(move || {
            let opened = Store::open(&opening.root).map(Arc::new).map_err(|error| {
                tracing::error!("[library] could not open the list: {error:#}");
                format!("{error:#}")
            });
            let items = opened
                .as_ref()
                .ok()
                .and_then(|store| store.items().ok())
                .unwrap_or_default();
            let _ = opening.store.set(opened);
            if items.iter().any(|item| item.state == State::Ready) {
                let _ = opening.master();
            }
            if items
                .iter()
                .any(|item| matches!(item.state, State::Queued | State::Active | State::Sealing))
            {
                crate::runtime::spawn(async move {
                    let _ = opening.get().await;
                });
            }
        });
        library
    }

    /// Lets the library save music: `music` is how a track's audio is found.
    pub fn use_music(&self, music: Arc<Music>) {
        let _ = self.music.set(music);
    }

    /// The list of saved items, or an error while it is still opening or could not be opened.
    pub fn store(&self) -> Result<Arc<Store>, String> {
        match self.store.get() {
            Some(opened) => opened.clone(),
            None => Err("The library is still opening.".into()),
        }
    }

    /// The master key. Asking again after a failure asks the keychain again.
    pub fn master(&self) -> Result<MasterKey, String> {
        if let Some(key) = self.master.get() {
            return Ok(key.clone());
        }
        let key = keys::load(&self.root).map_err(|error| {
            tracing::error!("[library] no key for the library: {error:#}");
            format!("{error:#}")
        })?;
        let _ = self.master.set(key.clone());
        Ok(key)
    }

    /// The downloader, started now if it has not been.
    pub async fn get(&self) -> Result<Arc<Library>, String> {
        self.downloader
            .get_or_init(|| async {
                // The list may still be opening, which takes a moment.
                while self.store.get().is_none() {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                let store = self.store()?;
                let this = self.clone();
                let master: MasterSource =
                    Arc::new(move || this.master().map_err(|error| anyhow::anyhow!(error)));
                Library::open(
                    &self.root,
                    store,
                    master,
                    self.music.get().cloned(),
                    self.upload_bps,
                )
                .await
                .map_err(|error| {
                    tracing::error!("[library] could not start downloading: {error:#}");
                    format!("{error:#}")
                })
            })
            .await
            .clone()
    }

    /// The downloader if it is already running.
    pub fn running(&self) -> Option<Arc<Library>> {
        self.downloader.get()?.as_ref().ok().cloned()
    }

    /// Free space on the drive the library is on.
    pub fn free_space(&self) -> u64 {
        std::fs::create_dir_all(&self.root).ok();
        akidev_library::download::free_space(&self.root)
    }

    /// The saved copy of a video: of this release when one is named, else of the newest.
    pub fn saved_video(&self, media_id: &str, release: Option<&str>) -> Option<Item> {
        self.store()
            .ok()?
            .ready_video(media_id, release)
            .ok()
            .flatten()
    }

    /// The subtitles saved with an item, with where each file is.
    pub fn saved_subtitles(&self, item_id: &str) -> Vec<(SavedSubtitle, PathBuf)> {
        let Some(saved) = self
            .store()
            .ok()
            .and_then(|store| store.saved_subtitles(item_id).ok())
        else {
            return Vec::new();
        };
        saved
            .into_iter()
            .map(|subtitle| {
                let path = self.root.join(&subtitle.file);
                (subtitle, path)
            })
            .filter(|(_, path)| path.is_file())
            .collect()
    }

    /// The saved copy of a track.
    pub fn saved_track(&self, track_id: &str) -> Option<Item> {
        self.store().ok()?.ready_track(track_id).ok().flatten()
    }

    /// The saved file of a ready item, opened for playing.
    pub fn open(&self, item: &Item) -> Result<Sealed, String> {
        let file = item.file.as_ref().ok_or("This has no saved file.")?;
        Sealed::open(&self.root.join(file), &self.master()?).map_err(|error| format!("{error:#}"))
    }

    /// The media ids that have a saved copy, for marking them on a list. Read from the database at
    /// most once a second, since a list asks on every frame.
    pub fn saved_media_ids(&self) -> HashSet<String> {
        let mut cached = self.saved.lock().unwrap();
        if let Some((read, ids)) = cached.as_ref()
            && read.elapsed() < Duration::from_secs(1)
        {
            return ids.clone();
        }
        let ids: HashSet<String> = self
            .store()
            .ok()
            .and_then(|store| store.items().ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|item| item.state == State::Ready)
            .map(|item| item.media_id)
            .collect();
        *cached = Some((Instant::now(), ids.clone()));
        ids
    }
}
