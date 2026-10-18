//! Saving an item: finding it in its swarm, fetching the one file, locking it into a saved file,
//! and keeping the list of what is saved.
//!
//! One item is fetched at a time, oldest first. The fetch uses a torrent session of its own, kept
//! on disk, so a download carries on after the app is closed and opened again. The file is fetched
//! plain into a staging folder, then sealed (see `container`), and the plain copy and the torrent
//! are deleted. Nothing is uploaded: the session accepts no incoming connections, and the torrent
//! is dropped the moment its file is complete.

use crate::container::{self, MasterKey};
use crate::store::{Item, Kind, State, Store, SubtitleRef};
use akidev_core::music::Music;
use akidev_core::releases::{MIN_VIDEO_BYTES, VideoFile, pick_video_file};
use anyhow::Context;
use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
    SessionPersistenceConfig,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Gives the master key when it is needed. The key lives in the system's keychain, which may ask
/// the user for permission, so it is fetched when a file is first sealed or opened and not before.
pub type MasterSource = Arc<dyn Fn() -> anyhow::Result<MasterKey> + Send + Sync>;

/// The free space on the drive `dir` is on, in bytes. Unknown counts as plenty.
pub fn free_space(dir: &Path) -> u64 {
    fs2::available_space(dir).unwrap_or(u64::MAX)
}

/// How often progress is read.
const LOOK_EVERY: Duration = Duration::from_millis(500);
/// How long a track's download may go without a byte before it is given up on.
const AUDIO_QUIET_FOR: Duration = Duration::from_secs(30);
/// How often the disk is checked again while there is not enough room.
const SPACE_EVERY: Duration = Duration::from_secs(30);
/// Disk kept free beyond the item itself, so the machine is not left with nothing.
const SPARE_BYTES: u64 = 256 * 1024 * 1024;

/// How an item in the queue is going, for the screen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    /// Bytes of the file fetched so far, and its size once known.
    pub done: u64,
    pub total: u64,
    /// Bytes a second.
    pub speed: u64,
    pub peers: u32,
    /// What is going on in words: "Looking for peers", "Waiting for space"...
    pub note: String,
}

/// How long things may take. The app uses `Timing::default()`; tests use short ones.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// How long a swarm gets to say what files it has.
    pub metadata: Duration,
    /// How long without a byte before the screen says nothing is arriving.
    pub stalled: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            metadata: Duration::from_secs(120),
            stalled: Duration::from_secs(5 * 60),
        }
    }
}

/// How the fetching is done: how long things may take, and who to ask besides the swarm. The app
/// uses the trackers of the published sources; tests name a seeder on this machine.
pub struct Fetching {
    pub timing: Timing,
    /// Peers to try directly.
    pub peers: Vec<SocketAddr>,
    pub trackers: Vec<String>,
}

enum Stop {
    Pause,
    Cancel,
}

enum Outcome {
    /// The file is complete at this path, with this length.
    Fetched(PathBuf, u64),
    Paused,
    Cancelled,
    Failed(String),
}

pub struct Library {
    root: PathBuf,
    store: Arc<Store>,
    master: MasterSource,
    /// Finds the audio of a track, for the music in the library.
    music: Option<Arc<Music>>,
    session: Arc<Session>,
    timing: Timing,
    /// Peers to try directly, besides what the swarm finds. Tests use this to reach a seeder.
    peers: Vec<SocketAddr>,
    trackers: Vec<String>,
    progress: Mutex<HashMap<String, Progress>>,
    /// The names of items that finished saving, until the screen has said so.
    finished: Mutex<Vec<String>>,
    /// What the user asked of the item being fetched now.
    stop: Mutex<Option<(String, Stop)>>,
    wake: Notify,
}

impl Library {
    /// Opens the library in `root` and starts fetching what is queued. `upload_bps` caps uploads.
    pub async fn open(
        root: &Path,
        store: Arc<Store>,
        master: MasterSource,
        music: Option<Arc<Music>>,
        upload_bps: Option<u32>,
    ) -> anyhow::Result<Arc<Library>> {
        let session_dir = root.join("session");
        std::fs::create_dir_all(&session_dir)?;
        let session = Session::new_with_opts(
            root.join("staging"),
            SessionOptions {
                // Its own DHT port: the streaming session keeps the usual one.
                dht: Some(librqbit::DhtSessionConfig {
                    port: None,
                    persistence: None,
                    ..Default::default()
                }),
                persistence: Some(SessionPersistenceConfig::Json {
                    folder: Some(session_dir),
                }),
                fastresume: true,
                ratelimits: librqbit::limits::LimitsConfig {
                    upload_bps: upload_bps.and_then(std::num::NonZeroU32::new),
                    download_bps: None,
                },
                ..Default::default()
            },
        )
        .await?;
        let fetching = Fetching {
            timing: Timing::default(),
            peers: Vec::new(),
            trackers: akidev_core::sources::current().trackers.clone(),
        };
        Library::start(root, store, master, music, session, fetching).await
    }

    /// Opens the library on a session the caller made.
    pub async fn start(
        root: &Path,
        store: Arc<Store>,
        master: MasterSource,
        music: Option<Arc<Music>>,
        session: Arc<Session>,
        fetching: Fetching,
    ) -> anyhow::Result<Arc<Library>> {
        // Whatever was being fetched when the app closed is fetched again.
        for item in store.items()? {
            if matches!(item.state, State::Active | State::Sealing) {
                store.set_state(&item.id, State::Queued, None)?;
            }
        }
        let library = Arc::new(Library {
            root: root.to_path_buf(),
            store,
            master,
            music,
            session,
            timing: fetching.timing,
            peers: fetching.peers,
            trackers: fetching.trackers,
            progress: Mutex::new(HashMap::new()),
            finished: Mutex::new(Vec::new()),
            stop: Mutex::new(None),
            wake: Notify::new(),
        });
        library.tidy().await?;
        tokio::spawn(run(library.clone()));
        Ok(library)
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// How an item that is being fetched is going.
    pub fn progress(&self, id: &str) -> Option<Progress> {
        self.progress.lock().unwrap().get(id).cloned()
    }

    /// Whether something is being fetched right now.
    pub fn busy(&self) -> bool {
        !self.progress.lock().unwrap().is_empty()
    }

    /// Queues an item with the subtitles to save beside it. False when it was already in the library.
    pub fn add_with_subtitles(
        &self,
        item: &Item,
        subtitles: &[SubtitleRef],
    ) -> anyhow::Result<bool> {
        self.store.add_subtitles(&item.id, subtitles)?;
        self.add(item)
    }

    /// The names of items that have finished saving since this was last asked.
    pub fn take_finished(&self) -> Vec<String> {
        std::mem::take(&mut self.finished.lock().unwrap())
    }

    /// Queues an item. False when it was already in the library.
    pub fn add(&self, item: &Item) -> anyhow::Result<bool> {
        let added = self.store.add(item)?;
        // Asking again for one that stopped or failed tries it again.
        let retried = !added
            && self
                .store
                .item(&item.id)?
                .is_some_and(|stored| matches!(stored.state, State::Paused | State::Failed));
        if retried {
            self.resume(&item.id)?;
        }
        self.wake.notify_one();
        Ok(added || retried)
    }

    /// Stops an item, keeping what has been fetched.
    pub fn pause(&self, id: &str) -> anyhow::Result<()> {
        if self.ask(id, Stop::Pause) {
            return Ok(());
        }
        if self
            .store
            .item(id)?
            .is_some_and(|item| item.state == State::Queued)
        {
            self.store.set_state(id, State::Paused, None)?;
        }
        Ok(())
    }

    /// Puts a paused or failed item back in the queue.
    pub fn resume(&self, id: &str) -> anyhow::Result<()> {
        if self
            .store
            .item(id)?
            .is_some_and(|item| matches!(item.state, State::Paused | State::Failed))
        {
            self.store.set_state(id, State::Queued, None)?;
            self.wake.notify_one();
        }
        Ok(())
    }

    /// Removes an item: stops it if it is being fetched, and deletes what was saved or fetched.
    pub async fn remove(&self, id: &str) -> anyhow::Result<()> {
        if self.ask(id, Stop::Cancel) {
            // The worker finishes the removal when it sees the request.
            return Ok(());
        }
        self.discard(id).await
    }

    fn ask(&self, id: &str, stop: Stop) -> bool {
        let mut current = self.stop.lock().unwrap();
        if self.progress.lock().unwrap().contains_key(id) {
            *current = Some((id.to_string(), stop));
            true
        } else {
            false
        }
    }

    fn requested(&self, id: &str) -> Option<Stop> {
        let mut current = self.stop.lock().unwrap();
        match current.take() {
            Some((asked, stop)) if asked == id => Some(stop),
            other => {
                *current = other;
                None
            }
        }
    }

    fn note(&self, id: &str, note: &str) {
        self.progress
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .note = note.to_string();
    }

    /// Deletes an item's saved file and staged torrent, and its row.
    async fn discard(&self, id: &str) -> anyhow::Result<()> {
        if let Some(item) = self.store.item(id)? {
            if let Some(file) = &item.file {
                let _ = std::fs::remove_file(self.root.join(file));
            }
            let _ = std::fs::remove_dir_all(self.root.join(subtitle_dir(&item.id)));
            self.drop_torrent_of(&item).await;
        }
        self.store.remove(id)
    }

    /// Whether another item of the same release is still waiting for the torrent. Every episode of a
    /// season pack shares one, so one episode's end must not take the others' torrent with it.
    fn others_need(&self, id: &str, release: &str) -> bool {
        self.store.items().is_ok_and(|items| {
            items.iter().any(|other| {
                other.id != id
                    && other
                        .release
                        .as_deref()
                        .is_some_and(|hash| hash.eq_ignore_ascii_case(release))
                    && matches!(
                        other.state,
                        State::Queued | State::Active | State::Paused | State::Sealing
                    )
            })
        })
    }

    /// Drops the item's torrent, unless another item of the pack still needs it.
    async fn drop_torrent_of(&self, item: &Item) {
        let Some(release) = item.release.as_deref() else {
            return;
        };
        if self.others_need(&item.id, release) {
            return;
        }
        if let Ok(hash) = Id20::from_str(release) {
            self.drop_torrent(hash).await;
        }
    }

    async fn drop_torrent(&self, hash: Id20) {
        if let Err(error) = self.session.delete(TorrentIdOrHash::Hash(hash), true).await {
            tracing::debug!("[library] deleting a torrent failed: {error:#}");
        }
    }

    /// Drops torrents the session remembers that no waiting item needs, and sealed files that no
    /// item points at.
    async fn tidy(&self) -> anyhow::Result<()> {
        let needed: HashSet<String> = self
            .store
            .items()?
            .into_iter()
            .filter(|item| item.state != State::Ready && item.state != State::Failed)
            .filter_map(|item| item.release)
            .map(|hash| hash.to_lowercase())
            .collect();
        // Saved files that no item points at (a seal that was cut short) and the halves of ones
        // that never finished.
        let referenced: HashSet<PathBuf> = self
            .store
            .items()?
            .into_iter()
            .filter_map(|item| item.file)
            .map(|file| self.root.join(file))
            .collect();
        for path in files_under(&self.root.join("files")) {
            if !referenced.contains(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
        // What was being read when the app closed starts again from the beginning.
        let _ = std::fs::remove_dir_all(self.root.join("staging").join("music"));
        let remembered: Vec<Id20> = self
            .session
            .with_torrents(|torrents| torrents.map(|(_, handle)| handle.info_hash()).collect());
        for hash in remembered {
            if !needed.contains(&hash.as_string().to_lowercase()) {
                self.drop_torrent(hash).await;
            }
        }
        Ok(())
    }

    /// The next item to fetch: the oldest that is waiting.
    fn next(&self) -> anyhow::Result<Option<Item>> {
        let mut waiting: Vec<Item> = self
            .store
            .items()?
            .into_iter()
            .filter(|item| item.state == State::Queued)
            .collect();
        waiting.sort_by_key(|item| item.added);
        Ok(waiting.into_iter().next())
    }

    /// Fetches the item's file. Does not touch the database: the caller does, from the outcome.
    async fn fetch(&self, item: &Item) -> Outcome {
        let Some(magnet) = item.magnet.as_deref() else {
            return Outcome::Failed("There is no source for this yet.".into());
        };
        self.note(&item.id, "Looking for peers");
        let options = AddTorrentOptions {
            // Nothing is chosen until the file list is known.
            only_files: Some(Vec::new()),
            overwrite: true,
            trackers: Some(self.trackers.clone()),
            initial_peers: (!self.peers.is_empty()).then(|| self.peers.clone()),
            ..Default::default()
        };
        let adding = self
            .session
            .add_torrent(AddTorrent::from_url(magnet), Some(options));
        let added = self
            .unless_stopped(&item.id, tokio::time::timeout(self.timing.metadata, adding))
            .await;
        let added = match added {
            Ok(added) => added,
            Err(Stop::Pause) => return Outcome::Paused,
            Err(Stop::Cancel) => return Outcome::Cancelled,
        };
        let handle = match added {
            Ok(Ok(
                AddTorrentResponse::Added(_, handle)
                | AddTorrentResponse::AlreadyManaged(_, handle),
            )) => handle,
            Ok(Ok(AddTorrentResponse::ListOnly(_))) => {
                return Outcome::Failed("The source answered with a list only.".into());
            }
            Ok(Err(error)) => return Outcome::Failed(format!("{error:#}")),
            Err(_) => {
                self.drop_torrent_of(item).await;
                return Outcome::Failed("No one is sharing this right now.".into());
            }
        };
        let initialised = self
            .unless_stopped(
                &item.id,
                tokio::time::timeout(self.timing.metadata, handle.wait_until_initialized()),
            )
            .await;
        match initialised {
            Err(Stop::Pause) => return Outcome::Paused,
            Err(Stop::Cancel) => return Outcome::Cancelled,
            Ok(Err(_)) => return Outcome::Failed("The files could not be checked in time.".into()),
            Ok(Ok(Err(error))) => return Outcome::Failed(format!("{error:#}")),
            Ok(Ok(Ok(_))) => {}
        }
        let Some(file) = choose_file(&handle, item) else {
            return Outcome::Failed("There is no video in this release.".into());
        };
        let _ = self
            .store
            .set_file_index(&item.id, file.index as u32, file.length);

        if let Some(outcome) = self.wait_for_space(item, file.length).await {
            return outcome;
        }
        let chosen: HashSet<usize> = HashSet::from([file.index]);
        if let Err(error) = self.session.update_only_files(&handle, &chosen).await {
            return Outcome::Failed(format!("{error:#}"));
        }
        if handle.is_paused()
            && let Err(error) = self.session.unpause(&handle).await
        {
            return Outcome::Failed(format!("{error:#}"));
        }
        self.watch(&handle, item, &file).await
    }

    /// Fetches a track's audio: finds where it is, then reads it into the staging folder.
    async fn fetch_audio(&self, item: &Item) -> Outcome {
        let Some(music) = &self.music else {
            return Outcome::Failed("Music cannot be saved here.".into());
        };
        self.note(&item.id, "Finding the track");
        let found = match music.resolve(&item.media_id).await {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!("[library] no audio for {}: {error:#}", item.media_id);
                return Outcome::Failed("Could not find audio for this track.".into());
            }
        };
        self.fetch_url(item, &found.url, &found.headers).await
    }

    /// Reads the file at `url` into the staging folder, reporting progress, until it ends or the
    /// user stops it.
    async fn fetch_url(&self, item: &Item, url: &str, headers: &[(String, String)]) -> Outcome {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        let mut request = client.get(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let mut response = match request
            .send()
            .await
            .and_then(|response| response.error_for_status())
        {
            Ok(response) => response,
            Err(error) => {
                return Outcome::Failed(format!("The audio could not be fetched: {error}"));
            }
        };
        let total = response.content_length().unwrap_or(0);
        if let Some(outcome) = self.wait_for_space(item, total.max(16 * 1024 * 1024)).await {
            return outcome;
        }
        let dir = self.root.join("staging").join("music");
        let path = dir.join(saved_name(&item.id).file_name().unwrap_or_default());
        let written = async {
            std::fs::create_dir_all(&dir)?;
            let mut file = std::fs::File::create(&path)?;
            let mut done = 0u64;
            let started = Instant::now();
            // A connection that goes quiet ends the fetch, which a pause or removal could not otherwise reach.
            while let Some(chunk) = tokio::time::timeout(AUDIO_QUIET_FOR, response.chunk())
                .await
                .map_err(|_| anyhow::anyhow!("the audio stopped arriving"))??
            {
                match self.requested(&item.id) {
                    Some(Stop::Cancel) => return Ok(Outcome::Cancelled),
                    // A track is a few megabytes: stopping it drops what was read, and it starts over.
                    Some(Stop::Pause) => return Ok(Outcome::Paused),
                    None => {}
                }
                std::io::Write::write_all(&mut file, &chunk)?;
                done += chunk.len() as u64;
                let seconds = started.elapsed().as_secs_f64().max(0.001);
                self.progress.lock().unwrap().insert(
                    item.id.clone(),
                    Progress {
                        done,
                        total: total.max(done),
                        speed: (done as f64 / seconds) as u64,
                        peers: 1,
                        note: "Downloading".into(),
                    },
                );
            }
            file.sync_all()?;
            Ok::<Outcome, anyhow::Error>(Outcome::Fetched(path.clone(), done))
        }
        .await;
        match written {
            Ok(Outcome::Fetched(path, length)) if length > 0 => Outcome::Fetched(path, length),
            Ok(Outcome::Fetched(..)) => {
                let _ = std::fs::remove_file(&path);
                Outcome::Failed("The audio was empty.".into())
            }
            Ok(other) => {
                let _ = std::fs::remove_file(&path);
                other
            }
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                Outcome::Failed(format!("The audio could not be fetched: {error:#}"))
            }
        }
    }

    /// Waits until there is room for the file and its sealed copy. None when there is.
    async fn wait_for_space(&self, item: &Item, length: u64) -> Option<Outcome> {
        let needed = length.saturating_mul(2).saturating_add(SPARE_BYTES);
        loop {
            let free = free_space(&self.root);
            if free >= needed {
                return None;
            }
            self.note(&item.id, "Waiting for space");
            match self.requested_within(&item.id, SPACE_EVERY).await {
                Some(Stop::Pause) => return Some(Outcome::Paused),
                Some(Stop::Cancel) => return Some(Outcome::Cancelled),
                None => {}
            }
        }
    }

    /// Runs `work`, but gives up as soon as the user pauses or removes the item.
    async fn unless_stopped<T>(
        &self,
        id: &str,
        work: impl std::future::Future<Output = T>,
    ) -> Result<T, Stop> {
        tokio::pin!(work);
        loop {
            tokio::select! {
                done = &mut work => return Ok(done),
                _ = tokio::time::sleep(LOOK_EVERY) => {
                    if let Some(stop) = self.requested(id) {
                        return Err(stop);
                    }
                }
            }
        }
    }

    /// Waits up to `time` for a pause or removal request.
    async fn requested_within(&self, id: &str, time: Duration) -> Option<Stop> {
        let until = Instant::now() + time;
        while Instant::now() < until {
            if let Some(stop) = self.requested(id) {
                return Some(stop);
            }
            tokio::time::sleep(LOOK_EVERY.min(time)).await;
        }
        self.requested(id)
    }

    /// Reads progress until the file is complete or the user stops it.
    async fn watch(&self, handle: &Arc<ManagedTorrent>, item: &Item, file: &VideoFile) -> Outcome {
        let mut last_bytes = 0;
        let mut last_change = Instant::now();
        loop {
            match self.requested(&item.id) {
                Some(Stop::Pause) => {
                    let _ = self.session.pause(handle).await;
                    return Outcome::Paused;
                }
                Some(Stop::Cancel) => return Outcome::Cancelled,
                None => {}
            }
            let stats = handle.stats();
            if let Some(error) = &stats.error {
                return Outcome::Failed(error.clone());
            }
            let done = stats.file_progress.get(file.index).copied().unwrap_or(0);
            if done != last_bytes {
                last_bytes = done;
                last_change = Instant::now();
            }
            let (speed, peers) = stats.live.as_ref().map_or((0, 0), |live| {
                (
                    live.download_speed.as_bytes(),
                    live.snapshot.peer_stats.live,
                )
            });
            let note = if done >= file.length {
                "Checking"
            } else if last_change.elapsed() >= self.timing.stalled {
                "Nothing is arriving right now"
            } else if peers == 0 {
                "Looking for peers"
            } else {
                "Downloading"
            };
            self.progress.lock().unwrap().insert(
                item.id.clone(),
                Progress {
                    done,
                    total: file.length,
                    speed,
                    peers,
                    note: note.into(),
                },
            );
            if done >= file.length {
                return match staged_path(handle, file) {
                    Some(path) => Outcome::Fetched(path, file.length),
                    None => Outcome::Failed("The fetched file could not be found.".into()),
                };
            }
            tokio::time::sleep(LOOK_EVERY).await;
        }
    }

    /// Works on one item from start to finish and records how it ended.
    async fn work(&self, item: Item) {
        let id = item.id.clone();
        self.progress
            .lock()
            .unwrap()
            .insert(id.clone(), Progress::default());
        // A request meant for an earlier item must not stop this one.
        self.requested(&id);
        let _ = self.store.set_state(&id, State::Active, None);
        let mut outcome = match item.kind {
            Kind::Video => self.fetch(&item).await,
            Kind::Music => self.fetch_audio(&item).await,
        };
        // Removed while it was finishing, or while its row was being read: nothing is sealed.
        if matches!(outcome, Outcome::Fetched(..)) {
            let removed = self.store.item(&id).ok().flatten().is_none();
            if removed || matches!(self.requested(&id), Some(Stop::Cancel)) {
                if let Outcome::Fetched(path, _) = &outcome {
                    let _ = std::fs::remove_file(path);
                }
                outcome = Outcome::Cancelled;
            }
        }
        let result = match outcome {
            Outcome::Paused => self.store.set_state(&id, State::Paused, None),
            Outcome::Cancelled => self.discard(&id).await,
            Outcome::Failed(why) => {
                self.drop_torrent_of(&item).await;
                self.store.set_state(&id, State::Failed, Some(&why))
            }
            Outcome::Fetched(path, length) => self.seal(&item, &path, length).await,
        };
        if let Err(error) = result {
            tracing::warn!("[library] could not record how {id} ended: {error:#}");
        }
        self.progress.lock().unwrap().remove(&id);
    }

    /// Fetches the subtitles chosen for an item and keeps them beside it. One that cannot be fetched
    /// is skipped: the film does not wait on it, and it has its own subtitles inside as well.
    async fn save_subtitles(&self, item: &Item) {
        let Ok(pending) = self.store.pending_subtitles(&item.id) else {
            return;
        };
        let dir = subtitle_dir(&item.id);
        for (index, subtitle) in pending.iter().enumerate() {
            let text = match akidev_core::subtitles::fetch(&subtitle.url).await {
                Ok(text) => text,
                Err(error) => {
                    tracing::warn!("[library] subtitle {} not saved: {error:#}", subtitle.url);
                    continue;
                }
            };
            let file = dir.join(format!("{index}-{}.vtt", safe_name(&subtitle.lang)));
            let written = std::fs::create_dir_all(self.root.join(&dir))
                .and_then(|()| std::fs::write(self.root.join(&file), text));
            if written.is_ok() {
                let _ =
                    self.store
                        .set_subtitle_file(&item.id, &subtitle.url, &file.to_string_lossy());
            }
        }
    }

    /// Locks the fetched file into its saved file, then deletes the plain copy and the torrent.
    async fn seal(&self, item: &Item, plain: &Path, length: u64) -> anyhow::Result<()> {
        self.store.set_state(&item.id, State::Sealing, None)?;
        self.note(&item.id, "Locking it into the library");
        let relative = saved_name(&item.id);
        let (root, plain_owned, master) =
            (self.root.clone(), plain.to_path_buf(), self.master.clone());
        let target = relative.clone();
        let sealed = tokio::task::spawn_blocking(move || {
            let output = root.join(&target);
            std::fs::create_dir_all(output.parent().context("no folder")?)?;
            container::seal(&plain_owned, &output, &master()?)
        })
        .await?;
        // The subtitles are kept before the item counts as saved, so it plays with them from the start.
        if sealed.is_ok() {
            self.save_subtitles(item).await;
        }
        let saved = sealed.and_then(|_| {
            self.store
                .set_saved(&item.id, &relative.to_string_lossy(), length)
        });
        match saved {
            Ok(()) => {
                self.finished
                    .lock()
                    .unwrap()
                    .push(if item.subtitle.is_empty() {
                        item.title.clone()
                    } else {
                        format!("{}: {}", item.title, item.subtitle)
                    });
            }
            Err(error) => {
                tracing::warn!("[library] sealing failed: {error:#}");
                // Nothing points at a half-made or unrecorded file.
                let _ = std::fs::remove_file(self.root.join(&relative));
                self.store
                    .set_state(&item.id, State::Failed, Some("It could not be saved."))?;
            }
        }
        // The plain copy goes either way; the torrent goes unless another episode of its pack waits.
        let _ = std::fs::remove_file(plain);
        self.drop_torrent_of(item).await;
        Ok(())
    }
}

/// The long-running part: take the next queued item, work on it, repeat; sleep when none is left.
async fn run(library: Arc<Library>) {
    loop {
        match library.next() {
            Ok(Some(item)) => library.work(item).await,
            Ok(None) => library.wake.notified().await,
            Err(error) => {
                tracing::warn!("[library] could not read the queue: {error:#}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Every file under `dir`, however deep.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// The file of the torrent to save: the one chosen last time, else the best match for the episode.
fn choose_file(handle: &Arc<ManagedTorrent>, item: &Item) -> Option<VideoFile> {
    let files: Vec<VideoFile> = handle
        .with_metadata(|metadata| {
            metadata
                .file_infos
                .iter()
                .enumerate()
                .map(|(index, file)| VideoFile {
                    index,
                    name: file.relative_filename.to_string_lossy().into_owned(),
                    length: file.len,
                })
                .collect()
        })
        .ok()?;
    if let Some(index) = item.file_index
        && let Some(file) = files.iter().find(|file| file.index == index as usize)
    {
        return Some(file.clone());
    }
    pick_video_file(&files, item.episode, MIN_VIDEO_BYTES, item.season).cloned()
}

/// Where a fetched file is on disk: in the torrent's folder, under its own path.
fn staged_path(handle: &Arc<ManagedTorrent>, file: &VideoFile) -> Option<PathBuf> {
    let direct = handle.output_folder().join(&file.name);
    if direct.is_file() {
        return Some(direct);
    }
    let named = handle.name()?;
    let inside = handle.output_folder().join(named).join(&file.name);
    inside.is_file().then_some(inside)
}

/// Where an item's subtitles are kept: a folder named by the hash of the item's id.
fn subtitle_dir(id: &str) -> PathBuf {
    let saved = saved_name(id);
    let name = saved.file_stem().unwrap_or_default().to_owned();
    PathBuf::from("subtitles").join(name)
}

/// A language code made safe to use in a file name.
fn safe_name(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The saved file's name, under the library folder: the hash of the item's id, so nothing in it
/// says what it is.
fn saved_name(id: &str) -> PathBuf {
    let digest = Sha256::digest(id.as_bytes());
    let hex: String = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    PathBuf::from("files")
        .join(&hex[..2])
        .join(format!("{hex}.akc"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::Sealed;
    use crate::store::Kind;
    use librqbit::spawn_utils::BlockingSpawner;
    use librqbit::{CreateTorrentOptions, ListenerOptions, create_torrent};

    const MIB: usize = 1024 * 1024;

    /// A file of bytes that do not compress, different for each seed.
    fn noise(seed: u64, bytes: usize) -> Vec<u8> {
        let mut state = seed | 1;
        (0..bytes)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("akidev-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn local_session(dir: &Path, listen: bool, upload_bps: Option<u32>) -> Arc<Session> {
        Session::new_with_opts(
            dir.to_path_buf(),
            SessionOptions {
                dht: None,
                disable_trackers: true,
                persistence: None,
                fastresume: false,
                listen: listen.then(|| ListenerOptions {
                    listen_addr: "127.0.0.1:0".parse().unwrap(),
                    ..Default::default()
                }),
                ratelimits: librqbit::limits::LimitsConfig {
                    upload_bps: upload_bps.and_then(std::num::NonZeroU32::new),
                    download_bps: None,
                },
                ..Default::default()
            },
        )
        .await
        .unwrap()
    }

    /// A season pack of three 6 MiB episodes, seeded from this machine. Returns the seeder, the
    /// pack's info hash, and the episodes' bytes.
    async fn seeded_pack(
        dir: &Path,
        upload_bps: Option<u32>,
    ) -> (Arc<Session>, String, Vec<Vec<u8>>) {
        let pack = dir.join("Show.S01");
        std::fs::create_dir_all(&pack).unwrap();
        let episodes: Vec<Vec<u8>> = (1..=3).map(|n| noise(n * 7919, 6 * MIB)).collect();
        for (index, bytes) in episodes.iter().enumerate() {
            std::fs::write(pack.join(format!("Show - 0{}.mkv", index + 1)), bytes).unwrap();
        }
        let torrent = create_torrent(
            &pack,
            CreateTorrentOptions {
                name: None,
                trackers: Vec::new(),
                piece_length: Some(256 * 1024),
            },
            &BlockingSpawner::new(2),
        )
        .await
        .unwrap();
        let seeder = local_session(dir, true, upload_bps).await;
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
        (seeder, torrent.info_hash().as_string(), episodes)
    }

    fn episode_item(hash: &str, episode: u32) -> Item {
        let media = format!("series-tt1-s1e{episode}");
        let mut item = Item::new(
            Item::video_id(&media, hash),
            Kind::Video,
            media,
            "Show".into(),
        );
        item.release = Some(hash.into());
        item.magnet = Some(format!("magnet:?xt=urn:btih:{hash}"));
        item.season = Some(1);
        item.episode = Some(episode);
        item
    }

    async fn library_in(
        root: &Path,
        seeder: Option<&Arc<Session>>,
        timing: Timing,
    ) -> Arc<Library> {
        let session = local_session(&root.join("staging"), false, None).await;
        let peers = seeder
            .map(|seeder| vec![seeder.listen_addr().unwrap()])
            .unwrap_or_default();
        Library::start(
            root,
            Arc::new(Store::open(root).unwrap()),
            Arc::new(|| Ok(MasterKey([5; 32]))),
            None,
            session,
            Fetching {
                timing,
                peers,
                trackers: Vec::new(),
            },
        )
        .await
        .unwrap()
    }

    fn quick() -> Timing {
        Timing {
            metadata: Duration::from_secs(20),
            stalled: Duration::from_secs(60),
        }
    }

    async fn until(library: &Library, id: &str, state: State) -> Item {
        for _ in 0..240 {
            let item = library.store().item(id).unwrap();
            if let Some(item) = item.filter(|item| item.state == state) {
                return item;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        panic!(
            "{id} never reached {state:?}: {:?}",
            library.store().item(id).unwrap()
        );
    }

    /// The saved file of a ready item, opened with the key these tests use.
    fn saved_file(library: &Library, id: &str) -> Sealed {
        let item = library.store().item(id).unwrap().unwrap();
        Sealed::open(&library.root.join(item.file.unwrap()), &MasterKey([5; 32])).unwrap()
    }

    fn read_all(sealed: &Sealed) -> Vec<u8> {
        let mut bytes = vec![0u8; sealed.len() as usize];
        assert_eq!(sealed.read_at(0, &mut bytes).unwrap(), bytes.len());
        bytes
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_episode_of_a_pack_is_fetched_sealed_and_nothing_else_is_kept() {
        let base = scratch("pack");
        let (seeder, hash, episodes) = seeded_pack(&base.join("seed"), None).await;
        let root = base.join("library");
        let library = library_in(&root, Some(&seeder), quick()).await;

        let item = episode_item(&hash, 2);
        assert!(library.add(&item).unwrap());
        let saved = until(&library, &item.id, State::Ready).await;

        // The second file of the pack, byte for byte, and no other.
        assert_eq!(saved.file_index, Some(1));
        assert_eq!(saved.bytes, episodes[1].len() as u64);
        assert_eq!(read_all(&saved_file(&library, &item.id)), episodes[1]);
        // The saved file is the only one left: no plain video, no torrent, nothing shared.
        let kept: Vec<PathBuf> = files_under(&root)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "mkv"))
            .collect();
        assert!(kept.is_empty(), "plain files left behind: {kept:?}");
        assert_eq!(
            library.session.with_torrents(|torrents| torrents.count()),
            0
        );
        assert!(library.progress(&item.id).is_none());
        // Adding it again changes nothing.
        assert!(!library.add(&item).unwrap());
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_download_can_be_paused_and_carries_on_where_it_was() {
        let base = scratch("pause");
        // About 1.5 MB a second, so six megabytes take several seconds.
        let (seeder, hash, episodes) = seeded_pack(&base.join("seed"), Some(1_500_000)).await;
        let library = library_in(&base.join("library"), Some(&seeder), quick()).await;
        let item = episode_item(&hash, 3);
        library.add(&item).unwrap();

        for _ in 0..120 {
            if library
                .progress(&item.id)
                .is_some_and(|progress| progress.done > 0)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        library.pause(&item.id).unwrap();
        let paused = until(&library, &item.id, State::Paused).await;
        assert!(paused.file_index.is_some());
        // Still paused a moment later: nothing restarts it.
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            library.store().item(&item.id).unwrap().unwrap().state,
            State::Paused
        );

        library.resume(&item.id).unwrap();
        until(&library, &item.id, State::Ready).await;
        assert_eq!(read_all(&saved_file(&library, &item.id)), episodes[2]);
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn removing_a_download_in_progress_deletes_everything_about_it() {
        let base = scratch("cancel");
        let (seeder, hash, _) = seeded_pack(&base.join("seed"), Some(1_000_000)).await;
        let root = base.join("library");
        let library = library_in(&root, Some(&seeder), quick()).await;
        let item = episode_item(&hash, 1);
        library.add(&item).unwrap();
        for _ in 0..120 {
            if library
                .progress(&item.id)
                .is_some_and(|progress| progress.done > 0)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        library.remove(&item.id).await.unwrap();
        for _ in 0..80 {
            if library.store().item(&item.id).unwrap().is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(library.store().item(&item.id).unwrap().is_none());
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            library.session.with_torrents(|torrents| torrents.count()),
            0
        );
        let kept: Vec<PathBuf> = files_under(&root)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "mkv"))
            .collect();
        assert!(kept.is_empty(), "plain files left behind: {kept:?}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn nobody_sharing_it_fails_with_a_reason_and_can_be_tried_again() {
        let base = scratch("nobody");
        let library = library_in(
            &base.join("library"),
            None,
            Timing {
                metadata: Duration::from_secs(2),
                stalled: Duration::from_secs(60),
            },
        )
        .await;
        let item = episode_item("0123456789abcdef0123456789abcdef01234567", 1);
        library.add(&item).unwrap();
        let failed = until(&library, &item.id, State::Failed).await;
        assert_eq!(
            failed.note.as_deref(),
            Some("No one is sharing this right now.")
        );
        library.resume(&item.id).unwrap();
        assert!(matches!(
            library.store().item(&item.id).unwrap().unwrap().state,
            State::Queued | State::Active
        ));
        // A removal of an item that is not being fetched is immediate.
        let other = episode_item("fedcba9876543210fedcba9876543210fedcba98", 2);
        library.store().add(&other).unwrap();
        library.remove(&other.id).await.unwrap();
        assert!(library.store().item(&other.id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn what_was_in_progress_when_the_app_closed_is_fetched_again() {
        let base = scratch("restart");
        let (seeder, hash, episodes) = seeded_pack(&base.join("seed"), None).await;
        let root = base.join("library");
        let item = episode_item(&hash, 1);
        {
            let store = Store::open(&root).unwrap();
            store.add(&item).unwrap();
            store.set_state(&item.id, State::Active, None).unwrap();
        }
        let library = library_in(&root, Some(&seeder), quick()).await;
        until(&library, &item.id, State::Ready).await;
        assert_eq!(read_all(&saved_file(&library, &item.id)), episodes[0]);
        let _ = std::fs::remove_dir_all(base);
    }

    /// Serves `body` to anyone who asks, for as long as the test runs, and returns its address.
    async fn serve_audio(body: Vec<u8>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let body = body.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut request = [0u8; 2048];
                    let _ = socket.read(&mut request).await;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: audio/mp4\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });
        format!("http://{address}/track.m4a")
    }

    fn track_item(id: &str) -> Item {
        let mut item = Item::new(
            id.to_string(),
            Kind::Music,
            id.to_string(),
            "A track".into(),
        );
        item.subtitle = "An artist".into();
        item
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_track_is_read_sealed_and_the_plain_file_is_deleted() {
        let base = scratch("track");
        let root = base.join("library");
        let library = library_in(&root, None, quick()).await;
        let audio = noise(11, 3 * MIB + 17);
        let url = serve_audio(audio.clone()).await;
        let item = track_item("dQw4w9WgXcQ");
        library.store().add(&item).unwrap();

        let Outcome::Fetched(path, length) = library.fetch_url(&item, &url, &[]).await else {
            panic!("the track was not fetched");
        };
        assert_eq!(length, audio.len() as u64);
        library.seal(&item, &path, length).await.unwrap();

        assert_eq!(read_all(&saved_file(&library, &item.id)), audio);
        assert!(!path.exists(), "the plain track is still on disk");
        assert_eq!(library.take_finished(), ["A track: An artist"]);
        assert!(library.take_finished().is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_track_that_cannot_be_fetched_fails_with_a_reason_and_leaves_nothing() {
        let base = scratch("track-gone");
        let root = base.join("library");
        let library = library_in(&root, None, quick()).await;
        let item = track_item("gone");
        library.store().add(&item).unwrap();
        // Nothing is listening here.
        let outcome = library
            .fetch_url(&item, "http://127.0.0.1:1/nothing", &[])
            .await;
        assert!(matches!(outcome, Outcome::Failed(why) if why.contains("could not be fetched")));
        // Music with no way to find audio says so instead of waiting.
        let other = track_item("other");
        library.add(&other).unwrap();
        let failed = until(&library, &other.id, State::Failed).await;
        assert_eq!(failed.note.as_deref(), Some("Music cannot be saved here."));
        assert!(
            files_under(&root)
                .iter()
                .all(|path| !path.starts_with(root.join("staging").join("music")))
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_packs_torrent_is_kept_while_another_of_its_episodes_waits() {
        let base = scratch("shared");
        let library = library_in(&base.join("library"), None, quick()).await;
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let (one, two) = (episode_item(hash, 1), episode_item(hash, 2));
        library.store().add(&one).unwrap();
        library.store().add(&two).unwrap();
        library
            .store()
            .set_state(&one.id, State::Ready, None)
            .unwrap();
        library
            .store()
            .set_state(&two.id, State::Paused, None)
            .unwrap();
        // The episode that is saved does not need the torrent; the one waiting does.
        assert!(library.others_need(&one.id, hash));
        assert!(!library.others_need(&two.id, hash));
        assert!(!library.others_need(&one.id, "fedcba9876543210fedcba9876543210fedcba98"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn asking_again_for_one_that_failed_tries_it_again() {
        let base = scratch("again");
        let library = library_in(
            &base.join("library"),
            None,
            Timing {
                metadata: Duration::from_secs(1),
                stalled: Duration::from_secs(60),
            },
        )
        .await;
        let item = episode_item("0123456789abcdef0123456789abcdef01234567", 1);
        assert!(library.add(&item).unwrap());
        until(&library, &item.id, State::Failed).await;
        // The download button adds it again: that is a retry, not "already there".
        assert!(library.add(&item).unwrap());
        let state = library.store().item(&item.id).unwrap().unwrap().state;
        assert!(matches!(
            state,
            State::Queued | State::Active | State::Failed
        ));
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn saved_files_nothing_points_at_are_swept_when_the_library_opens() {
        let base = scratch("sweep");
        let root = base.join("library");
        let stray = root.join("files").join("ab").join("left-behind.akc.part");
        std::fs::create_dir_all(stray.parent().unwrap()).unwrap();
        std::fs::write(&stray, b"half").unwrap();
        let kept = root.join("files").join("cd").join("kept.akc");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, b"whole").unwrap();
        let store = Store::open(&root).unwrap();
        let mut item = episode_item("0123456789abcdef0123456789abcdef01234567", 1);
        item.file = Some("files/cd/kept.akc".into());
        item.state = State::Ready;
        store.add(&item).unwrap();
        drop(store);
        let _library = library_in(&root, None, quick()).await;
        assert!(!stray.exists());
        assert!(kept.exists());
        let _ = std::fs::remove_dir_all(base);
    }
}
