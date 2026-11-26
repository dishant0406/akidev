//! Two phones in one process: a receiver and a sender talking over localhost, the way they do over Wi-Fi.

use akidev_manga::share::receive::{Events, Received, Receiver, Server, serve};
use akidev_manga::share::send::{Outgoing, Peer, SendError, Sending, probe};
use akidev_manga::share::{Device, FileMeta, Identity};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Yes,
    No,
    Wait,
}

/// The receiving screen, as a test: answers at once, or not at all.
struct Screen {
    answer: Answer,
    receiver: Mutex<Option<Arc<Receiver>>>,
    asked: Mutex<Vec<(String, Device, Vec<FileMeta>)>>,
    finished: Mutex<Vec<Received>>,
    ended: Mutex<Vec<String>>,
    progress: Mutex<(u64, u64)>,
    seen: Mutex<Vec<String>>,
}

impl Events for Screen {
    fn asked(&self, session: &str, from: &Device, files: &[FileMeta]) {
        self.asked
            .lock()
            .unwrap()
            .push((session.into(), from.clone(), files.to_vec()));
        let receiver = self.receiver.lock().unwrap().clone().unwrap();
        match self.answer {
            Answer::Yes => receiver.answer(session, true),
            Answer::No => receiver.answer(session, false),
            Answer::Wait => {}
        }
    }
    fn progress(&self, _: &str, done: u64, total: u64) {
        *self.progress.lock().unwrap() = (done, total);
    }
    fn finished(&self, _: &str, files: Vec<Received>) {
        *self.finished.lock().unwrap() = files;
    }
    fn ended(&self, _: &str, reason: &str) {
        self.ended.lock().unwrap().push(reason.into());
    }
    fn seen(&self, ip: &str, _: &Device) {
        self.seen.lock().unwrap().push(ip.into());
    }
}

struct Phones {
    server: Server,
    screen: Arc<Screen>,
    peer: Peer,
    me: Device,
    folder: PathBuf,
}

impl Drop for Phones {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

async fn start(name: &str, answer: Answer) -> Phones {
    let folder =
        std::env::temp_dir().join(format!("akidev-share-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let screen = Arc::new(Screen {
        answer,
        receiver: Mutex::new(None),
        asked: Mutex::default(),
        finished: Mutex::default(),
        ended: Mutex::default(),
        progress: Mutex::default(),
        seen: Mutex::default(),
    });
    let server = serve(
        Identity::new("Receiver", "Test", "mobile").unwrap(),
        folder.join("in"),
        0,
        screen.clone(),
    )
    .await
    .unwrap();
    *screen.receiver.lock().unwrap() = Some(server.receiver.clone());
    let peer = Peer {
        ip: "127.0.0.1".into(),
        port: server.device.port,
        protocol: "https".into(),
    };
    let me = Identity::new("Sender", "Test", "mobile").unwrap().device;
    Phones {
        server,
        screen,
        peer,
        me,
        folder,
    }
}

/// A file of `size` bytes that is not all the same.
fn file(phones: &Phones, name: &str, size: usize) -> Outgoing {
    let path = phones
        .folder
        .join(format!("out-{}", name.replace('/', "_")));
    std::fs::create_dir_all(&phones.folder).unwrap();
    std::fs::write(
        &path,
        (0..size).map(|i| (i % 251) as u8).collect::<Vec<u8>>(),
    )
    .unwrap();
    Outgoing {
        path,
        name: name.into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_files_arrive_whole() {
    let phones = start("whole", Answer::Yes).await;
    let files = [
        file(&phones, "Book.epub", 3 * 1024 * 1024 + 7),
        file(&phones, "card.akidev.json", 300),
    ];
    let sending = Sending::new();
    sending.run(&phones.me, &phones.peer, &files).await.unwrap();

    let got = phones.screen.finished.lock().unwrap().clone();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].file_name, "Book.epub");
    assert_eq!(got[0].file_type, "application/epub+zip");
    assert_eq!(got[1].file_type, "application/vnd.akidev.share+json");
    for (got, sent) in got.iter().zip(&files) {
        assert_eq!(
            std::fs::read(&got.path).unwrap(),
            std::fs::read(&sent.path).unwrap()
        );
    }
    let total = 3 * 1024 * 1024 + 7 + 300;
    assert_eq!(*phones.screen.progress.lock().unwrap(), (total, total));
    assert_eq!(sending.progress(), (total, total));
    let asked = phones.screen.asked.lock().unwrap();
    assert_eq!(asked[0].1.alias, "Sender");
    assert_eq!(asked[0].2.len(), 2);

    phones.server.receiver.clean();
    assert!(!got[0].path.exists(), "clean removes what was kept");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_no_sends_nothing() {
    let phones = start("no", Answer::No).await;
    let files = [file(&phones, "Book.epub", 1000)];
    let result = Sending::new().run(&phones.me, &phones.peer, &files).await;
    assert_eq!(result, Err(SendError::Declined));
    assert!(phones.screen.finished.lock().unwrap().is_empty());
    assert_eq!(phones.screen.ended.lock().unwrap().as_slice(), ["declined"]);
    assert!(
        std::fs::read_dir(phones.folder.join("in"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_sender_is_told_the_phone_is_busy() {
    let phones = start("busy", Answer::Wait).await;
    let files = [file(&phones, "Book.epub", 1000)];
    let first = {
        let (me, peer, files) = (phones.me.clone(), phones.peer.clone(), files.to_vec());
        tokio::spawn(async move { Sending::new().run(&me, &peer, &files).await })
    };
    while phones.screen.asked.lock().unwrap().is_empty() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let second = Sending::new().run(&phones.me, &phones.peer, &files).await;
    assert_eq!(second, Err(SendError::Busy));

    let id = phones.screen.asked.lock().unwrap()[0].0.clone();
    phones.server.receiver.answer(&id, true);
    first.await.unwrap().unwrap();
    assert_eq!(phones.screen.finished.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_with_the_wrong_hash_is_refused() {
    let phones = start("hash", Answer::Yes).await;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let base = format!("https://127.0.0.1:{}/api/localsend/v2", phones.peer.port);
    let prepared: serde_json::Value = client
        .post(format!("{base}/prepare-upload"))
        .json(&serde_json::json!({
            "info": phones.me,
            "files": {"a": {"id": "a", "fileName": "a.epub", "size": 4, "fileType": "application/epub+zip", "sha256": "00"}},
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = prepared["sessionId"].as_str().unwrap();
    let token = prepared["files"]["a"].as_str().unwrap();

    let wrong_token = client
        .post(format!("{base}/upload?sessionId={id}&fileId=a&token=nope"))
        .body("abcd")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_token.status(), 403);
    let wrong_hash = client
        .post(format!(
            "{base}/upload?sessionId={id}&fileId=a&token={token}"
        ))
        .body("abcd")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_hash.status(), 422);
    assert!(phones.screen.finished.lock().unwrap().is_empty());
    assert_eq!(
        phones.screen.ended.lock().unwrap().as_slice(),
        ["a file did not arrive intact"]
    );
    let gone = client
        .post(format!(
            "{base}/upload?sessionId={id}&fileId=a&token={token}"
        ))
        .body("abcd")
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), 403, "the send is over");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_larger_than_it_said_is_refused() {
    let phones = start("size", Answer::Yes).await;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let base = format!("https://127.0.0.1:{}/api/localsend/v2", phones.peer.port);
    let prepared: serde_json::Value = client
        .post(format!("{base}/prepare-upload"))
        .json(&serde_json::json!({
            "info": phones.me,
            "files": {"a": {"id": "a", "fileName": "a.epub", "size": 2, "fileType": "application/epub+zip"}},
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let (id, token) = (
        prepared["sessionId"].as_str().unwrap(),
        prepared["files"]["a"].as_str().unwrap(),
    );
    let big = client
        .post(format!(
            "{base}/upload?sessionId={id}&fileId=a&token={token}"
        ))
        .body("abcd")
        .send()
        .await
        .unwrap();
    assert_eq!(big.status(), 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_name_with_folders_stays_inside() {
    let phones = start("name", Answer::Yes).await;
    let files = [file(&phones, "../../evil.epub", 10)];
    Sending::new()
        .run(&phones.me, &phones.peer, &files)
        .await
        .unwrap();
    let got = phones.screen.finished.lock().unwrap()[0].clone();
    assert_eq!(got.file_name, "evil.epub");
    assert!(got.path.starts_with(phones.folder.join("in")));
}

#[tokio::test(flavor = "multi_thread")]
async fn too_many_files_are_refused() {
    let phones = start("many", Answer::Yes).await;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let files: serde_json::Map<String, serde_json::Value> = (0..201)
        .map(|i| (i.to_string(), serde_json::json!({"id": i.to_string(), "fileName": "a", "size": 1, "fileType": "x"})))
        .collect();
    let answer = client
        .post(format!(
            "https://127.0.0.1:{}/api/localsend/v2/prepare-upload",
            phones.peer.port
        ))
        .json(&serde_json::json!({"info": phones.me, "files": files}))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 400);
    assert!(phones.screen.asked.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_tells_each_phone_about_the_other() {
    let phones = start("hello", Answer::Yes).await;
    let there = probe(&phones.peer, &phones.me, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(there.alias, "Receiver");
    assert_eq!(there.fingerprint, phones.server.device.fingerprint);
    assert_eq!(phones.screen.seen.lock().unwrap().as_slice(), ["127.0.0.1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sender_can_stop() {
    let phones = start("stop", Answer::Yes).await;
    let files = [file(&phones, "Big.cbz", 40 * 1024 * 1024)];
    let sending = Sending::new();
    let run = {
        let (sending, me, peer, files) = (
            sending.clone(),
            phones.me.clone(),
            phones.peer.clone(),
            files.to_vec(),
        );
        tokio::spawn(async move { sending.run(&me, &peer, &files).await })
    };
    while sending.progress().0 == 0 {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    sending.stop();
    assert_eq!(run.await.unwrap(), Err(SendError::Stopped));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(phones.screen.finished.lock().unwrap().is_empty());
    assert_eq!(
        phones.screen.ended.lock().unwrap().len(),
        1,
        "the receiver let go"
    );
}
