//! Voice models through the facade: what a repository offers, what is installed, and what a failed download leaves.

use akidev_manga::repo::sha256_hex;
use akimanga::{Engine, VoiceProgress};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Ignore;
impl VoiceProgress for Ignore {
    fn update(&self, _done: u64, _total: u64) {}
}

/// A voices repository on this machine: an index, a definition and one file that is not a model.
async fn repository() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let file = b"{}".to_vec();
    let definition = json!({
        "id": "test-model", "name": "Test model", "version": 1, "family": "ptts", "lang": "en",
        "languages": ["en"],
        "licence": { "name": "MIT", "url": "https://example.org", "commercial": true },
        "files": [{ "path": "config.json", "url": format!("{base}files/config.json"),
                    "sha256": sha256_hex(&file), "bytes": file.len() }],
    })
    .to_string();
    let voice = |id: &str, family: &str| {
        json!({
            "id": id, "name": id, "version": 1, "path": "voices/test-model.json",
            "sha256": sha256_hex(definition.as_bytes()), "family": family, "languages": ["en"],
            "bytes": 2, "licence": "MIT", "commercial": true,
        })
    };
    let index = json!({
        "schema": 1, "name": "Test voices",
        "voices": [voice("test-model", "ptts"), voice("future-model", "something-new")],
    })
    .to_string();
    let files = Arc::new(HashMap::from([
        ("index.json".to_string(), index.into_bytes()),
        (
            "voices/test-model.json".to_string(),
            definition.into_bytes(),
        ),
        ("files/config.json".to_string(), file),
    ]));
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let files = files.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 4096];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).to_string();
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let (status, body) = match files.get(path) {
                    Some(body) => ("200 OK", body.clone()),
                    None => ("404 Not Found", b"no".to_vec()),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    base
}

#[tokio::test]
async fn a_repository_lists_models_and_a_bad_one_installs_nothing() {
    let dir = std::env::temp_dir().join(format!("aki-voices-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let engine = Engine::new(dir.to_string_lossy().into()).unwrap();
    let base = repository().await;

    let offered = engine.voices_offered(base.clone()).await.unwrap();
    let flags: Vec<(&str, bool)> = offered
        .iter()
        .map(|model| (model.id.as_str(), model.usable))
        .collect();
    assert_eq!(flags, [("test-model", true), ("future-model", false)]);

    // The file is not a model, so the check after the download refuses it and nothing stays.
    assert!(
        engine
            .install_voice(base.clone(), "test-model".into(), Arc::new(Ignore))
            .await
            .is_err()
    );
    assert!(
        engine
            .install_voice(base.clone(), "future-model".into(), Arc::new(Ignore))
            .await
            .is_err()
    );
    assert!(
        engine
            .install_voice(base, "missing".into(), Arc::new(Ignore))
            .await
            .is_err()
    );
    assert!(engine.voices_installed().is_empty());
    assert!(!dir.join("voices/test-model.part").exists());
    assert!(engine.open_speaker("test-model".into()).is_err());
    assert!(engine.remove_voice("test-model".into()).is_err());

    // A repository that cannot be reached is an error, which the app reads as "not available".
    assert!(
        engine
            .voices_offered("http://127.0.0.1:1/".into())
            .await
            .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
