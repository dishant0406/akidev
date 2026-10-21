//! The facade end to end, against a local server that answers with the madara fixtures: install a
//! theme and a source, browse, read a series, keep it, and come back to it in a new `Engine`.

use akimanga::{Engine, MangaError};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../manga/sources");

/// A server that answers by path (the query is ignored) from the madara fixtures.
async fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 8192];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
                let method = head.split_whitespace().next().unwrap_or("GET").to_string();
                let file = match (method.as_str(), path.as_str()) {
                    ("GET", "/manga/page/1/") => Some("popular.html"),
                    ("GET", "/page/1/") => Some("search.html"),
                    ("GET", "/manga/solo-leveling/") => Some("detail.html"),
                    ("POST", "/manga/solo-leveling/ajax/chapters/") => Some("chapters.html"),
                    ("GET", "/manga/solo-leveling/chapter-179/") => Some("pages.html"),
                    ("GET", "/manga/guarded/") => None,
                    _ => None,
                };
                let (status, body) = match file {
                    Some(file) => ("200 OK", std::fs::read_to_string(Path::new(FIXTURES).join("fixtures/madara-example").join(file)).unwrap()),
                    None if path == "/manga/guarded/" => ("503 Service Unavailable", "<title>Just a moment...</title>".to_string()),
                    None => ("404 Not Found", "no".to_string()),
                };
                let reply = format!("HTTP/1.1 {status}\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                let _ = socket.write_all(reply.as_bytes()).await;
            });
        }
    });
    address
}

fn definition(base: &str) -> String {
    format!(r#"{{"extends":"madara","id":"local","name":"Local","version":1,"lang":["en"],"base_url":"{base}","rate_per_second":20}}"#)
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_read_keep_and_return() {
    let base = serve().await;
    let dir = std::env::temp_dir().join(format!("akimanga-engine-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let engine = Engine::new(dir.to_string_lossy().to_string()).unwrap();
    assert!(engine.sources().iter().any(|source| source.id == "mangadex"));

    engine.install_theme("madara".into(), std::fs::read_to_string(Path::new(FIXTURES).join("themes/madara.json")).unwrap()).unwrap();
    let info = engine.install_source(definition(&base)).unwrap();
    assert_eq!((info.id.as_str(), info.can_search), ("local", true));
    assert!(engine.install_source(definition(&base).replace("\"id\":\"local\"", "\"id\":\"mangadex\"")).is_err());

    let popular = engine.popular("local".into(), 1).await.unwrap();
    assert_eq!(popular.items.iter().map(|card| card.id.as_str()).collect::<Vec<_>>(), ["solo-leveling", "tower-of-god"]);
    let found = engine.search("local".into(), "solo leveling".into(), 1).await.unwrap();
    assert_eq!(found.items.len(), 1);

    let detail = engine.details("local".into(), "solo-leveling".into()).await.unwrap();
    assert_eq!(detail.title, "Solo Leveling");
    engine.set_in_library("local".into(), "solo-leveling".into(), true).await.unwrap();

    let list = engine.chapters("local".into(), "solo-leveling".into()).await.unwrap();
    assert_eq!(list.entries.len(), 3);
    assert!(list.new_ids.is_empty(), "the first fetch has no news");
    let pages = engine.pages("local".into(), "solo-leveling".into(), "chapter-179".into()).await.unwrap();
    assert_eq!(pages.images.len(), 3);

    // Nothing read: continue at the oldest chapter. Reading one moves it.
    assert_eq!(engine.resume("local".into(), "solo-leveling".into()).unwrap().unwrap().chapter, "chapter-178");
    engine.set_progress("local".into(), "solo-leveling".into(), "chapter-178".into(), 2, 0.5, false).unwrap();
    assert_eq!(engine.library().unwrap()[0].unread, 3);
    engine.mark("local".into(), "solo-leveling".into(), vec!["chapter-178".into()], true).unwrap();
    assert_eq!(engine.library().unwrap()[0].unread, 2);

    // A site that wants a check says so, in a way the app can act on.
    let error = engine.details("local".into(), "guarded".into()).await.unwrap_err();
    assert!(matches!(error, MangaError::Challenge { status: 503, .. }), "{error:?}");

    // A new engine on the same folder finds the library, the source and the progress.
    drop(engine);
    let again = Engine::new(dir.to_string_lossy().to_string()).unwrap();
    assert!(again.sources().iter().any(|source| source.id == "local"));
    assert_eq!(again.library().unwrap()[0].unread, 2);
    assert_eq!(again.cached_chapters("local".into(), "solo-leveling".into()).unwrap().iter().filter(|entry| entry.finished).count(), 1);
    again.remove_source("local".into()).unwrap();
    assert!(!again.sources().iter().any(|source| source.id == "local"));
    let _ = std::fs::remove_dir_all(&dir);
}
