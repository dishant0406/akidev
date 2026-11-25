//! The torrent engine: joins a swarm, picks the file to play and hands out its bytes.
//!
//! This streams: it does not download the file. No file is ever selected, so the only
//! pieces fetched are the ones a reader asks for and a window ahead of where it is
//! reading. Selecting the file made the engine fetch every remaining piece at full speed
//! behind the playhead, which is downloading with playback on top. What has been read
//! stays on disk until the torrent is evicted or the app quits. Disk and sockets are
//! bounded by evicting the oldest torrent.

use crate::config::Config;
use crate::model::Release;
use crate::releases::{
    MIN_VIDEO_BYTES, VideoFile, pick_selected_video_file_for_release, pick_video_file_for_release,
    release_title_name,
};
use anyhow::{Context, bail};
use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

type Handle = Arc<ManagedTorrent>;
type Lifecycle = tokio::sync::OwnedMutexGuard<()>;

/// Weak entries release completed requests; the cap bounds concurrent untrusted hashes.
#[derive(Default)]
struct HashLocks(Mutex<HashMap<Id20, Weak<tokio::sync::Mutex<()>>>>);

impl HashLocks {
    fn get(&self, hash: Id20) -> anyhow::Result<Arc<tokio::sync::Mutex<()>>> {
        let mut locks = self.0.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() != 0);
        if let Some(lock) = locks.get(&hash).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        if locks.len() >= 1024 {
            bail!("too many concurrent torrent operations");
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(hash, Arc::downgrade(&lock));
        Ok(lock)
    }
}

/// A readable, seekable view of the chosen file.
pub trait FileReader: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> FileReader for T {}

/// The file chosen out of a torrent. A pack holds many episodes, so the hash alone does
/// not name what plays: the file index does.
#[derive(Debug, Clone, PartialEq)]
pub struct Playable {
    pub info_hash: String,
    pub file_index: usize,
    pub file_name: String,
    pub length: u64,
}

/// One playable file. Several entries share a torrent when a pack serves more than one
/// of its episodes.
struct Entry {
    playable: Playable,
    torrent_id: usize,
    handle: Handle,
    last_used: Instant,
    /// How many responses are reading this file right now.
    readers: Arc<AtomicUsize>,
    /// A stream held open for as long as the file is playable. librqbit disconnects from
    /// the peers it no longer needs once nothing is being read, and then waits on a slow
    /// reconnect for the next read, which is what a seek is. An open stream keeps them.
    _hold: Option<Box<dyn FileReader>>,
}

/// Held while a response reads from a torrent, so eviction leaves it alone.
pub struct Reader(Arc<AtomicUsize>);

impl Drop for Reader {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Deletes a torrent from the session when dropped, unless disarmed. `open` is dropped
/// when the viewer leaves the watch page, and without this a torrent added by then
/// would stay in the session, downloading and holding a slot, with no entry to evict it.
struct AddedTorrent {
    session: Arc<Session>,
    torrent_id: Option<usize>,
    lifecycle: Option<Lifecycle>,
}

impl AddedTorrent {
    fn disarm(mut self) {
        self.torrent_id = None;
    }

    fn delete_task(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        let id = self.torrent_id.take()?;
        let session = self.session.clone();
        let lifecycle = self.lifecycle.take();
        let runtime = tokio::runtime::Handle::try_current().ok()?;
        Some(runtime.spawn(async move {
            // Deletion retains the hash lock even when its caller is cancelled.
            let _lifecycle = lifecycle;
            if let Err(error) = session.delete(TorrentIdOrHash::Id(id), true).await {
                tracing::debug!("[torrent] delete failed: {error:#}");
            }
        }))
    }

    async fn cleanup(mut self) {
        if let Some(deleted) = self.delete_task() {
            let _ = deleted.await;
        }
    }
}

impl Drop for AddedTorrent {
    fn drop(&mut self) {
        self.delete_task();
    }
}

pub struct Engine {
    session: Arc<Session>,
    max_torrents: usize,
    max_cache_bytes: u64,
    metadata_timeout: Duration,
    active: Mutex<Vec<Entry>>,
    hash_locks: HashLocks,
}

/// How a stream is going, for the "buffering" readout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamStats {
    pub downloaded_bytes: u64,
    pub bytes_per_second: u64,
    pub peers: u32,
}

/// How much of each end of the file is fetched as soon as it is chosen. Players read the
/// start, and look for an index at the end, before they show a frame. Asking for both up
/// front makes those reads local.
const WARM_UP_BYTES: u64 = 3 * 1024 * 1024;
/// Maximum time to deliver the start of the file. A swarm that cannot deliver it is
/// not ready for playback and must not be committed as playable.
const HEAD_WAIT: Duration = Duration::from_secs(15);

/// The directory the engine owns inside the user's chosen one. It is the only place
/// the engine deletes from, so a `cache_dir` that holds other files is never wiped.
const DOWNLOADS_DIR: &str = "akidev-torrents";

impl Engine {
    /// Where downloads live for a given base directory.
    pub fn downloads_dir(base: &Path) -> PathBuf {
        base.join(DOWNLOADS_DIR)
    }

    /// Starts a session whose downloads live under `downloads_dir(base)`. That
    /// directory is emptied first: it holds scratch data from a previous run, and
    /// deleting it costs nothing but a re-download.
    pub async fn start(config: &Config, base: &Path) -> anyhow::Result<Engine> {
        let dir = Engine::downloads_dir(base);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        // With a port, peers can connect in as well as being connected to, and the trackers and
        // the DHT are told that port. The DHT shares the number over UDP.
        let options = |dht: Option<librqbit::DhtSessionConfig>, port: Option<u16>| SessionOptions {
            dht,
            persistence: None,
            fastresume: false,
            listen: port.map(|port| librqbit::ListenerOptions {
                listen_addr: (std::net::Ipv6Addr::UNSPECIFIED, port).into(),
                announce_port: (port != 0).then_some(port),
                ..Default::default()
            }),
            ratelimits: librqbit::limits::LimitsConfig {
                upload_bps: config.upload_limit_bps.and_then(std::num::NonZeroU32::new),
                download_bps: None,
            },
            ..Default::default()
        };
        let dht_on = |port: Option<u16>| librqbit::DhtSessionConfig {
            port,
            persistence: None,
            ..Default::default()
        };
        // The DHT remembers the port it used and binds it again, in a file every copy of the
        // library on the machine shares. A second window, or the server beside the app on a
        // developer's machine, then fails to start with "address already in use", and nothing
        // can be played. When the wanted port is taken the engine takes any free one instead.
        let first = match config.listen_port {
            Some(port) => {
                Session::new_with_opts(dir.clone(), options(Some(dht_on(Some(port))), Some(port)))
                    .await
            }
            None => {
                Session::new_with_opts(dir.clone(), options(Some(Default::default()), None)).await
            }
        };
        let session = match first {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!("[torrent] the ports could not be used as asked: {error:#}");
                let port = config.listen_port.map(|_| 0);
                Session::new_with_opts(dir, options(Some(dht_on(None)), port)).await?
            }
        };
        tracing::info!(
            "[torrent] listening on {:?}, upload limit {:?}",
            session.listen_addr(),
            config.upload_limit_bps
        );
        Ok(Engine::new(session, config))
    }

    fn new(session: Arc<Session>, config: &Config) -> Engine {
        Engine {
            session,
            max_torrents: config.max_torrents.max(1),
            max_cache_bytes: config.max_cache_bytes,
            metadata_timeout: Duration::from_millis(config.metadata_timeout_ms),
            active: Mutex::new(Vec::new()),
            hash_locks: HashLocks::default(),
        }
    }

    /// Joins the release's swarm and chooses the file to play.
    ///
    /// `budget` is what is left of the caller's overall deadline: waiting for peers is
    /// clamped to it, so one slow swarm cannot push the whole request past its limit.
    /// `fresh` drops idle cached data for this release first, so a retry after a dead
    /// swarm does not get the same dead torrent back. It is rejected while any file in
    /// the torrent has a response reader. Several opens may run at once, which
    /// is how the resolver races its candidates.
    pub async fn open(
        &self,
        release: &Release,
        episode: Option<u32>,
        season: Option<u32>,
        budget: Duration,
        fresh: bool,
        trackers: &[String],
    ) -> anyhow::Result<Playable> {
        self.open_with_peers(release, episode, season, budget, fresh, trackers, &[], None)
            .await
    }

    /// Opens with catalogue evidence, validating actual filenames before download.
    /// Both aliases and the requested film year must come from the resolved media.
    #[allow(clippy::too_many_arguments)]
    pub async fn open_for_identity(
        &self,
        release: &Release,
        episode: Option<u32>,
        season: Option<u32>,
        budget: Duration,
        fresh: bool,
        trackers: &[String],
        title: &str,
        titles: &[String],
        year: Option<u32>,
    ) -> anyhow::Result<Playable> {
        self.open_with_peers(
            release,
            episode,
            season,
            budget,
            fresh,
            trackers,
            &[],
            Some((title, titles, year)),
        )
        .await
    }

    /// `open`, also trying `peers` directly. Tests use it to reach a seeder on localhost.
    #[allow(clippy::too_many_arguments)]
    async fn open_with_peers(
        &self,
        release: &Release,
        episode: Option<u32>,
        season: Option<u32>,
        budget: Duration,
        fresh: bool,
        trackers: &[String],
        peers: &[SocketAddr],
        identity: Option<(&str, &[String], Option<u32>)>,
    ) -> anyhow::Result<Playable> {
        let deadline = tokio::time::Instant::now()
            .checked_add(budget)
            .context("startup budget is too large")?;
        if budget.is_zero() {
            bail!("timed out opening the torrent");
        }
        // Bound eviction, fresh deletion and cleanup too. Dropping the inner future
        // retains the AddedTorrent guard's cancellation cleanup.
        tokio::time::timeout_at(
            deadline,
            self.open_until(
                release, episode, season, deadline, fresh, trackers, peers, identity,
            ),
        )
        .await
        .context("timed out opening the torrent")?
    }

    #[allow(clippy::too_many_arguments)]
    async fn open_until(
        &self,
        release: &Release,
        episode: Option<u32>,
        season: Option<u32>,
        deadline: tokio::time::Instant,
        fresh: bool,
        trackers: &[String],
        peers: &[SocketAddr],
        identity: Option<(&str, &[String], Option<u32>)>,
    ) -> anyhow::Result<Playable> {
        let hash = Id20::from_str(&release.info_hash).context("bad info hash")?;
        let mut lifecycle = Some(self.hash_locks.get(hash)?.lock_owned().await);
        let started = Instant::now();
        let metadata_deadline = tokio::time::Instant::now()
            .checked_add(self.metadata_timeout)
            .map_or(deadline, |end| deadline.min(end));

        if fresh {
            let dropped = {
                let mut active = self.active.lock().unwrap();
                // A pack shares its session and disk across episodes. Check every
                // sibling under the same lock used to pin response readers, before
                // removing anything or arming deletion.
                if active.iter().any(|entry| {
                    entry.playable.info_hash == release.info_hash
                        && entry.readers.load(Ordering::SeqCst) != 0
                }) {
                    bail!("cannot refresh the torrent while it is in use");
                }
                let before = active.len();
                active.retain(|e| e.playable.info_hash != release.info_hash);
                active.len() != before
            };
            if dropped && let Some(handle) = self.session.get(TorrentIdOrHash::Hash(hash)) {
                AddedTorrent {
                    session: self.session.clone(),
                    torrent_id: Some(handle.id()),
                    lifecycle: lifecycle.take(),
                }
                .cleanup()
                .await;
                lifecycle = Some(self.hash_locks.get(hash)?.lock_owned().await);
            }
        }

        // A pack already in the session is reused: the torrent is added once and only
        // the file choice is redone, because each episode is a different file.
        let existing = self
            .active
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.playable.info_hash == release.info_hash)
            .map(|e| {
                // Pin under the lock shared with eviction, before exposing the handle.
                e.readers.fetch_add(1, Ordering::SeqCst);
                (e.torrent_id, e.handle.clone(), Reader(e.readers.clone()))
            });
        if let Some((torrent_id, handle, _pin)) = existing {
            // Pinned while the new file is chosen, so a racing open cannot evict the
            // torrent underneath it. The sibling entry's stream already keeps the swarm
            // connected, so no further stream is held for this file.
            let (file_index, playable) = tokio::time::timeout_at(
                metadata_deadline,
                self.choose_file(&handle, release, episode, season, identity),
            )
            .await
            .context("timed out choosing a file")??;
            let head_deadline = deadline.min(tokio::time::Instant::now() + HEAD_WAIT);
            fetch_start(&handle, file_index, playable.length, head_deadline).await?;
            return Ok(self.commit(torrent_id, handle, file_index, playable, None));
        }

        // A torrent dropped a moment ago may still be deleting; adding it again now would
        // attach to the one being removed.
        for _ in 0..40 {
            if self.session.get(TorrentIdOrHash::Hash(hash)).is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        self.make_room().await;

        let options = AddTorrentOptions {
            // Nothing is selected: see the module docs.
            only_files: Some(Vec::new()),
            overwrite: true,
            trackers: Some(trackers.to_vec()),
            initial_peers: (!peers.is_empty()).then(|| peers.to_vec()),
            ..Default::default()
        };
        // With persistence disabled librqbit has no suspension between insertion and
        // Added. Metadata timeouts own nothing. Never arm for AlreadyManaged.
        let mut guard = AddedTorrent {
            session: self.session.clone(),
            torrent_id: None,
            lifecycle,
        };
        let added = tokio::time::timeout_at(
            metadata_deadline,
            self.session
                .add_torrent(AddTorrent::from_url(&release.magnet), Some(options)),
        )
        .await;

        let (torrent_id, handle) = match added {
            Ok(Ok(AddTorrentResponse::Added(id, handle))) => {
                guard.torrent_id = Some(id);
                (id, handle)
            }
            Ok(Ok(AddTorrentResponse::AlreadyManaged(id, handle))) => (id, handle),
            Ok(Ok(AddTorrentResponse::ListOnly(_))) => {
                bail!("unexpected list-only response")
            }
            Ok(Err(error)) => {
                bail!("{error:#}")
            }
            Err(_) => {
                // Metadata resolution precedes insertion; nothing owned to delete.
                bail!("timed out waiting for peers");
            }
        };

        let metadata_ms = started.elapsed().as_millis();
        let chosen = match tokio::time::timeout_at(
            metadata_deadline,
            self.choose_file(&handle, release, episode, season, identity),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("timed out reading the file list")),
        };
        match chosen {
            Ok((file_index, playable)) => {
                // Until the swarm has delivered the start of the file, this future can be
                // dropped (a faster candidate won), and the guard then deletes the torrent.
                let head_deadline = deadline.min(tokio::time::Instant::now() + HEAD_WAIT);
                let held = tokio::time::timeout_at(head_deadline, async {
                    let hold = hold_open(&handle, file_index, playable.length).await?;
                    // Parking at EOF keeps peers connected; it is not readiness.
                    fetch_start(&handle, file_index, playable.length, head_deadline).await?;
                    Ok(hold)
                })
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("timed out opening the file")));
                match held {
                    Ok(hold) => {
                        log_open(&handle, release, metadata_ms, started.elapsed().as_millis());
                        let playable =
                            self.commit(torrent_id, handle, file_index, playable, Some(hold));
                        guard.disarm();
                        Ok(playable)
                    }
                    Err(error) => {
                        guard.cleanup().await;
                        Err(error)
                    }
                }
            }
            Err(error) => {
                guard.cleanup().await;
                Err(error)
            }
        }
    }

    /// Records the chosen file as playable, or refreshes it when it already is.
    fn commit(
        &self,
        torrent_id: usize,
        handle: Handle,
        file_index: usize,
        playable: Playable,
        hold: Option<Box<dyn FileReader>>,
    ) -> Playable {
        let mut active = self.active.lock().unwrap();
        if let Some(entry) = active.iter_mut().find(|e| {
            e.playable.info_hash == playable.info_hash && e.playable.file_index == file_index
        }) {
            entry.last_used = Instant::now();
            return entry.playable.clone();
        }
        tokio::spawn(fetch_end(handle.clone(), file_index, playable.length));
        active.push(Entry {
            playable: playable.clone(),
            torrent_id,
            handle,
            last_used: Instant::now(),
            readers: Arc::new(AtomicUsize::new(0)),
            _hold: hold,
        });
        playable
    }

    async fn choose_file(
        &self,
        handle: &Handle,
        release: &Release,
        episode: Option<u32>,
        season: Option<u32>,
        identity: Option<(&str, &[String], Option<u32>)>,
    ) -> anyhow::Result<(usize, Playable)> {
        let files: Vec<VideoFile> = handle.with_metadata(|metadata| {
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
        })?;

        let inferred = release_title_name(&release.title);
        let (title, titles, year) = identity.unwrap_or((&inferred, &[], None));
        let file = if release.file_idx.is_some() {
            pick_selected_video_file_for_release(
                &files,
                episode,
                season,
                &release.title,
                title,
                titles,
                year,
                release.file_idx,
            )
        } else {
            pick_video_file_for_release(
                &files,
                episode,
                MIN_VIDEO_BYTES,
                season,
                &release.title,
                title,
                titles,
                year,
            )
        }
        .context("no matching video file in the release")?;

        handle.wait_until_initialized().await?;
        let name = file
            .name
            .rsplit('/')
            .next()
            .unwrap_or(&file.name)
            .to_string();
        Ok((
            file.index,
            Playable {
                info_hash: release.info_hash.clone(),
                file_index: file.index,
                file_name: name,
                length: file.length,
            },
        ))
    }

    /// Opens a reader at the start of a playable file.
    pub async fn stream(
        &self,
        info_hash: &str,
        file_index: usize,
    ) -> anyhow::Result<(Playable, Box<dyn FileReader>, Reader)> {
        let (handle, playable, readers) = {
            let mut active = self.active.lock().unwrap();
            let entry = active
                .iter_mut()
                .find(|e| e.playable.info_hash == info_hash && e.playable.file_index == file_index)
                .context("this stream has expired")?;
            entry.last_used = Instant::now();
            // Counted under the lock that eviction also takes, so the entry is either
            // pinned here or already gone.
            entry.readers.fetch_add(1, Ordering::SeqCst);
            (
                entry.handle.clone(),
                entry.playable.clone(),
                entry.readers.clone(),
            )
        };
        let reader = Reader(readers);
        let stream = handle.stream(playable.file_index).await?;
        Ok((playable, Box::new(stream), reader))
    }

    /// How many torrents are in the session.
    pub fn active_torrents(&self) -> usize {
        let active = self.active.lock().unwrap();
        let mut ids: Vec<usize> = active.iter().map(|e| e.torrent_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// How a stream is going, or `None` once it is gone.
    pub fn stats(&self, info_hash: &str, file_index: usize) -> Option<StreamStats> {
        let handle = self
            .active
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.playable.info_hash == info_hash && e.playable.file_index == file_index)
            .map(|e| e.handle.clone())?;
        let live = handle.stats().live?;
        Some(StreamStats {
            downloaded_bytes: live.snapshot.downloaded_and_checked_bytes,
            bytes_per_second: live.download_speed.as_bytes(),
            peers: live.snapshot.peer_stats.live,
        })
    }

    /// Frees room for one more torrent. A torrent being read is never dropped: the
    /// limit is allowed to be exceeded until its response ends.
    ///
    /// Both limits are checked because a count alone does not bound disk: a 1080p
    /// episode is roughly 1.4GB.
    async fn make_room(&self) {
        loop {
            // Picking the victim and removing its entries happen under one lock, so a
            // reader cannot start between the "nobody is reading" check and the removal.
            let victim = {
                let mut active = self.active.lock().unwrap();
                let bytes: u64 = active.iter().map(|entry| entry.playable.length).sum();
                let mut torrents: Vec<usize> = active.iter().map(|e| e.torrent_id).collect();
                torrents.sort_unstable();
                torrents.dedup();
                if torrents.len() < self.max_torrents && bytes < self.max_cache_bytes {
                    return;
                }
                // A torrent is idle when none of its files is being read.
                let idle = |id: usize| {
                    active
                        .iter()
                        .filter(|e| e.torrent_id == id)
                        .all(|e| e.readers.load(Ordering::SeqCst) == 0)
                };
                let victim = active
                    .iter()
                    .filter(|entry| idle(entry.torrent_id))
                    .filter_map(|entry| {
                        let hash = Id20::from_str(&entry.playable.info_hash).ok()?;
                        let lifecycle = self.hash_locks.get(hash).ok()?.try_lock_owned().ok()?;
                        Some((entry.torrent_id, entry.last_used, lifecycle))
                    })
                    .min_by_key(|(_, last_used, _)| *last_used)
                    .map(|(id, _, lifecycle)| (id, lifecycle));
                if let Some((torrent_id, _)) = &victim {
                    active.retain(|entry| entry.torrent_id != *torrent_id);
                }
                victim
            };
            let Some((torrent_id, lifecycle)) = victim else {
                return;
            };
            self.remove(torrent_id, lifecycle).await;
        }
    }

    /// Drops every torrent that nobody has read from for `idle`, and its files. A server calls
    /// this on a timer, so an episode watched an hour ago does not hold a slot and its disk.
    pub async fn sweep_idle(&self, idle: Duration) -> usize {
        let victims: Vec<(usize, Lifecycle)> = {
            let mut active = self.active.lock().unwrap();
            let mut torrents: Vec<usize> = active.iter().map(|e| e.torrent_id).collect();
            torrents.sort_unstable();
            torrents.dedup();
            let stale: Vec<(usize, Lifecycle)> = torrents
                .into_iter()
                .filter(|&id| {
                    active.iter().filter(|e| e.torrent_id == id).all(|e| {
                        e.readers.load(Ordering::SeqCst) == 0 && e.last_used.elapsed() >= idle
                    })
                })
                .filter_map(|id| {
                    let entry = active.iter().find(|e| e.torrent_id == id)?;
                    let hash = Id20::from_str(&entry.playable.info_hash).ok()?;
                    let lifecycle = self.hash_locks.get(hash).ok()?.try_lock_owned().ok()?;
                    Some((id, lifecycle))
                })
                .collect();
            active.retain(|entry| !stale.iter().any(|(id, _)| *id == entry.torrent_id));
            stale
        };
        // Arm every removed victim before awaiting: cancellation cleans them all.
        let victims: Vec<_> = victims
            .into_iter()
            .map(|(id, lifecycle)| AddedTorrent {
                session: self.session.clone(),
                torrent_id: Some(id),
                lifecycle: Some(lifecycle),
            })
            .collect();
        let count = victims.len();
        for victim in victims {
            victim.cleanup().await;
        }
        count
    }

    /// Deletes a torrent and its files. The deletion runs as its own task, so it still
    /// completes when the caller is dropped half way, which happens when the viewer
    /// leaves the page that was waiting.
    async fn remove(&self, id: usize, lifecycle: Lifecycle) {
        AddedTorrent {
            session: self.session.clone(),
            torrent_id: Some(id),
            lifecycle: Some(lifecycle),
        }
        .cleanup()
        .await;
    }
}

/// One line per opened torrent: how long the swarm took to give the file list and then the start of
/// the file, and how the peers look at that moment. These are the numbers to read before changing
/// anything about how a torrent is fetched.
fn log_open(handle: &Handle, release: &Release, metadata_ms: u128, head_ms: u128) {
    let Some(live) = handle.stats().live else {
        return;
    };
    let peers = &live.snapshot.peer_stats;
    tracing::info!(
        "[torrent] \"{}\": file list after {metadata_ms}ms, start of the file after {head_ms}ms; \
         peers live {} (tcp {}, utp {}), connecting {}, queued {}, seen {}, dead {}; \
         down {}, up {}, {} bytes uploaded",
        release.title,
        peers.live,
        peers.live_tcp,
        peers.live_utp,
        peers.connecting,
        peers.queued,
        peers.seen,
        peers.dead,
        live.download_speed,
        live.upload_speed,
        live.snapshot.uploaded_bytes,
    );
}

/// Opens a stream parked at the end of the file: it asks for no pieces, and counts as a
/// reader, which keeps the swarm connected.
async fn hold_open(
    handle: &Handle,
    file_index: usize,
    length: u64,
) -> anyhow::Result<Box<dyn FileReader>> {
    let mut stream = handle.clone().stream(file_index).await?;
    stream.seek(std::io::SeekFrom::Start(length)).await?;
    Ok(Box::new(stream))
}

/// Reads `bytes` from `from` in a file, which makes the engine fetch those pieces first.
async fn read_range(
    handle: Handle,
    file_index: usize,
    from: u64,
    bytes: u64,
) -> anyhow::Result<()> {
    let mut stream = handle.stream(file_index).await?;
    stream.seek(std::io::SeekFrom::Start(from)).await?;
    read_bytes(&mut stream, bytes).await
}

/// Reads the entire requested range. Premature EOF is not a successful warmup.
async fn read_bytes<R: AsyncRead + Unpin>(stream: &mut R, bytes: u64) -> anyhow::Result<()> {
    let mut remaining = bytes;
    let mut buffer = vec![0u8; 256 * 1024];
    while remaining > 0 {
        let want = remaining.min(buffer.len() as u64) as usize;
        stream.read_exact(&mut buffer[..want]).await?;
        remaining -= want as u64;
    }
    Ok(())
}

/// Bounds the entire head operation, including opening and seeking its reader.
async fn head_ready<F>(deadline: tokio::time::Instant, read: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    // timeout_at may poll an immediately-ready future after its deadline. Do not
    // allow already-expired startup work to be reported as ready.
    if tokio::time::Instant::now() >= deadline {
        bail!("timed out reading the start of the file");
    }
    tokio::time::timeout_at(deadline, read)
        .await
        .context("timed out reading the start of the file")?
}

/// Readiness requires the whole head range, not just a parked reader or any bytes.
async fn fetch_start(
    handle: &Handle,
    file_index: usize,
    length: u64,
    deadline: tokio::time::Instant,
) -> anyhow::Result<()> {
    head_ready(
        deadline,
        read_range(handle.clone(), file_index, 0, length.min(WARM_UP_BYTES)),
    )
    .await
}

/// Fetches the end of the file in the background: players look there for an index (an MP4's
/// moov, an MKV's cues) before showing a frame. Failure only means the player waits instead.
async fn fetch_end(handle: Handle, file_index: usize, length: u64) {
    let from = length.saturating_sub(WARM_UP_BYTES);
    let _ = tokio::time::timeout(
        Duration::from_secs(60),
        read_range(handle, file_index, from, length - from),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use librqbit::spawn_utils::BlockingSpawner;
    use librqbit::{CreateTorrentOptions, ListenerOptions, create_torrent};

    static SWARM_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// A session that touches no network, holding one tiny torrent that is added from
    /// bytes, so no peers are needed.
    async fn session_with_torrent(name: &str) -> (Arc<Session>, Id20, PathBuf) {
        let dir = std::env::temp_dir().join(format!("akidev-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let session = Session::new_with_opts(
            dir.clone(),
            SessionOptions {
                dht: None,
                disable_trackers: true,
                persistence: None,
                fastresume: false,
                listen: None,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let mut bytes =
            b"d4:infod6:lengthi5e4:name5:a.mkv12:piece lengthi16384e6:pieces20:".to_vec();
        bytes.extend_from_slice(&[7u8; 20]);
        bytes.extend_from_slice(b"ee");
        let response = session
            .add_torrent(
                AddTorrent::from_bytes(bytes),
                Some(AddTorrentOptions {
                    paused: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
        let AddTorrentResponse::Added(_, handle) = response else {
            panic!("expected the torrent to be added");
        };
        (session, handle.info_hash(), dir)
    }

    async fn torrent_count(session: &Session) -> usize {
        session.with_torrents(|torrents| torrents.count())
    }

    #[tokio::test]
    async fn head_read_requires_the_entire_range_and_propagates_errors() {
        let mut complete = std::io::Cursor::new(vec![1u8; 12]);
        read_bytes(&mut complete, 12).await.unwrap();
        let mut short = std::io::Cursor::new(vec![1u8; 11]);
        let error = read_bytes(&mut short, 12).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        let error = head_ready(deadline, async {
            Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "fixture").into())
        })
        .await
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::ConnectionReset
        );
    }

    #[tokio::test]
    async fn head_deadline_is_shared_and_timeout_drops_reader_ownership() {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(40);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let readers = Arc::new(AtomicUsize::new(1));
        let pin = Reader(readers.clone());
        let error = head_ready(deadline, async move {
            let _pin = pin;
            std::future::pending::<anyhow::Result<()>>().await
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert_eq!(readers.load(Ordering::SeqCst), 0);
        // Even immediately-ready work cannot reuse an expired deadline.
        assert!(head_ready(deadline, async { Ok(()) }).await.is_err());
    }

    #[tokio::test]
    async fn public_open_bounds_pre_add_waits_and_zero_budget_does_no_work() {
        let (session, hash, _dir) = session_with_torrent("open-deadline").await;
        let engine = Engine::new(session.clone(), &Config::default());
        let release = Release {
            title: "Movie 2024".into(),
            info_hash: hash.as_string(),
            file_idx: None,
            magnet: format!("magnet:?xt=urn:btih:{}", hash.as_string()),
            size_bytes: None,
            seeders: None,
            source: "test".into(),
            sources: vec!["test".into()],
        };
        // The session already contains this torrent but there is no playable entry.
        // Waiting for deletion used to take two seconds outside every timeout.
        let started = Instant::now();
        let result = engine
            .open(&release, None, None, Duration::from_millis(40), false, &[])
            .await;
        assert!(result.unwrap_err().to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(engine.active_torrents(), 0);
        assert_eq!(torrent_count(&session).await, 1);
        assert!(
            engine
                .open(&release, None, None, Duration::ZERO, true, &[])
                .await
                .is_err()
        );
        assert_eq!(torrent_count(&session).await, 1);
    }

    #[tokio::test]
    async fn dropping_an_armed_guard_deletes_the_torrent() {
        let (session, hash, dir) = session_with_torrent("armed").await;
        assert_eq!(torrent_count(&session).await, 1);
        let locks = HashLocks::default();
        let lock = locks.get(hash).unwrap();
        drop(AddedTorrent {
            session: session.clone(),
            torrent_id: Some(session.get(TorrentIdOrHash::Hash(hash)).unwrap().id()),
            lifecycle: Some(lock.clone().lock_owned().await),
        });
        // The next same-hash operation cannot run until owned cancellation cleanup ends.
        let _next = lock.lock_owned().await;
        assert_eq!(torrent_count(&session).await, 0);
        for _ in 0..50 {
            if torrent_count(&session).await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(torrent_count(&session).await, 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_disarmed_guard_leaves_the_torrent() {
        let (session, hash, dir) = session_with_torrent("disarmed").await;
        AddedTorrent {
            session: session.clone(),
            torrent_id: Some(session.get(TorrentIdOrHash::Hash(hash)).unwrap().id()),
            lifecycle: None,
        }
        .disarm();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(torrent_count(&session).await, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hash_lock_registry_reuses_live_locks_and_prunes_dead_entries() {
        let locks = HashLocks::default();
        let hash = Id20::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
        let first = locks.get(hash).unwrap();
        let second = locks.get(hash).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        drop(first);
        drop(second);
        assert_eq!(locks.0.lock().unwrap().len(), 1);
        let _live = locks
            .get(Id20::from_str("1123456789abcdef0123456789abcdef01234567").unwrap())
            .unwrap();
        assert_eq!(locks.0.lock().unwrap().len(), 1);
    }

    /// A file of incompressible bytes, so nothing about the transfer is shortened.
    fn write_noise(path: &Path, bytes: usize) {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let data: Vec<u8> = (0..bytes)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect();
        std::fs::write(path, data).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn fresh_preserves_a_pinned_sibling_and_replaces_an_idle_pack() {
        let _swarm = SWARM_TEST.lock().await;
        let base = std::env::temp_dir().join(format!("akidev-fresh-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (seed_dir, leech_dir) = (base.join("seed"), base.join("leech"));
        std::fs::create_dir_all(&seed_dir).unwrap();
        std::fs::create_dir_all(&leech_dir).unwrap();
        for episode in 1..=2 {
            write_noise(
                &seed_dir.join(format!("Show.S01E{episode:02}.mkv")),
                8 * 1024 * 1024,
            );
        }
        let local = |dir: &Path, listen: bool| {
            let dir = dir.to_path_buf();
            async move {
                Session::new_with_opts(
                    dir,
                    SessionOptions {
                        dht: None,
                        disable_trackers: true,
                        persistence: None,
                        fastresume: false,
                        listen: listen.then(|| ListenerOptions {
                            listen_addr: "127.0.0.1:0".parse().unwrap(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
            }
        };
        // Multi-file torrent paths include the seed directory's name.
        let seeder = local(&base, true).await;
        let torrent = create_torrent(
            &seed_dir,
            CreateTorrentOptions {
                name: None,
                trackers: Vec::new(),
                piece_length: Some(256 * 1024),
            },
            &BlockingSpawner::new(2),
        )
        .await
        .unwrap();
        let seeding = seeder
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
        tokio::time::timeout(Duration::from_secs(10), seeding.wait_until_completed())
            .await
            .unwrap()
            .unwrap();
        let engine = Engine::new(local(&leech_dir, false).await, &Config::default());
        let hash = torrent.info_hash();
        let release = Release {
            title: "Show S01".into(),
            info_hash: hash.as_string(),
            file_idx: None,
            magnet: format!("magnet:?xt=urn:btih:{}", hash.as_string()),
            size_bytes: None,
            seeders: Some(1),
            source: "test".into(),
            sources: vec!["test".into()],
        };
        let peers = [seeder.listen_addr().unwrap()];
        let open_episode = |episode, fresh| {
            engine.open_with_peers(
                &release,
                Some(episode),
                Some(1),
                Duration::from_secs(30),
                fresh,
                &[],
                &peers,
                None,
            )
        };
        let first = open_episode(1, false).await.unwrap();
        let sibling = open_episode(2, false).await.unwrap();
        assert_ne!(first.file_index, sibling.file_index);
        let original = engine.session.get(TorrentIdOrHash::Hash(hash)).unwrap();
        let (_, mut reader, pin) = engine
            .stream(&release.info_hash, sibling.file_index)
            .await
            .unwrap();
        // The fresh target itself is idle: only its sibling response owns a pin.
        assert_eq!(
            engine
                .active
                .lock()
                .unwrap()
                .iter()
                .find(|entry| { entry.playable.file_index == first.file_index })
                .unwrap()
                .readers
                .load(Ordering::SeqCst),
            0
        );
        let error = open_episode(1, true).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "cannot refresh the torrent while it is in use"
        );
        assert_eq!(engine.active.lock().unwrap().len(), 2);
        assert_eq!(torrent_count(&engine.session).await, 1);
        assert!(Arc::ptr_eq(
            &original,
            &engine.session.get(TorrentIdOrHash::Hash(hash)).unwrap()
        ));
        assert!(
            engine
                .stats(&release.info_hash, sibling.file_index)
                .is_some()
        );
        // The existing response can seek and read actual seeded bytes after rejection.
        let offset = 4 * 1024 * 1024;
        reader.seek(std::io::SeekFrom::Start(offset)).await.unwrap();
        let mut bytes = vec![0; 256 * 1024];
        // Reads real bytes through a local torrent, so a loaded machine needs more than a few seconds.
        tokio::time::timeout(Duration::from_secs(60), reader.read_exact(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        let expected = std::fs::read(seed_dir.join("Show.S01E02.mkv")).unwrap();
        assert_eq!(
            bytes,
            expected[offset as usize..offset as usize + bytes.len()]
        );
        drop(reader);
        drop(pin);

        // Once all responses end, ordinary fresh still replaces the shared torrent.
        assert_eq!(open_episode(1, true).await.unwrap(), first);
        assert_eq!(engine.active.lock().unwrap().len(), 1);
        assert_eq!(torrent_count(&engine.session).await, 1);
        assert!(!Arc::ptr_eq(
            &original,
            &engine.session.get(TorrentIdOrHash::Hash(hash)).unwrap()
        ));
        assert!(
            engine
                .stream(&release.info_hash, sibling.file_index)
                .await
                .is_err()
        );
        drop(engine);
        drop(seeder);
        let _ = std::fs::remove_dir_all(base);
    }

    /// The point of the engine: playing a file must fetch what is read and a window ahead of
    /// it, not the whole file. Selecting the file made a seeder on this machine hand over all
    /// 128 MiB within seconds while only 6 MiB had been read.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn streaming_fetches_a_window_and_not_the_whole_file() {
        let _swarm = SWARM_TEST.lock().await;
        const FILE: usize = 128 * 1024 * 1024;
        const READ: u64 = 6 * 1024 * 1024;
        let base = std::env::temp_dir().join(format!("akidev-stream-{}", std::process::id()));
        let (seed_dir, leech_dir) = (base.join("seed"), base.join("leech"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&seed_dir).unwrap();
        std::fs::create_dir_all(&leech_dir).unwrap();
        write_noise(&seed_dir.join("movie.mkv"), FILE);

        let local = |dir: &Path, listen: bool| {
            let dir = dir.to_path_buf();
            async move {
                Session::new_with_opts(
                    dir,
                    SessionOptions {
                        dht: None,
                        disable_trackers: true,
                        persistence: None,
                        fastresume: false,
                        listen: listen.then(|| ListenerOptions {
                            listen_addr: "127.0.0.1:0".parse().unwrap(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
            }
        };

        let torrent = create_torrent(
            &seed_dir.join("movie.mkv"),
            CreateTorrentOptions {
                name: None,
                trackers: Vec::new(),
                piece_length: Some(256 * 1024),
            },
            &BlockingSpawner::new(2),
        )
        .await
        .unwrap();
        let seeder = local(&seed_dir, true).await;
        let seeding = seeder
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
        seeding.wait_until_completed().await.unwrap();

        let engine = Engine::new(
            local(&leech_dir, false).await,
            &Config {
                max_torrents: 1,
                ..Config::default()
            },
        );
        let hash = torrent.info_hash().as_string();
        let release = Release {
            title: "Movie 2024".into(),
            info_hash: hash.clone(),
            file_idx: None,
            magnet: format!("magnet:?xt=urn:btih:{hash}"),
            size_bytes: None,
            seeders: Some(1),
            source: "test".into(),
            sources: vec!["test".into()],
        };
        let peers = [seeder.listen_addr().unwrap()];
        // Race a new add against invalid same-hash validation, not just cached reuse.
        let (opened, rejected) = tokio::join!(
            engine.open_with_peers(
                &release,
                None,
                None,
                Duration::from_secs(30),
                false,
                &[],
                &peers,
                None,
            ),
            engine.open_with_peers(
                &release,
                None,
                None,
                Duration::from_secs(30),
                false,
                &[],
                &peers,
                Some(("Unrelated Documentary", &[], None)),
            ),
        );
        let playable = opened.unwrap();
        assert!(rejected.is_err());
        assert_eq!(playable.length, FILE as u64);

        // Public opens concurrently reuse the local swarm. A bad identity and a
        // cancelled same-hash waiter must not remove the successful caller's torrent.
        let lifecycle = engine
            .hash_locks
            .get(torrent.info_hash())
            .unwrap()
            .lock_owned()
            .await;
        let mut success =
            Box::pin(engine.open(&release, None, None, Duration::from_secs(5), false, &[]));
        let mut failure = Box::pin(engine.open_for_identity(
            &release,
            None,
            None,
            Duration::from_secs(5),
            false,
            &[],
            "Unrelated Documentary",
            &[],
            None,
        ));
        let mut cancelled =
            Box::pin(engine.open(&release, None, None, Duration::from_secs(5), false, &[]));
        tokio::select! {
            biased;
            result = &mut success => panic!("success escaped lifecycle lock: {result:?}"),
            result = &mut failure => panic!("failure escaped lifecycle lock: {result:?}"),
            result = &mut cancelled => panic!("cancelled escaped lifecycle lock: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        drop(cancelled);
        drop(lifecycle);
        let (succeeded, failed) = tokio::join!(success, failure);
        assert_eq!(succeeded.unwrap(), playable);
        assert!(failed.is_err());
        assert_eq!(engine.active_torrents(), 1);
        assert_eq!(torrent_count(&engine.session).await, 1);

        // An engine adopting an externally managed session must not claim ownership
        // when librqbit returns AlreadyManaged. Failed validation leaves the seeder.
        let adopting = Engine::new(seeder.clone(), &Config::default());
        assert!(
            adopting
                .open_with_peers(
                    &release,
                    None,
                    None,
                    Duration::from_secs(10),
                    false,
                    &[],
                    &[seeder.listen_addr().unwrap()],
                    Some(("Unrelated Documentary", &[], None)),
                )
                .await
                .is_err()
        );
        assert_eq!(torrent_count(&seeder).await, 1);

        let started = Instant::now();
        let (_, mut reader, _pinned) = engine.stream(&hash, playable.file_index).await.unwrap();
        // Both eviction paths must leave a response-pinned local swarm intact.
        assert_eq!(engine.sweep_idle(Duration::ZERO).await, 0);
        engine.make_room().await;
        assert_eq!(engine.active_torrents(), 1);
        let mut buffer = vec![0u8; 256 * 1024];
        let mut read = 0;
        while read < READ {
            read += reader.read(&mut buffer).await.unwrap() as u64;
        }
        let first_read = started.elapsed();
        // Long enough for an engine that downloads everything to have done so.
        tokio::time::sleep(Duration::from_secs(4)).await;

        let stats = engine.stats(&hash, playable.file_index).unwrap();
        let fetched = stats.downloaded_bytes;

        // The peers are idle now. A read or seek must still get its bytes at once, and not wait
        // for an idle peer's five second poll: librqbit as published takes 5s for the first and
        // a second or more for the second.
        let asked = Instant::now();
        reader
            .seek(std::io::SeekFrom::Start(FILE as u64 * 7 / 10))
            .await
            .unwrap();
        reader.read_exact(&mut buffer).await.unwrap();
        let waited = asked.elapsed();

        let _ = std::fs::remove_dir_all(&base);
        assert!(
            first_read < Duration::from_millis(1500),
            "the first read took {first_read:?}"
        );
        assert!(
            waited < Duration::from_millis(750),
            "a seek waited {waited:?}"
        );
        assert!(fetched >= READ, "fetched only {fetched} bytes");
        assert!(
            fetched < FILE as u64 / 2,
            "fetched {fetched} of {FILE} bytes after reading {READ}: that is downloading, not streaming"
        );
    }

    #[test]
    fn downloads_live_in_a_subdirectory_the_engine_owns() {
        let base = Path::new("/home/user/videos");
        assert_eq!(
            Engine::downloads_dir(base),
            Path::new("/home/user/videos/akidev-torrents")
        );
    }
}
