//! Sending a book or a comic to a phone on the same Wi-Fi, with no server in between. The wire format is LocalSend's
//! (v2), so the other phone only needs this engine, or the LocalSend app. See pipeline/SHARE_PLAN.md.
//!
//! `receive` is the phone that gets the file (a small HTTPS server), `send` is the one that gives it, and `find` is how
//! they learn of each other.

pub mod card;
pub mod find;
pub mod receive;
pub mod send;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;

/// LocalSend's port, for the server and for the announcements.
pub const PORT: u16 = 53317;
/// LocalSend's multicast address; a device that has just started says so here.
pub const MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
/// Every call of the protocol starts with this.
pub const API: &str = "/api/localsend/v2";
/// The most files in one send.
pub const MAX_FILES: usize = 200;
/// The largest file taken; a comic archive is the biggest thing sent.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// How a device describes itself: in an announcement, a `register` call and the start of a send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub alias: String,
    pub version: String,
    #[serde(default)]
    pub device_model: Option<String>,
    #[serde(default)]
    pub device_type: Option<String>,
    pub fingerprint: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub download: bool,
    /// Only in an announcement: true asks the others to answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announce: Option<bool>,
}

fn default_port() -> u16 {
    PORT
}

fn default_protocol() -> String {
    "https".into()
}

/// One file of a send, as told before any bytes move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    pub file_type: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
}

/// Who this phone is: what it tells the others, and the certificate its server shows. The certificate is made fresh each
/// time the app starts, and its hash is the fingerprint.
pub struct Identity {
    pub device: Device,
    cert: Vec<u8>,
    key: Vec<u8>,
}

impl Identity {
    pub fn new(alias: &str, model: &str, kind: &str) -> anyhow::Result<Identity> {
        let made = rcgen::generate_simple_self_signed(vec!["akidev.local".to_string()])
            .context("could not make a certificate")?;
        let cert = made.cert.der().to_vec();
        Ok(Identity {
            device: Device {
                alias: alias.to_string(),
                version: "2.0".into(),
                device_model: Some(model.to_string()),
                device_type: Some(kind.to_string()),
                fingerprint: hex(&Sha256::digest(&cert)),
                port: PORT,
                protocol: "https".into(),
                download: false,
                announce: None,
            },
            cert,
            key: made.signing_key.serialize_der(),
        })
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A random word nobody can guess: a session's id and each file's token.
pub(crate) fn secret() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("no random numbers: {error}"))?;
    Ok(hex(&bytes))
}

/// The type of a file, by its name, in the words LocalSend uses.
pub fn file_type(name: &str) -> &'static str {
    let name = name.to_lowercase();
    if name.ends_with(".epub") {
        "application/epub+zip"
    } else if name.ends_with(".cbz") {
        "application/vnd.comicbook+zip"
    } else if name.ends_with(".akidev.json") {
        "application/vnd.akidev.share+json"
    } else {
        "application/octet-stream"
    }
}

/// A file name that is safe to write: no folders, no dots at the front, no control characters, never empty.
pub fn clean_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let kept: String = last
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect::<String>()
        .trim_start_matches('.')
        .trim()
        .to_string();
    if kept.is_empty() { "file".into() } else { kept }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_their_folders() {
        assert_eq!(clean_name("../../evil.epub"), "evil.epub");
        assert_eq!(clean_name("C:\\books\\a.epub"), "a.epub");
        assert_eq!(clean_name(".hidden"), "hidden");
        assert_eq!(clean_name(""), "file");
        assert_eq!(clean_name("..."), "file");
    }

    #[test]
    fn types_follow_the_name() {
        assert_eq!(file_type("A.EPUB"), "application/epub+zip");
        assert_eq!(file_type("x.cbz"), "application/vnd.comicbook+zip");
        assert_eq!(
            file_type("x.akidev.json"),
            "application/vnd.akidev.share+json"
        );
        assert_eq!(file_type("x.zip"), "application/octet-stream");
    }

    #[test]
    fn a_device_reads_what_localsend_sends() {
        let json = r#"{"alias":"Pixel","version":"2.0","deviceModel":null,"deviceType":"mobile","fingerprint":"ab","port":53317,"protocol":"http","download":false,"announce":true}"#;
        let device: Device = serde_json::from_str(json).unwrap();
        assert_eq!(device.protocol, "http");
        assert_eq!(device.announce, Some(true));
    }
}
