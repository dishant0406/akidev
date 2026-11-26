//! A series shared from one engine to another through the app-facing calls: a card goes over the network and the other
//! phone ends up with the source and the series.

use akimanga::{Engine, ShareEvents, ShareOffered, ShareOutcome, SharePeer, ShareReceived};
use std::path::Path;
use std::sync::{Arc, Mutex};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../manga/testdata");

#[derive(Default)]
struct Screen {
    share: Mutex<Option<Arc<akimanga::Share>>>,
    finished: Mutex<Vec<ShareReceived>>,
    asked: Mutex<Vec<(String, Vec<ShareOffered>)>>,
}

impl ShareEvents for Screen {
    fn asked(&self, session: String, from: String, files: Vec<ShareOffered>) {
        self.asked.lock().unwrap().push((from, files));
        self.share
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .answer(session, true);
    }
    fn progress(&self, _: String, _: u64, _: u64) {}
    fn finished(&self, _: String, files: Vec<ShareReceived>) {
        *self.finished.lock().unwrap() = files;
    }
    fn ended(&self, _: String, _: String) {}
    fn seen(&self, _: SharePeer) {}
}

#[tokio::test(flavor = "multi_thread")]
async fn a_series_from_a_hand_added_source_arrives_with_its_source() {
    let temp = std::env::temp_dir().join(format!("akimanga-share-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let path = |name: &str| temp.join(name).to_string_lossy().to_string();

    // The sender has a source added by hand, and the series saved from it.
    let sender = Engine::new(path("sender")).unwrap();
    let theme = std::fs::read_to_string(Path::new(FIXTURES).join("themes/madara.json")).unwrap();
    sender.install_theme("madara".into(), theme).unwrap();
    let source = r#"{"extends":"madara","id":"pasted","name":"Pasted","version":1,"base_url":"http://127.0.0.1:1","rate_per_second":20}"#;
    sender.install_source(source.into()).unwrap();
    let card = sender
        .share_card(
            "pasted".into(),
            "solo-leveling".into(),
            "Solo Leveling".into(),
            Some("https://example.com/c.jpg".into()),
        )
        .unwrap();
    assert_eq!(card.name, "Solo Leveling.akidev.json");
    assert!(
        sender
            .share_card("device".into(), "x".into(), "X".into(), None)
            .is_err(),
        "a comic from a file goes as the file"
    );

    // The receiver has nothing, and opens the door.
    let receiver = Engine::new(path("receiver")).unwrap();
    let screen = Arc::new(Screen::default());
    let there = receiver
        .share(
            "Receiver".into(),
            "Test".into(),
            "mobile".into(),
            None,
            screen.clone(),
        )
        .unwrap();
    *screen.share.lock().unwrap() = Some(there.clone());
    let port = there.start().await.unwrap();

    // The sender finds it by its address, as when the system found it, and sends.
    let here = sender
        .share(
            "Sender".into(),
            "Test".into(),
            "mobile".into(),
            None,
            Arc::new(Screen::default()),
        )
        .unwrap();
    let found = here
        .probe("127.0.0.1".into(), port, "https".into())
        .await
        .unwrap();
    assert_eq!(found.alias, "Receiver");
    let send = here.sender(found, vec![card]);
    assert_eq!(send.run().await.unwrap(), ShareOutcome::Sent);
    assert_eq!(send.progress().done, send.progress().total);

    {
        let asked = screen.asked.lock().unwrap();
        assert_eq!(
            (asked[0].0.as_str(), asked[0].1[0].kind.as_str()),
            ("Sender", "card")
        );
    }
    let got = screen.finished.lock().unwrap()[0].clone();
    assert_eq!(got.kind, "card");

    // The card shows what it is; the source is new here, so the screen asks before adding it.
    let read = receiver.share_read_card(got.path.clone()).unwrap();
    assert_eq!(
        (read.source_name.as_str(), read.installed),
        ("Pasted", false)
    );
    assert!(receiver.sources().is_empty());
    receiver.share_add_card(got.path).await.unwrap();
    assert_eq!(receiver.sources().len(), 1);
    let library = receiver.library().unwrap();
    assert_eq!(
        (
            library[0].source.as_str(),
            library[0].id.as_str(),
            library[0].title.as_str()
        ),
        ("pasted", "solo-leveling", "Solo Leveling")
    );
    assert!(receiver.share_read_card(path("nothing")).is_err());

    there.clean();
    there.stop();
    sender.share_clear_outgoing();
    let _ = std::fs::remove_dir_all(&temp);
}

/// A comic imported from a file is shared as a CBZ, and the other phone reads it as one.
#[tokio::test(flavor = "multi_thread")]
async fn a_comic_from_a_file_goes_as_the_file() {
    use std::io::Write;
    let temp = std::env::temp_dir().join(format!("akimanga-share-cbz-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let cbz = temp.join("Little Comic.cbz");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&cbz).unwrap());
    for name in ["1.jpg", "2.jpg"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(name.as_bytes()).unwrap();
    }
    zip.finish().unwrap();

    let sender = Engine::new(temp.join("sender").to_string_lossy().to_string()).unwrap();
    let series = sender
        .import_file(cbz.to_string_lossy().to_string())
        .unwrap();
    let file = sender.share_comic(series, "Little Comic".into()).unwrap();
    assert_eq!(file.name, "Little Comic.cbz");

    let receiver = Engine::new(temp.join("receiver").to_string_lossy().to_string()).unwrap();
    let screen = Arc::new(Screen::default());
    let there = receiver
        .share(
            "Receiver".into(),
            "Test".into(),
            "mobile".into(),
            None,
            screen.clone(),
        )
        .unwrap();
    *screen.share.lock().unwrap() = Some(there.clone());
    let port = there.start().await.unwrap();
    let here = sender
        .share(
            "Sender".into(),
            "Test".into(),
            "mobile".into(),
            None,
            Arc::new(Screen::default()),
        )
        .unwrap();
    let found = here
        .probe("127.0.0.1".into(), port, "https".into())
        .await
        .unwrap();
    assert_eq!(
        here.sender(found, vec![file]).run().await.unwrap(),
        ShareOutcome::Sent
    );

    let got = screen.finished.lock().unwrap()[0].clone();
    assert_eq!(got.kind, "comic");
    receiver.import_file(got.path).unwrap();
    assert_eq!(receiver.library().unwrap().len(), 1);
    there.stop();
    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn a_scanned_code_finds_the_phone_that_made_it() {
    let temp = std::env::temp_dir().join(format!("akimanga-share-code-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let engine = Engine::new(temp.to_string_lossy().to_string()).unwrap();
    let share = engine
        .share(
            "Receiver".into(),
            "Test".into(),
            "mobile".into(),
            Some("127.0.0.1".into()),
            Arc::new(Screen::default()),
        )
        .unwrap();
    // Nothing to say before the door is open.
    assert_eq!(share.make_code(None, None, None, None), None);
    let port = share.start().await.unwrap();

    let wifi = share.make_code(None, None, None, None).unwrap();
    assert_eq!(wifi, format!("akidev://share?h=127.0.0.1&p={port}"));
    let hotspot = share
        .make_code(
            Some("192.168.49.1".into()),
            Some("DIRECT-ab".into()),
            Some("pass word".into()),
            None,
        )
        .unwrap();
    let read = share.read_code(hotspot).unwrap();
    assert_eq!(read.ip.as_deref(), Some("192.168.49.1"));
    assert_eq!(read.ssid.as_deref(), Some("DIRECT-ab"));
    assert_eq!(read.password.as_deref(), Some("pass word"));
    let peer = share
        .make_code(None, None, None, Some("akidev-1a2b3c4d".into()))
        .unwrap();
    assert_eq!(
        share.read_code(peer).unwrap().peer.as_deref(),
        Some("akidev-1a2b3c4d")
    );
    // A network with no address, and text that is not ours, make nothing.
    assert_eq!(share.make_code(Some("nope".into()), None, None, None), None);
    assert!(share.read_code("https://example.com".into()).is_err());

    // Another phone uses the Wi-Fi code to find this one.
    let other = Engine::new(temp.join("other").to_string_lossy().to_string()).unwrap();
    let there = other
        .share(
            "Sender".into(),
            "Test".into(),
            "mobile".into(),
            None,
            Arc::new(Screen::default()),
        )
        .unwrap();
    assert_eq!(there.probe_code(wifi).await.unwrap().alias, "Receiver");
    share.stop();
    let _ = std::fs::remove_dir_all(&temp);
}
