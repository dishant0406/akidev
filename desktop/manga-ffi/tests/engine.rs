//! The facade end to end, against a local server that answers with the madara fixtures: install a
//! theme and a source, browse, read a series, keep it, and come back to it in a new `Engine`.

use akidev_manga::repo;
use akimanga::{Engine, MangaError, SourceState};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The smallest thing that starts like a JPEG: the engine checks the first bytes and the size, not the picture.
const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0xFF,
    0xD9,
];
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../manga/testdata");

/// How many times the test server was asked for chapter 179's pages.
static PAGE_ASKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A server that answers by path (the query is ignored) from the madara fixtures, and serves the repository
/// folder `repo_dir` (if any) under /repo/.
async fn serve(repo_dir: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let repo_dir = repo_dir.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 8192];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).to_string();
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .to_string();
                let method = head.split_whitespace().next().unwrap_or("GET").to_string();
                if path == "/manga/solo-leveling/chapter-179/" {
                    PAGE_ASKS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    // Slow enough that a second ask arrives while the first is still being answered.
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
                let file = match (method.as_str(), path.as_str()) {
                    ("GET", "/manga/page/1/") => Some("popular.html"),
                    ("GET", "/page/1/") => Some("search.html"),
                    ("GET", "/manga/solo-leveling/") => Some("detail.html"),
                    ("POST", "/manga/solo-leveling/ajax/chapters/") => Some("chapters.html"),
                    ("GET", "/manga/solo-leveling/chapter-179/") => Some("pages.html"),
                    ("GET", "/manga/guarded/") => None,
                    _ => None,
                };
                if let Some(inside) = path.strip_prefix("/repo/") {
                    let (status, body) = match std::fs::read(repo_dir.join(inside)) {
                        Ok(body) => ("200 OK", body),
                        Err(_) => ("404 Not Found", b"no".to_vec()),
                    };
                    let reply = format!(
                        "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                    return;
                }
                let (status, body) = match file {
                    Some(file) => (
                        "200 OK",
                        std::fs::read_to_string(
                            Path::new(FIXTURES)
                                .join("fixtures/madara-example")
                                .join(file),
                        )
                        .unwrap(),
                    ),
                    None if path == "/manga/guarded/" => (
                        "503 Service Unavailable",
                        "<title>Just a moment...</title>".to_string(),
                    ),
                    None => ("404 Not Found", "no".to_string()),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            });
        }
    });
    address
}

/// A repository folder with the madara theme and a source called `local` (version `version`) reading `base`.
fn make_repo(dir: &Path, base: &str, version: u64, key: Option<String>) {
    std::fs::create_dir_all(dir.join("themes")).unwrap();
    std::fs::create_dir_all(dir.join("sources")).unwrap();
    std::fs::copy(
        Path::new(FIXTURES).join("themes/madara.json"),
        dir.join("themes/madara.json"),
    )
    .unwrap();
    std::fs::write(dir.join("repo.json"), r#"{"name":"Test repo"}"#).unwrap();
    let source = format!(
        r#"{{"extends":"madara","id":"local","name":"Local","version":{version},"lang":["en"],"base_url":"{base}","rate_per_second":20}}"#
    );
    std::fs::write(dir.join("sources/local.json"), source).unwrap();
    let index = repo::build_index(dir, key).unwrap();
    std::fs::write(
        dir.join("index.json"),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn add_a_repository_install_browse_read_update_and_return() {
    let temp = std::env::temp_dir().join(format!("akimanga-engine-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let (repo_dir, data_dir) = (temp.join("repo"), temp.join("data"));
    let address = serve(repo_dir.clone()).await;
    make_repo(&repo_dir, &address, 1, None);

    // The app is a renderer: nothing is installed and nothing is known until a repository is added.
    let engine = Engine::new(data_dir.to_string_lossy().to_string()).unwrap();
    assert!(engine.sources().is_empty());
    assert!(engine.repos().unwrap().is_empty());
    assert!(engine.popular("local".into(), 1).await.is_err());

    let repo_link = format!("{address}/repo/");
    let view = engine.add_repo(repo_link.clone()).await.unwrap();
    assert_eq!(
        (view.info.name.as_str(), view.info.signed),
        ("Test repo", false)
    );
    assert_eq!(view.sources.len(), 1);
    assert_eq!(view.sources[0].state, SourceState::NotInstalled);
    assert!(
        engine.sources().is_empty(),
        "adding a repository installs nothing"
    );

    let info = engine
        .install_from_repo(view.info.base.clone(), "local".into())
        .await
        .unwrap();
    assert_eq!((info.id.as_str(), info.can_search), ("local", true));
    assert_eq!(
        engine
            .repo_sources(view.info.base.clone())
            .await
            .unwrap()
            .sources[0]
            .state,
        SourceState::Installed
    );

    // What a repository installed keeps its `extends`, so it follows its theme rather than a frozen copy.
    let kept: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(data_dir.join("sources/local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(kept["extends"].as_str(), Some("madara"));
    assert!(
        kept.get("detail").is_none(),
        "a repository's source is not flattened"
    );
    assert!(
        engine
            .install_from_repo(view.info.base.clone(), "nope".into())
            .await
            .is_err()
    );

    assert!(
        engine.remembered_listing("local".into(), false).is_none(),
        "nothing is remembered before the first load"
    );
    let popular = engine.popular("local".into(), 1).await.unwrap();
    let remembered = engine.remembered_listing("local".into(), false).unwrap();
    assert_eq!(
        remembered
            .items
            .iter()
            .map(|card| card.id.as_str())
            .collect::<Vec<_>>(),
        ["solo-leveling", "tower-of-god"]
    );
    assert_eq!(
        popular
            .items
            .iter()
            .map(|card| card.id.as_str())
            .collect::<Vec<_>>(),
        ["solo-leveling", "tower-of-god"]
    );
    assert_eq!(
        engine
            .search("local".into(), "solo leveling".into(), 1, Vec::new())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        engine
            .details("local".into(), "solo-leveling".into())
            .await
            .unwrap()
            .title,
        "Solo Leveling"
    );
    engine
        .set_in_library("local".into(), "solo-leveling".into(), true)
        .await
        .unwrap();
    assert_eq!(
        engine
            .chapters_checked_ms("local".into(), "solo-leveling".into())
            .unwrap(),
        None
    );
    let list = engine
        .chapters("local".into(), "solo-leveling".into())
        .await
        .unwrap();
    assert!(
        engine
            .chapters_checked_ms("local".into(), "solo-leveling".into())
            .unwrap()
            .is_some()
    );
    assert_eq!(engine.updates().unwrap().len(), 1);
    assert_eq!(list.entries.len(), 3);
    assert!(list.new_ids.is_empty(), "the first fetch has no news");
    assert_eq!(
        engine
            .pages("local".into(), "solo-leveling".into(), "chapter-179".into())
            .await
            .unwrap()
            .images
            .len(),
        3
    );
    assert_eq!(
        engine
            .resume("local".into(), "solo-leveling".into())
            .unwrap()
            .unwrap()
            .chapter,
        "chapter-178"
    );
    engine
        .set_progress(
            "local".into(),
            "solo-leveling".into(),
            "chapter-178".into(),
            2,
            0.5,
            false,
        )
        .unwrap();
    engine
        .mark(
            "local".into(),
            "solo-leveling".into(),
            vec!["chapter-178".into()],
            true,
        )
        .unwrap();
    assert_eq!(engine.library().unwrap()[0].unread, 2);

    // A site that wants a check says so, in a way the app can act on.
    let error = engine
        .details("local".into(), "guarded".into())
        .await
        .unwrap_err();
    assert!(
        matches!(error, MangaError::Challenge { status: 503, .. }),
        "{error:?}"
    );

    // The repository publishes version 2: the app sees it and updates the one source it came from.
    assert!(engine.update_sources().await.unwrap().is_empty());
    make_repo(&repo_dir, &address, 2, None);
    assert_eq!(
        engine
            .repo_sources(view.info.base.clone())
            .await
            .unwrap()
            .sources[0]
            .state,
        SourceState::UpdateAvailable
    );
    assert_eq!(engine.update_sources().await.unwrap(), ["local"]);
    assert_eq!(engine.sources()[0].version, 2);

    // A file that is not what the index says (changed after the index was written) is refused.
    std::fs::write(
        repo_dir.join("sources/local.json"),
        std::fs::read_to_string(repo_dir.join("sources/local.json"))
            .unwrap()
            .replace("\"version\":2", "\"version\":3"),
    )
    .unwrap();
    let error = engine
        .install_from_repo(view.info.base.clone(), "local".into())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hash differs"), "{error}");
    assert_eq!(
        engine.sources()[0].version,
        2,
        "a refused file leaves what was installed alone"
    );
    make_repo(&repo_dir, &address, 2, None);

    // A new engine on the same folder finds the repository, the source, the library and the progress.
    drop(engine);
    let again = Engine::new(data_dir.to_string_lossy().to_string()).unwrap();
    assert_eq!(again.repos().unwrap().len(), 1);
    assert_eq!(again.sources().len(), 1);
    assert_eq!(again.library().unwrap()[0].unread, 2);
    again.remove_source("local".into()).unwrap();
    assert!(again.sources().is_empty());
    again.remove_repo(view.info.base).unwrap();
    assert!(again.repos().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_signed_repository_cannot_swap_its_key_unnoticed() {
    let temp = std::env::temp_dir().join(format!("akimanga-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let repo_dir = temp.join("repo");
    let address = serve(repo_dir.clone()).await;
    let engine = Engine::new(temp.join("data").to_string_lossy().to_string()).unwrap();

    let sign = |dir: &Path| {
        let (secret, public) = repo::generate_key().unwrap();
        make_repo(dir, &address, 1, Some(public));
        let text = std::fs::read(dir.join("index.json")).unwrap();
        std::fs::write(
            dir.join("index.json.sig"),
            repo::sign(&text, &secret).unwrap(),
        )
        .unwrap();
    };
    sign(&repo_dir);
    let link = format!("{address}/repo/");
    assert!(engine.add_repo(link.clone()).await.unwrap().info.signed);
    // The same address is now served with a different key, correctly signed by it.
    sign(&repo_dir);
    let error = engine.add_repo(link.clone()).await.unwrap_err();
    assert!(error.to_string().contains("different key"), "{error}");
    // Removing it and adding it again is the person choosing to trust the new key.
    engine.remove_repo(format!("{address}/repo/")).unwrap();
    assert!(engine.add_repo(link).await.is_ok());
    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_library_survives_a_backup_into_a_fresh_install_and_holds_files_from_the_device() {
    let temp = std::env::temp_dir().join(format!("akimanga-backup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let repo_dir = temp.join("repo");
    let address = serve(repo_dir.clone()).await;
    make_repo(&repo_dir, &address, 1, None);
    let link = format!("{address}/repo/");
    let engine = Engine::new(temp.join("one").to_string_lossy().to_string()).unwrap();

    // Looking at a repository keeps nothing.
    assert_eq!(
        engine
            .preview_repo(link.clone())
            .await
            .unwrap()
            .sources
            .len(),
        1
    );
    assert!(engine.repos().unwrap().is_empty());

    let view = engine.add_repo(link).await.unwrap();
    engine
        .install_from_repo(view.info.base, "local".into())
        .await
        .unwrap();
    engine
        .details("local".into(), "solo-leveling".into())
        .await
        .unwrap();
    engine
        .set_in_library("local".into(), "solo-leveling".into(), true)
        .await
        .unwrap();
    engine
        .chapters("local".into(), "solo-leveling".into())
        .await
        .unwrap();
    engine
        .mark(
            "local".into(),
            "solo-leveling".into(),
            vec!["chapter-178".into()],
            true,
        )
        .unwrap();
    let reading = engine.add_category("Reading".into()).unwrap();
    engine
        .set_category("local".into(), "solo-leveling".into(), reading, true)
        .unwrap();
    assert!(engine.add_category("  ".into()).is_err());
    assert_eq!(engine.categories().unwrap()[0].count, 1);
    // The order given is the order kept (here the only category named, with the second one added first in line).
    let later = engine.add_category("Later".into()).unwrap();
    engine.reorder_categories(vec![later, reading]).unwrap();
    assert_eq!(
        engine
            .categories()
            .unwrap()
            .iter()
            .map(|category| category.name.as_str())
            .collect::<Vec<_>>(),
        ["Later", "Reading"]
    );

    // A comic archive from the device sits in the library beside the rest and reads from its files.
    let archive = temp.join("Short Story.cbz");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    for name in ["2.jpg", "1.jpg"] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"image").unwrap();
    }
    writer.finish().unwrap();
    let id = engine
        .import_file(archive.to_string_lossy().to_string())
        .unwrap();
    assert!(id.starts_with("short-story-"), "{id}");
    let pages = engine
        .pages("device".into(), id.clone(), "1".into())
        .await
        .unwrap();
    assert_eq!(pages.images.len(), 2);
    assert!(pages.images[0].starts_with("file://") && pages.images[0].ends_with("0000.jpg"));
    assert_eq!(engine.library().unwrap().len(), 2);

    // Moving to a series that was never opened, or onto itself, is refused rather than losing the library entry.
    assert!(
        engine
            .migrate(
                "local".into(),
                "solo-leveling".into(),
                "local".into(),
                "nope".into()
            )
            .is_err()
    );
    assert!(
        engine
            .migrate(
                "local".into(),
                "solo-leveling".into(),
                "local".into(),
                "solo-leveling".into()
            )
            .is_err()
    );
    assert_eq!(engine.library().unwrap().len(), 2);

    // A cover the person chose shows in the library instead of the source's, as a file on the device.
    engine
        .set_custom_cover("local".into(), "solo-leveling".into(), JPEG.to_vec())
        .unwrap();
    let shown = |engine: &Engine| {
        engine
            .library()
            .unwrap()
            .into_iter()
            .find(|item| item.title == "Solo Leveling")
            .unwrap()
            .cover
            .unwrap()
    };
    let chosen = shown(&engine);
    assert!(
        chosen.starts_with("file://") && chosen.ends_with(".jpg"),
        "{chosen}"
    );
    assert_eq!(
        engine
            .custom_cover("local".into(), "solo-leveling".into())
            .unwrap()
            .as_deref(),
        Some(chosen.as_str())
    );
    assert_eq!(
        std::fs::read(chosen.trim_start_matches("file://")).unwrap(),
        JPEG
    );
    // Anything that is not a small JPEG is refused, and the cover stays as it was.
    assert!(
        engine
            .set_custom_cover(
                "local".into(),
                "solo-leveling".into(),
                b"not a picture".to_vec()
            )
            .is_err()
    );
    assert_eq!(shown(&engine), chosen);
    // Several in a row never share a file, however fast: the last one keeps its picture.
    for _ in 0..5 {
        engine
            .set_custom_cover("local".into(), "solo-leveling".into(), JPEG.to_vec())
            .unwrap();
        let now = shown(&engine);
        assert!(
            std::path::Path::new(now.trim_start_matches("file://")).exists(),
            "{now}"
        );
    }
    // A series that does not exist cannot be given one, and nothing is left behind for it.
    let before = std::fs::read_dir(temp.join("one").join("covers"))
        .unwrap()
        .count();
    assert!(
        engine
            .set_custom_cover("local".into(), "nope".into(), JPEG.to_vec())
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(temp.join("one").join("covers"))
            .unwrap()
            .count(),
        before
    );
    // Choosing another deletes the file it replaces.
    engine
        .set_custom_cover("local".into(), "solo-leveling".into(), JPEG.to_vec())
        .unwrap();
    let second = shown(&engine);
    assert_ne!(second, chosen);
    assert!(!std::path::Path::new(chosen.trim_start_matches("file://")).exists());
    // A marked page is part of the backup.
    engine
        .add_bookmark(
            "local".into(),
            "solo-leveling".into(),
            "chapter-178".into(),
            4,
            0.3,
        )
        .unwrap();

    // The backup goes into a fresh install: the library comes back, and the source is fetched again from its repository.
    let backup = engine.backup().unwrap();
    let fresh = Engine::new(temp.join("two").to_string_lossy().to_string()).unwrap();
    assert!(fresh.restore("not json".into()).await.is_err());
    let report = fresh.restore(backup).await.unwrap();
    assert_eq!(
        (
            report.series,
            report.sources_installed,
            report.sources_missing
        ),
        (2, vec!["local".to_string()], Vec::<String>::new())
    );
    assert_eq!(fresh.repos().unwrap().len(), 1);
    assert_eq!(fresh.sources().len(), 1);
    let library = fresh.library().unwrap();
    let reading_here = fresh
        .categories()
        .unwrap()
        .into_iter()
        .find(|category| category.name == "Reading")
        .unwrap()
        .id;
    assert!(
        library
            .iter()
            .any(|item| item.title == "Solo Leveling" && item.categories == [reading_here])
    );
    assert_eq!(
        fresh
            .cached_chapters("local".into(), "solo-leveling".into())
            .unwrap()
            .len(),
        0,
        "chapters are fetched again, not carried"
    );
    // The read state waits for the chapters to be fetched again, then shows.
    let list = fresh
        .chapters("local".into(), "solo-leveling".into())
        .await
        .unwrap();
    assert!(
        list.entries
            .iter()
            .any(|entry| entry.id == "chapter-178" && entry.finished)
    );
    // The chosen cover comes back as a file in the new install's own folder.
    let restored_cover = shown(&fresh);
    assert!(
        restored_cover.starts_with("file://") && restored_cover.contains("/two/"),
        "{restored_cover}"
    );
    assert_eq!(
        std::fs::read(restored_cover.trim_start_matches("file://")).unwrap(),
        JPEG
    );
    // A bookmark shows once its chapter is listed again.
    assert_eq!(
        fresh
            .bookmarks("local".into(), "solo-leveling".into())
            .unwrap()
            .iter()
            .map(|mark| (mark.chapter.as_str(), mark.page))
            .collect::<Vec<_>>(),
        [("chapter-178", 4)]
    );

    // Removing a series forgets all of it.
    fresh
        .forget_series("local".into(), "solo-leveling".into())
        .unwrap();
    assert!(
        fresh
            .library()
            .unwrap()
            .iter()
            .all(|item| item.id != "solo-leveling")
    );
    let _ = std::fs::remove_dir_all(&temp);
}

/// A source pasted on its own: it extends a theme, so the theme is merged into it when it is installed,
/// and it keeps working after the theme is gone. Removing a repository leaves a source installed by hand alone.
#[tokio::test(flavor = "multi_thread")]
async fn a_pasted_source_carries_its_theme_and_outlives_it() {
    let temp = std::env::temp_dir().join(format!("akimanga-pasted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let (repo_dir, data_dir) = (temp.join("repo"), temp.join("data"));
    let address = serve(repo_dir.clone()).await;
    make_repo(&repo_dir, &address, 1, None);

    // A source on its own, extending a theme that is not installed: refused, and said so plainly.
    let engine = Engine::new(data_dir.to_string_lossy().to_string()).unwrap();
    let alone = format!(
        r#"{{"extends":"madara","id":"pasted","name":"Pasted","version":1,"base_url":"{address}","rate_per_second":20}}"#
    );
    let error = engine.install_source(alone.clone()).unwrap_err();
    assert!(error.to_string().contains("madara"), "{error}");

    // The theme arrives (from a repository, or by hand); then the same pasted definition installs, theme and all.
    let theme = std::fs::read_to_string(Path::new(FIXTURES).join("themes/madara.json")).unwrap();
    engine.install_theme("madara".into(), theme).unwrap();
    engine.add_repo(format!("{address}/repo/")).await.unwrap();
    let preview = engine.preview_source(alone.clone()).unwrap();
    assert_eq!(
        (preview.id.as_str(), preview.name.as_str(), preview.version),
        ("pasted", "Pasted", 1)
    );
    assert!(engine.install_source("not json".into()).is_err());
    assert!(engine.preview_source("not json".into()).is_err());
    assert!(
        engine.preview_source(alone.clone()).is_ok() && engine.sources().is_empty(),
        "previewing installs nothing"
    );

    // A preview refuses exactly what an install refuses, so the sheet never offers one that would then fail.
    let bad_id = r#"{"extends":"madara","id":"Bad/Id","name":"Bad","version":1,"base_url":"http://127.0.0.1:1"}"#;
    assert!(engine.preview_source(bad_id.into()).is_err());
    assert!(engine.install_source(bad_id.into()).is_err());

    let info = engine.install_source(alone.clone()).unwrap();
    assert_eq!(info.id, "pasted");
    assert_eq!(engine.sources().len(), 1);

    // An older version than the one installed is refused, by a preview as well as an install.
    let older = alone.replace(r#""version":1"#, r#""version":0"#);
    assert!(engine.preview_source(older.clone()).is_err());
    assert!(engine.install_source(older).is_err());
    assert_eq!(
        engine
            .popular("pasted".into(), 1)
            .await
            .unwrap()
            .items
            .len(),
        2
    );

    // What is kept on disk is the definition with the theme in it: no `extends`, its own id and version.
    let kept: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(data_dir.join("sources/pasted.json")).unwrap(),
    )
    .unwrap();
    assert!(kept.get("extends").is_none() && kept.get("theme").is_none());
    assert_eq!(
        (kept["id"].as_str(), kept["version"].as_u64()),
        (Some("pasted"), Some(1))
    );

    // The theme goes away entirely — every theme file deleted — and the pasted source still loads, in a new
    // Engine too. That is the whole point: it is not quietly depending on a theme someone else installed.
    let base = engine.repos().unwrap()[0].base.clone();
    engine.remove_repo(base).unwrap();
    drop(engine);
    for entry in std::fs::read_dir(data_dir.join("themes")).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    let reopened = Engine::new(data_dir.to_string_lossy().to_string()).unwrap();
    assert_eq!(reopened.sources().len(), 1);
    assert_eq!(
        reopened
            .popular("pasted".into(), 1)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn a_backup_cannot_write_outside_the_covers_folder_or_name_a_picture_it_lacks() {
    let temp = std::env::temp_dir().join(format!("akimanga-covers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let archive = temp.join("Story.cbz");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    writer
        .start_file("1.jpg", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"image").unwrap();
    writer.finish().unwrap();
    let engine = Engine::new(temp.join("one").to_string_lossy().to_string()).unwrap();
    let id = engine
        .import_file(archive.to_string_lossy().to_string())
        .unwrap();
    engine
        .set_custom_cover("device".into(), id.clone(), JPEG.to_vec())
        .unwrap();
    let backup: serde_json::Value = serde_json::from_str(&engine.backup().unwrap()).unwrap();
    assert_eq!(backup["covers"].as_object().unwrap().len(), 1);

    // A name that climbs out of the folder, and a name whose picture the backup does not carry.
    for (name, carried) in [("../../escaped.jpg", true), ("custom/123.jpg", false)] {
        let mut hostile = backup.clone();
        hostile["library"]["series"][0]["custom_cover"] = name.into();
        if carried {
            hostile["covers"] = serde_json::json!({ name: "/9j/4AAQ" });
        } else {
            hostile["covers"] = serde_json::json!({});
        }
        let fresh_dir = temp.join(format!("fresh-{carried}"));
        let fresh = Engine::new(fresh_dir.to_string_lossy().to_string()).unwrap();
        fresh.restore(hostile.to_string()).await.unwrap();
        let cover = fresh.library().unwrap()[0].cover.clone();
        // Not a picture of ours: the series simply has no custom cover, and nothing was written beside the folder.
        assert!(
            cover.is_none_or(|cover| !cover.contains("/covers/")),
            "{name}"
        );
        assert!(!temp.join("escaped.jpg").exists() && !fresh_dir.join("escaped.jpg").exists());
        assert_eq!(
            fresh.custom_cover("device".into(), id.clone()).unwrap(),
            None
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chapters_pages_are_asked_of_the_site_once_however_many_ask_at_once() {
    let temp = std::env::temp_dir().join(format!("akimanga-pages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let (repo_dir, data_dir) = (temp.join("repo"), temp.join("data"));
    let address = serve(repo_dir.clone()).await;
    make_repo(&repo_dir, &address, 1, None);
    let engine = Engine::new(data_dir.to_string_lossy().to_string()).unwrap();
    let view = engine.add_repo(format!("{address}/repo/")).await.unwrap();
    engine
        .install_from_repo(view.info.base, "local".into())
        .await
        .unwrap();

    let before = PAGE_ASKS.load(std::sync::atomic::Ordering::SeqCst);
    let ask = || engine.pages("local".into(), "solo-leveling".into(), "chapter-179".into());
    // The series page fetching ahead, and the reader opening a moment later.
    let (ahead, reader) = tokio::join!(ask(), ask());
    assert_eq!(ahead.unwrap().images, reader.unwrap().images);
    // And once more later: from memory.
    ask().await.unwrap();
    assert_eq!(
        PAGE_ASKS.load(std::sync::atomic::Ordering::SeqCst) - before,
        1
    );
}
