//! Sending a book or a series to a phone on the same Wi-Fi: the part the apps call. The work is in `akidev_manga::share`;
//! see pipeline/SHARE_PLAN.md for what is sent and why.

use crate::{DEVICE, Engine, Result, failed, now_ms, plain_name};
use akidev_manga::model as m;
use akidev_manga::share::card::Card;
use akidev_manga::share::code::Code;
use akidev_manga::share::receive::{self, Received, Server};
use akidev_manga::share::send::{Outgoing, Peer, SendError, Sending};
use akidev_manga::share::{self, Device, FileMeta, Identity, PORT, clean_name, find};
use akidev_manga::{downloads, local};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

/// What the apps are told while this phone can receive.
#[uniffi::export(with_foreign)]
pub trait ShareEvents: Send + Sync {
    /// Someone wants to send these: ask the person, then call `Share::answer`.
    fn asked(&self, session: String, from: String, files: Vec<ShareOffered>);
    fn progress(&self, session: String, done: u64, total: u64);
    /// Everything is here. Import the files now; they are removed later.
    fn finished(&self, session: String, files: Vec<ShareReceived>);
    fn ended(&self, session: String, reason: String);
    /// A phone is on the network and can be sent to.
    fn seen(&self, peer: SharePeer);
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareOffered {
    pub name: String,
    pub size: u64,
    /// "book", "comic" (a CBZ), "card" (a series from a source) or "other".
    pub kind: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareReceived {
    pub path: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SharePeer {
    pub ip: String,
    pub alias: String,
    /// "mobile", "desktop" and so on, as the phone says.
    pub device_type: Option<String>,
    pub port: u16,
    pub protocol: String,
}

/// What a scanned QR code says (see `Share::make_code`).
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareCode {
    pub ip: Option<String>,
    pub port: Option<u16>,
    /// A temporary Wi-Fi to join first: its name and password.
    pub ssid: Option<String>,
    pub password: Option<String>,
    /// The name of an iPhone to link to directly.
    pub peer: Option<String>,
}

/// A file on this phone, and the name it is sent under.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareFile {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareProgress {
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum ShareOutcome {
    Sent,
    Declined,
    Busy,
    Stopped,
}

/// A series shared from a source, as read from a card.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareCard {
    pub title: String,
    pub cover: Option<String>,
    pub series: String,
    pub source: String,
    pub source_name: String,
    pub nsfw: bool,
    pub languages: Vec<String>,
    /// The repository the source comes from, shown in the question when there is one.
    pub repo: Option<String>,
    /// The source is already installed here, so nothing needs asking.
    pub installed: bool,
}

fn kind(name: &str) -> &'static str {
    match share::file_type(name) {
        "application/epub+zip" => "book",
        "application/vnd.comicbook+zip" => "comic",
        "application/vnd.akidev.share+json" => "card",
        _ => "other",
    }
}

fn peer(ip: &str, device: &Device) -> SharePeer {
    SharePeer {
        ip: ip.to_string(),
        alias: device.alias.clone(),
        device_type: device.device_type.clone(),
        port: device.port,
        protocol: device.protocol.clone(),
    }
}

/// Turns the engine's events into the app's.
struct Bridge(Arc<dyn ShareEvents>);

impl receive::Events for Bridge {
    fn asked(&self, session: &str, from: &Device, files: &[FileMeta]) {
        let files = files
            .iter()
            .map(|file| ShareOffered {
                name: clean_name(&file.file_name),
                size: file.size,
                kind: kind(&file.file_name).into(),
            })
            .collect();
        self.0.asked(session.into(), from.alias.clone(), files);
    }
    fn progress(&self, session: &str, done: u64, total: u64) {
        self.0.progress(session.into(), done, total);
    }
    fn finished(&self, session: &str, files: Vec<Received>) {
        let files = files
            .into_iter()
            .map(|file| ShareReceived {
                path: file.path.display().to_string(),
                name: file.file_name.clone(),
                kind: kind(&file.file_name).into(),
            })
            .collect();
        self.0.finished(session.into(), files);
    }
    fn ended(&self, session: &str, reason: &str) {
        self.0.ended(session.into(), reason.into());
    }
    fn seen(&self, ip: &str, device: &Device) {
        self.0.seen(peer(ip, device));
    }
}

/// One use of the share screen: this phone, visible to the others and ready to receive, until `stop` or until it is dropped.
#[derive(uniffi::Object)]
pub struct Share {
    folder: PathBuf,
    /// This phone's address on the Wi-Fi, as the app found it. The engine guesses when the app has none.
    address: Option<std::net::Ipv4Addr>,
    events: Arc<Bridge>,
    device: Mutex<Device>,
    identity: Mutex<Option<Identity>>,
    server: Mutex<Option<Server>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl Share {
    fn me(&self) -> Device {
        self.device.lock().unwrap().clone()
    }

    fn address(&self) -> Option<std::net::Ipv4Addr> {
        self.address.or_else(find::local_ip)
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.stop();
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Share {
    /// Opens the door for senders. Returns the port it listens on (53317 unless another app holds it).
    pub async fn start(&self) -> Result<u16> {
        let identity = self
            .identity
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| failed("this is already started"))?;
        // The identity is spent by the server, so what the others are told about it is kept first.
        let device = identity.device.clone();
        let events: Arc<dyn receive::Events> = self.events.clone();
        let server = match share::receive::serve(
            identity,
            self.folder.clone(),
            PORT,
            events.clone(),
        )
        .await
        {
            Ok(server) => server,
            Err(_) => {
                // Port taken: ask for any free one. The identity went with the first try, so a new one is made.
                let again = Identity::new(
                    &device.alias,
                    device.device_model.as_deref().unwrap_or(""),
                    device.device_type.as_deref().unwrap_or("mobile"),
                )
                .map_err(|error| failed(error.to_string()))?;
                share::receive::serve(again, self.folder.clone(), 0, events)
                    .await
                    .map_err(|error| failed(error.to_string()))?
            }
        };
        *self.device.lock().unwrap() = server.device.clone();
        let port = server.device.port;
        *self.server.lock().unwrap() = Some(server);
        Ok(port)
    }

    /// Says hello on the network the way the LocalSend app does, and hears the others. Not on iPhone: it blocks this.
    pub async fn listen(&self) -> Result<()> {
        let (me, events) = (self.me(), self.events.clone());
        let task = tokio::spawn(async move {
            let _ = find::listen(me, events).await;
        });
        self.tasks.lock().unwrap().push(task);
        Ok(())
    }

    /// Looks at every address on the Wi-Fi for another phone. Slow, for when the quick ways found nobody; each phone found
    /// comes through `seen`.
    pub async fn scan(&self) {
        if let Some(own) = self.address() {
            find::scan(&self.me(), own, self.events.clone()).await;
        }
    }

    /// Asks the phone at this address if it is there: for what the system found (Bonjour, service discovery), and for an
    /// address typed in by hand.
    pub async fn probe(&self, ip: String, port: u16, protocol: String) -> Result<SharePeer> {
        let there = Peer {
            ip: ip.clone(),
            port,
            protocol,
        };
        let device = share::send::probe(&there, &self.me(), Duration::from_secs(3))
            .await
            .map_err(|error| failed(error.to_string()))?;
        if device.fingerprint == self.me().fingerprint {
            return Err(failed("that is this phone"));
        }
        Ok(peer(&ip, &device))
    }

    /// What the QR code holds: how the other phone reaches this one. By default this phone's address on the Wi-Fi; `ip` replaces
    /// it (the address on a temporary Wi-Fi this phone made, with that Wi-Fi's `ssid` and `password`), and `peer` is the name an
    /// iPhone announces for a direct link. None when there is nothing to say yet: no address, no peer, or not receiving.
    pub fn make_code(
        &self,
        ip: Option<String>,
        ssid: Option<String>,
        password: Option<String>,
        peer: Option<String>,
    ) -> Option<String> {
        let port = self.server.lock().unwrap().as_ref()?.device.port;
        let ip = match ip {
            Some(ip) => Some(ip.parse().ok()?),
            None => self.address(),
        };
        let network = ssid.zip(password);
        // A temporary Wi-Fi is no use without the address on it.
        if network.is_some() && ip.is_none() {
            return None;
        }
        let code = Code {
            port: ip.map(|_| port),
            ip,
            network,
            peer,
        };
        (code.ip.is_some() || code.peer.is_some()).then(|| code.text())
    }

    /// What a scanned code says, or an error when it is not one of ours.
    pub fn read_code(&self, code: String) -> Result<ShareCode> {
        let found = Code::read(&code).ok_or_else(|| failed("that is not a code from Akidev"))?;
        let (ssid, password) = found.network.unzip();
        Ok(ShareCode {
            ip: found.ip.map(|ip| ip.to_string()),
            port: found.port,
            ssid,
            password,
            peer: found.peer,
        })
    }

    /// Finds the phone a scanned code points to, by its address. Fails for anything that is not one of our codes, or that has no
    /// address (a code with only a peer name is for the app's own direct link).
    pub async fn probe_code(&self, code: String) -> Result<SharePeer> {
        let found = self.read_code(code)?;
        let (Some(ip), Some(port)) = (found.ip, found.port) else {
            return Err(failed("that code has no address"));
        };
        self.probe(ip, port, "https".into()).await
    }

    /// The person's answer to `asked`.
    pub fn answer(&self, session: String, accept: bool) {
        if let Some(server) = self.server.lock().unwrap().as_ref() {
            server.receiver.answer(&session, accept);
        }
    }

    /// The person stops a send that is coming in.
    pub fn stop_receiving(&self, session: String) {
        if let Some(server) = self.server.lock().unwrap().as_ref() {
            server.receiver.stop(&session);
        }
    }

    /// Removes the files of sends that are over.
    pub fn clean(&self) {
        if let Some(server) = self.server.lock().unwrap().as_ref() {
            server.receiver.clean();
        }
    }

    /// Closes the door and goes quiet.
    pub fn stop(&self) {
        for task in self.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
        if let Some(server) = self.server.lock().unwrap().take() {
            server.stop();
        }
    }

    /// Gets a send ready to the phone `to`; `run` starts it.
    pub fn sender(&self, to: SharePeer, files: Vec<ShareFile>) -> Arc<ShareSend> {
        Arc::new(ShareSend {
            sending: Sending::new(),
            me: self.me(),
            to: Peer {
                ip: to.ip,
                port: to.port,
                protocol: to.protocol,
            },
            files: files
                .into_iter()
                .map(|file| Outgoing {
                    path: PathBuf::from(file.path),
                    name: file.name,
                })
                .collect(),
        })
    }
}

/// One send to one phone.
#[derive(uniffi::Object)]
pub struct ShareSend {
    sending: Arc<Sending>,
    me: Device,
    to: Peer,
    files: Vec<Outgoing>,
}

#[uniffi::export(async_runtime = "tokio")]
impl ShareSend {
    /// Sends the files. Waits for the other person's yes, so it may take a minute before anything moves.
    pub async fn run(&self) -> Result<ShareOutcome> {
        match self.sending.run(&self.me, &self.to, &self.files).await {
            Ok(()) => Ok(ShareOutcome::Sent),
            Err(SendError::Declined) => Ok(ShareOutcome::Declined),
            Err(SendError::Busy) => Ok(ShareOutcome::Busy),
            Err(SendError::Stopped) => Ok(ShareOutcome::Stopped),
            Err(SendError::Failed(reason)) => Err(failed(reason)),
        }
    }

    pub fn progress(&self) -> ShareProgress {
        let (done, total) = self.sending.progress();
        ShareProgress { done, total }
    }

    pub fn stop(&self) {
        self.sending.stop();
    }
}

impl Engine {
    fn outgoing(&self) -> Result<PathBuf> {
        let folder = self.dir.join("share-out");
        std::fs::create_dir_all(&folder)
            .map_err(|error| failed(format!("cannot make room to send: {error}")))?;
        Ok(folder)
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Engine {
    /// This phone as the others see it. `kind` is "mobile" or "desktop". `wifi_address` is the phone's address on the Wi-Fi.
    pub fn share(
        &self,
        alias: String,
        model: String,
        kind: String,
        wifi_address: Option<String>,
        events: Arc<dyn ShareEvents>,
    ) -> Result<Arc<Share>> {
        let identity =
            Identity::new(&alias, &model, &kind).map_err(|error| failed(error.to_string()))?;
        Ok(Arc::new(Share {
            folder: self.dir.join("share-in"),
            address: wifi_address.and_then(|ip| ip.parse().ok()),
            events: Arc::new(Bridge(events)),
            device: Mutex::new(identity.device.clone()),
            identity: Mutex::new(Some(identity)),
            server: Mutex::new(None),
            tasks: Mutex::new(Vec::new()),
        }))
    }

    /// A book as it is: the EPUB, under the book's title.
    pub fn share_book(&self, id: String) -> Result<ShareFile> {
        let row = self
            .store
            .book(&id)?
            .ok_or_else(|| failed("that book is not on the shelf"))?;
        Ok(ShareFile {
            path: self.dir.join("books").join(&row.file).display().to_string(),
            name: format!("{}.epub", clean_name(&row.title)),
        })
    }

    /// A series read from a source: a card with the source and the series, a few kilobytes and no pictures.
    pub fn share_card(
        &self,
        source: String,
        series: String,
        title: String,
        cover: Option<String>,
    ) -> Result<ShareFile> {
        if source == DEVICE || !plain_name(&source) {
            return Err(failed("a comic from a file is shared as the file"));
        }
        let info = self.source(&source)?.info();
        let repo = self.installed_origin(source.clone());
        // A source added by hand has no repository, so the card carries the source itself.
        let definition = match repo {
            Some(_) => None,
            None => Some(
                std::fs::read_to_string(self.dir.join("sources").join(format!("{source}.json")))
                    .map_err(|_| failed("this source cannot be shared"))?,
            ),
        };
        let card = Card {
            card: 1,
            title,
            cover: cover.filter(|cover| cover.starts_with("http")),
            series,
            source,
            source_name: info.name,
            nsfw: info.nsfw,
            languages: info.languages,
            repo,
            definition,
        };
        let path = self.outgoing()?.join(card.file_name());
        card.write(&path)
            .map_err(|error| failed(error.to_string()))?;
        Ok(ShareFile {
            path: path.display().to_string(),
            name: card.file_name(),
        })
    }

    /// A comic that came from a file: its pages packed back into a CBZ.
    pub fn share_comic(&self, series: String, title: String) -> Result<ShareFile> {
        if !plain_name(&series) {
            return Err(failed("that is not a comic on this phone"));
        }
        let pages = downloads::pages(&self.dir.join("local").join(&series));
        if pages.is_empty() {
            return Err(failed("that comic has no pages here"));
        }
        let name = format!("{}.cbz", clean_name(&title));
        let path = self.outgoing()?.join(&name);
        local::export(&pages, &path).map_err(|error| failed(error.to_string()))?;
        Ok(ShareFile {
            path: path.display().to_string(),
            name,
        })
    }

    /// Removes what `share_card` and `share_comic` made, once the send is over.
    pub fn share_clear_outgoing(&self) {
        let _ = std::fs::remove_dir_all(self.dir.join("share-out"));
    }

    /// Reads a card that arrived, to show what it is before anything is added.
    pub fn share_read_card(&self, path: String) -> Result<ShareCard> {
        let card =
            Card::read(std::path::Path::new(&path)).map_err(|error| failed(error.to_string()))?;
        let installed = self.sources.read().unwrap().contains_key(&card.source);
        Ok(ShareCard {
            title: card.title,
            cover: card.cover,
            series: card.series,
            source: card.source,
            source_name: card.source_name,
            nsfw: card.nsfw,
            languages: card.languages,
            repo: card.repo,
            installed,
        })
    }

    /// Adds the card's source (if this phone does not have it) and the series, to the library. Ask the person first when the
    /// source is new: it is code that runs here.
    pub async fn share_add_card(&self, path: String) -> Result<()> {
        let card =
            Card::read(std::path::Path::new(&path)).map_err(|error| failed(error.to_string()))?;
        let installed = self.sources.read().unwrap().contains_key(&card.source);
        if !installed {
            let _ = match (card.definition, card.repo) {
                (Some(definition), _) => self.install_source(definition)?,
                (None, Some(repo)) => self.install_from_repo(repo, card.source.clone()).await?,
                (None, None) => return Err(failed("the card does not say where its source is")),
            };
            if !self.sources.read().unwrap().contains_key(&card.source) {
                return Err(failed("the card names a different source than it carries"));
            }
        }
        self.store.upsert_series(
            &card.source,
            &card.series,
            &card.title,
            card.cover.as_deref(),
            None,
            m::Status::Unknown,
        )?;
        self.store
            .add_to_library(&card.source, &card.series, now_ms())?;
        Ok(())
    }
}
