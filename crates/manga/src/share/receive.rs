//! The phone that gets a file: a small HTTPS server that answers LocalSend's calls, asks the person before anything is
//! kept, and writes what arrives into a folder of its own. It never runs while the person is not on the receive screen.

use super::{API, Device, FileMeta, Identity, MAX_FILE_BYTES, MAX_FILES, clean_name, secret};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

/// How long the person has to say yes.
const ASK_FOR: Duration = Duration::from_secs(60);
/// A send with no news for this long is over, and the next one may start.
const IDLE: Duration = Duration::from_secs(120);
/// The most a request about a send may weigh; the files themselves are not counted.
const MAX_REQUEST: usize = 1024 * 1024;
/// Progress is told this often, in bytes.
const REPORT_EVERY: u64 = 128 * 1024;

/// What the screen is told. Each call returns at once; the screen answers `asked` later with `Receiver::answer`.
pub trait Events: Send + Sync {
    /// Someone wants to send these files: show who and what, and ask.
    fn asked(&self, session: &str, from: &Device, files: &[FileMeta]);
    fn progress(&self, session: &str, done: u64, total: u64);
    /// Every file is here. They stay in their folder until `Receiver::clean`, so import them now.
    fn finished(&self, session: &str, files: Vec<Received>);
    /// The send stopped: declined, cancelled, nobody answered, or a file came wrong.
    fn ended(&self, session: &str, reason: &str);
    /// A phone said hello (the address it came from, and what it is).
    fn seen(&self, ip: &str, device: &Device);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Received {
    pub path: PathBuf,
    pub file_name: String,
    pub file_type: String,
}

struct Slot {
    meta: FileMeta,
    token: String,
    path: Option<PathBuf>,
}

struct Session {
    id: String,
    slots: BTreeMap<String, Slot>,
    /// Waiting for the person; becomes none once answered.
    answer: Option<oneshot::Sender<bool>>,
    accepted: bool,
    total: u64,
    done: u64,
    touched: Instant,
}

pub struct Receiver {
    device: Device,
    dir: PathBuf,
    events: Arc<dyn Events>,
    session: Mutex<Option<Session>>,
}

#[derive(Deserialize)]
struct Prepare {
    info: Device,
    files: BTreeMap<String, FileMeta>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Prepared {
    session_id: String,
    files: BTreeMap<String, String>,
}

type Reply = Response<Full<Bytes>>;

fn reply(code: u16, body: Option<String>) -> Reply {
    let mut builder = Response::builder().status(code);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(Full::new(Bytes::from(body.unwrap_or_default())))
        .expect("a status and a body are a response")
}

fn json(value: &impl Serialize) -> Reply {
    reply(200, serde_json::to_string(value).ok())
}

/// A send that ends when the phone on the other side goes away, unless it was ended on purpose first.
struct Ends<'a> {
    receiver: &'a Receiver,
    id: String,
    armed: bool,
}

impl Ends<'_> {
    fn now(&mut self, reason: &str) {
        self.armed = false;
        self.receiver.end(&self.id, reason);
    }
}

impl Drop for Ends<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.receiver.end(&self.id, "the other phone stopped");
        }
    }
}

impl Receiver {
    /// The person's answer to `Events::asked`.
    pub fn answer(&self, session: &str, accept: bool) {
        let waiting = match self.session.lock().unwrap().as_mut() {
            Some(open) if open.id == session => open.answer.take(),
            _ => None,
        };
        if let Some(waiting) = waiting {
            let _ = waiting.send(accept);
        }
    }

    /// The person stops a send that is coming in.
    pub fn stop(&self, session: &str) {
        self.end(session, "stopped on this phone");
    }

    /// Removes what earlier sends left, except the one that is coming in now.
    pub fn clean(&self) {
        let keep = self
            .session
            .lock()
            .unwrap()
            .as_ref()
            .map(|open| open.id.clone());
        for entry in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            if keep.as_deref() != entry.file_name().to_str() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    fn end(&self, id: &str, reason: &str) {
        let mut open = self.session.lock().unwrap();
        if open.as_ref().is_some_and(|open| open.id == id) {
            *open = None;
            drop(open);
            let _ = std::fs::remove_dir_all(self.dir.join(id));
            self.events.ended(id, reason);
        }
    }

    async fn handle(
        self: Arc<Self>,
        peer: IpAddr,
        request: Request<Incoming>,
    ) -> Result<Reply, Infallible> {
        let route = request
            .uri()
            .path()
            .strip_prefix(API)
            .unwrap_or("")
            .to_string();
        let query: BTreeMap<String, String> = request
            .uri()
            .query()
            .map(|query| {
                url::form_urlencoded::parse(query.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_default();
        Ok(match (request.method(), route.as_str()) {
            (&Method::POST, "/register") => self.register(peer, request).await,
            (&Method::GET, "/info") => json(&self.device),
            (&Method::POST, "/prepare-upload") => self.prepare(peer, request).await,
            (&Method::POST, "/upload") => self.upload(&query, request).await,
            (&Method::POST, "/cancel") => {
                if let Some(id) = query.get("sessionId") {
                    self.end(id, "the other phone cancelled");
                }
                reply(200, None)
            }
            _ => reply(404, None),
        })
    }

    async fn register(&self, peer: IpAddr, request: Request<Incoming>) -> Reply {
        let Ok(body) = Limited::new(request.into_body(), MAX_REQUEST)
            .collect()
            .await
        else {
            return reply(400, None);
        };
        match serde_json::from_slice::<Device>(&body.to_bytes()) {
            Ok(device) => {
                // A phone that finds itself on the network (its own address) is not news.
                if device.fingerprint != self.device.fingerprint {
                    self.events.seen(&peer.to_string(), &device);
                }
                json(&self.device)
            }
            Err(_) => reply(400, None),
        }
    }

    /// The first call of a send: nothing but names and sizes. The person decides here.
    async fn prepare(&self, peer: IpAddr, request: Request<Incoming>) -> Reply {
        let Ok(body) = Limited::new(request.into_body(), MAX_REQUEST)
            .collect()
            .await
        else {
            return reply(400, None);
        };
        let Ok(Prepare { info, files }) = serde_json::from_slice(&body.to_bytes()) else {
            return reply(400, None);
        };
        let sizes_fit = files.values().all(|file| file.size <= MAX_FILE_BYTES);
        if files.is_empty() || files.len() > MAX_FILES || !sizes_fit {
            return reply(400, None);
        }
        let (Ok(id), Some(slots)) = (secret(), tokens(files.clone())) else {
            return reply(500, None);
        };
        let (answer, answered) = oneshot::channel();
        {
            let mut open = self.session.lock().unwrap();
            if open
                .as_ref()
                .is_some_and(|open| open.touched.elapsed() < IDLE)
            {
                return reply(409, None);
            }
            *open = Some(Session {
                id: id.clone(),
                total: files.values().map(|file| file.size).sum(),
                slots,
                answer: Some(answer),
                accepted: false,
                done: 0,
                touched: Instant::now(),
            });
        }
        self.events.seen(&peer.to_string(), &info);
        let mut ends = Ends {
            receiver: self,
            id: id.clone(),
            armed: true,
        };
        let list: Vec<FileMeta> = files.into_values().collect();
        self.events.asked(&id, &info, &list);
        match tokio::time::timeout(ASK_FOR, answered).await {
            Ok(Ok(true)) => {}
            Ok(_) => {
                ends.now("declined");
                return reply(403, None);
            }
            Err(_) => {
                ends.now("nobody answered");
                return reply(403, None);
            }
        }
        if std::fs::create_dir_all(self.dir.join(&id)).is_err() {
            ends.now("could not make room for the files");
            return reply(500, None);
        }
        let mut open = self.session.lock().unwrap();
        let Some(open) = open.as_mut().filter(|open| open.id == id) else {
            return reply(403, None);
        };
        open.accepted = true;
        open.touched = Instant::now();
        let files = open
            .slots
            .iter()
            .map(|(id, slot)| (id.clone(), slot.token.clone()))
            .collect();
        ends.armed = false;
        json(&Prepared {
            session_id: id,
            files,
        })
    }

    /// One file's bytes. A file that comes wrong ends the whole send.
    async fn upload(&self, query: &BTreeMap<String, String>, request: Request<Incoming>) -> Reply {
        let (Some(id), Some(file_id), Some(token)) = (
            query.get("sessionId"),
            query.get("fileId"),
            query.get("token"),
        ) else {
            return reply(400, None);
        };
        let (meta, folder) = {
            let mut open = self.session.lock().unwrap();
            let Some(open) = open.as_mut().filter(|open| open.id == *id && open.accepted) else {
                return reply(403, None);
            };
            let index = open
                .slots
                .keys()
                .position(|key| key == file_id)
                .unwrap_or(0);
            let Some(slot) = open.slots.get(file_id).filter(|slot| slot.token == *token) else {
                return reply(403, None);
            };
            if slot.path.is_some() {
                return reply(409, None);
            }
            let meta = slot.meta.clone();
            open.touched = Instant::now();
            (meta, self.dir.join(id).join(index.to_string()))
        };
        let mut ends = Ends {
            receiver: self,
            id: id.clone(),
            armed: true,
        };
        let path = match self.store(id, &meta, &folder, request.into_body()).await {
            Ok(path) => path,
            Err((code, reason)) => {
                ends.now(reason);
                return reply(code, None);
            }
        };
        ends.armed = false;
        self.arrived(id, file_id, path);
        reply(200, None)
    }

    /// Writes the body into `folder`, checking its size and hash. The error is the status to answer with, and why.
    async fn store(
        &self,
        id: &str,
        meta: &FileMeta,
        folder: &std::path::Path,
        mut body: Incoming,
    ) -> Result<PathBuf, (u16, &'static str)> {
        const FAILED: (u16, &str) = (500, "could not write the file");
        let name = clean_name(&meta.file_name);
        let part = folder.join(format!("{name}.part"));
        std::fs::create_dir_all(folder).map_err(|_| FAILED)?;
        let mut file = tokio::fs::File::create(&part).await.map_err(|_| FAILED)?;
        let mut hash = Sha256::new();
        let (mut size, mut unreported) = (0u64, 0u64);
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|_| (400, "the file did not arrive"))?;
            let Ok(data) = frame.into_data() else {
                continue;
            };
            size += data.len() as u64;
            if size > meta.size {
                return Err((400, "a file was larger than it said"));
            }
            hash.update(&data);
            file.write_all(&data).await.map_err(|_| FAILED)?;
            unreported += data.len() as u64;
            if unreported >= REPORT_EVERY {
                self.add_done(id, std::mem::take(&mut unreported));
            }
        }
        file.flush().await.map_err(|_| FAILED)?;
        self.add_done(id, unreported);
        if size != meta.size {
            return Err((400, "a file was smaller than it said"));
        }
        if meta
            .sha256
            .as_ref()
            .is_some_and(|wanted| !wanted.eq_ignore_ascii_case(&super::hex(&hash.finalize())))
        {
            return Err((422, "a file did not arrive intact"));
        }
        let path = folder.join(name);
        std::fs::rename(&part, &path).map_err(|_| FAILED)?;
        Ok(path)
    }

    fn add_done(&self, id: &str, bytes: u64) {
        let told = match self.session.lock().unwrap().as_mut() {
            Some(open) if open.id == id => {
                open.done += bytes;
                open.touched = Instant::now();
                Some((open.done, open.total))
            }
            _ => None,
        };
        if let Some((done, total)) = told {
            self.events.progress(id, done, total);
        }
    }

    /// A file is in. When it was the last, the send is over.
    fn arrived(&self, id: &str, file_id: &str, path: PathBuf) {
        let all = {
            let mut open = self.session.lock().unwrap();
            let Some(session) = open.as_mut().filter(|open| open.id == id) else {
                return;
            };
            if let Some(slot) = session.slots.get_mut(file_id) {
                slot.path = Some(path);
            }
            if session.slots.values().any(|slot| slot.path.is_none()) {
                return;
            }
            let done = open.take().expect("the session was just open");
            done.slots
                .into_values()
                .filter_map(|slot| {
                    Some(Received {
                        path: slot.path?,
                        file_name: clean_name(&slot.meta.file_name),
                        file_type: slot.meta.file_type,
                    })
                })
                .collect::<Vec<_>>()
        };
        self.events.finished(id, all);
    }
}

/// Each file with its own secret, which the sender must show to send it.
fn tokens(files: BTreeMap<String, FileMeta>) -> Option<BTreeMap<String, Slot>> {
    files
        .into_iter()
        .map(|(id, meta)| {
            Some((
                id,
                Slot {
                    meta,
                    token: secret().ok()?,
                    path: None,
                },
            ))
        })
        .collect()
}

/// A running server. Dropping it, or calling `stop`, closes the door.
pub struct Server {
    pub device: Device,
    pub receiver: Arc<Receiver>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    pub fn stop(&self) {
        self.task.abort();
        let open = self.receiver.session.lock().unwrap().take();
        if let Some(open) = open {
            let _ = std::fs::remove_dir_all(self.receiver.dir.join(&open.id));
            self.receiver
                .events
                .ended(&open.id, "this phone stopped receiving");
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Opens the door on `port` (0 picks a free one; the real one is in `Server::device`). Files are kept under `dir`, which is
/// emptied first.
pub async fn serve(
    identity: Identity,
    dir: PathBuf,
    port: u16,
    events: Arc<dyn Events>,
) -> anyhow::Result<Server> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from(identity.cert)],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.key)),
    )?;
    let tls = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    let mut device = identity.device;
    device.port = listener.local_addr()?.port();
    let receiver = Arc::new(Receiver {
        device: device.clone(),
        dir,
        events,
        session: Mutex::new(None),
    });
    let serving = receiver.clone();
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        while let Ok((stream, peer)) = listener.accept().await {
            while connections.try_join_next().is_some() {}
            let (tls, receiver) = (tls.clone(), serving.clone());
            connections.spawn(async move {
                let Ok(stream) = tls.accept(stream).await else {
                    return;
                };
                let service = hyper::service::service_fn(move |request| {
                    receiver.clone().handle(peer.ip(), request)
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    Ok(Server {
        device,
        receiver,
        task,
    })
}
