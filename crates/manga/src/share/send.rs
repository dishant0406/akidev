//! The phone that gives a file: says hello to the other phone, tells it what is coming, and sends each file once the
//! person there has said yes.

use super::{API, Device, FileMeta, file_type, hex};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::AsyncReadExt;

/// Where a phone is, as the person picked it from the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub ip: String,
    pub port: u16,
    /// "https" or "http".
    pub protocol: String,
}

impl Peer {
    fn url(&self, route: &str) -> String {
        format!("{}://{}:{}{API}{route}", self.protocol, self.ip, self.port)
    }

    /// The address of a call that takes its answers in the address (`?sessionId=...`).
    fn url_with(&self, route: &str, params: &[(&str, &str)]) -> String {
        url::Url::parse_with_params(&self.url(route), params)
            .map_or_else(|_| self.url(route), String::from)
    }
}

/// A file to send, as it is on this phone.
#[derive(Debug, Clone)]
pub struct Outgoing {
    pub path: PathBuf,
    /// The name the other phone keeps it under.
    pub name: String,
}

#[derive(Debug, PartialEq)]
pub enum SendError {
    /// The person on the other phone said no, or did not answer in time.
    Declined,
    /// That phone is receiving from someone else.
    Busy,
    /// This phone stopped it.
    Stopped,
    Failed(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Declined => f.write_str("the other phone said no"),
            SendError::Busy => f.write_str("the other phone is busy"),
            SendError::Stopped => f.write_str("stopped"),
            SendError::Failed(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for SendError {}

fn failed(error: impl std::fmt::Display) -> SendError {
    SendError::Failed(error.to_string())
}

/// A client that talks to phones: they show a certificate made on the spot, which nothing can vouch for, so it is only
/// used to scramble the traffic. What keeps a stranger out is the person's yes and the secrets in each send.
fn client(timeout: Duration) -> anyhow::Result<reqwest::Client> {
    Ok(crate::http::client_builder()
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(timeout)
        .build()?)
}

/// Says hello to the phone at `peer` and learns what it is. Tries https, then http (the LocalSend app may run either).
pub async fn probe(peer: &Peer, me: &Device, wait: Duration) -> anyhow::Result<Device> {
    let mut last = anyhow::anyhow!("no answer");
    for protocol in [
        peer.protocol.as_str(),
        if peer.protocol == "https" {
            "http"
        } else {
            "https"
        },
    ] {
        let there = Peer {
            protocol: protocol.into(),
            ..peer.clone()
        };
        let attempt = async {
            let answer = client(wait)?
                .post(there.url("/register"))
                .json(me)
                .timeout(wait)
                .send()
                .await?;
            Ok::<Device, anyhow::Error>(answer.error_for_status()?.json().await?)
        };
        match attempt.await {
            Ok(device) => return Ok(device),
            Err(error) => last = error,
        }
    }
    Err(last)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Prepared {
    session_id: String,
    files: BTreeMap<String, String>,
}

/// What a file is, found by reading it once.
async fn describe(index: usize, file: &Outgoing) -> anyhow::Result<FileMeta> {
    let mut reading = tokio::fs::File::open(&file.path).await?;
    let (mut hash, mut size, mut chunk) = (Sha256::new(), 0u64, vec![0u8; 256 * 1024]);
    loop {
        let read = reading.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        hash.update(&chunk[..read]);
        size += read as u64;
    }
    Ok(FileMeta {
        id: index.to_string(),
        file_name: file.name.clone(),
        size,
        file_type: file_type(&file.name).to_string(),
        sha256: Some(hex(&hash.finalize())),
        preview: None,
    })
}

/// A send that can be stopped from another thread, and says how far it has got.
pub struct Sending {
    stopped: AtomicBool,
    sent: AtomicU64,
    total: AtomicU64,
}

impl Sending {
    pub fn new() -> Arc<Sending> {
        Arc::new(Sending {
            stopped: AtomicBool::new(false),
            sent: AtomicU64::new(0),
            total: AtomicU64::new(0),
        })
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    /// Bytes sent so far, and bytes in all (0 until the other phone has said yes).
    pub fn progress(&self) -> (u64, u64) {
        (
            self.sent.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Sends `files` to `peer`. Returns when the last file is in, or when something stops it.
    pub async fn run(
        self: &Arc<Self>,
        me: &Device,
        peer: &Peer,
        files: &[Outgoing],
    ) -> Result<(), SendError> {
        let mut metas = Vec::new();
        for (index, file) in files.iter().enumerate() {
            metas.push(describe(index, file).await.map_err(failed)?);
        }
        // The person on the other phone may take a minute to answer, so this call may wait that long.
        let asked = client(Duration::from_secs(90)).map_err(failed)?
            .post(peer.url("/prepare-upload"))
            .json(&serde_json::json!({
                "info": me,
                "files": metas.iter().map(|meta| (meta.id.clone(), meta)).collect::<BTreeMap<_, _>>(),
            }))
            .send()
            .await
            .map_err(failed)?;
        match asked.status().as_u16() {
            200 => {}
            403 => return Err(SendError::Declined),
            409 => return Err(SendError::Busy),
            code => {
                return Err(SendError::Failed(format!(
                    "the other phone answered {code}"
                )));
            }
        }
        let prepared: Prepared = asked.json().await.map_err(failed)?;
        self.total
            .store(metas.iter().map(|meta| meta.size).sum(), Ordering::Relaxed);
        let result = self.upload_all(peer, &prepared, files, &metas).await;
        if result.is_err() {
            // Best effort: tell the other phone to let go.
            if let Ok(client) = client(Duration::from_secs(5)) {
                let _ = client
                    .post(peer.url_with("/cancel", &[("sessionId", &prepared.session_id)]))
                    .send()
                    .await;
            }
        }
        result
    }

    async fn upload_all(
        self: &Arc<Self>,
        peer: &Peer,
        prepared: &Prepared,
        files: &[Outgoing],
        metas: &[FileMeta],
    ) -> Result<(), SendError> {
        let client = client(Duration::from_secs(30)).map_err(failed)?;
        for (file, meta) in files.iter().zip(metas) {
            let token = prepared
                .files
                .get(&meta.id)
                .ok_or_else(|| failed("the other phone left a file out"))?;
            let reading = tokio::fs::File::open(&file.path).await.map_err(failed)?;
            let body = futures::stream::unfold(
                (reading, Arc::clone(self)),
                |(mut reading, send)| async move {
                    if send.stopped.load(Ordering::Relaxed) {
                        return Some((Err(std::io::Error::other("stopped")), (reading, send)));
                    }
                    let mut chunk = vec![0u8; 64 * 1024];
                    match reading.read(&mut chunk).await {
                        Ok(0) => None,
                        Ok(read) => {
                            chunk.truncate(read);
                            send.sent.fetch_add(read as u64, Ordering::Relaxed);
                            Some((Ok(chunk), (reading, send)))
                        }
                        Err(error) => Some((Err(error), (reading, send))),
                    }
                },
            );
            let sent = client
                .post(peer.url_with(
                    "/upload",
                    &[
                        ("sessionId", &prepared.session_id),
                        ("fileId", &meta.id),
                        ("token", token),
                    ],
                ))
                .header("content-length", meta.size)
                .body(reqwest::Body::wrap_stream(body))
                .send()
                .await;
            if self.stopped.load(Ordering::Relaxed) {
                return Err(SendError::Stopped);
            }
            let status = sent.map_err(failed)?.status().as_u16();
            if status != 200 {
                return Err(SendError::Failed(format!(
                    "the other phone refused the file ({status})"
                )));
            }
        }
        Ok(())
    }
}
