//! Reading a repository over HTTP: the index, the files it names, and what is refused.

use akidev_manga::repo::{self, build_index, fetch_file, fetch_index};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serves files from a folder, with some paths replaced by given bytes.
async fn serve(dir: PathBuf, overrides: HashMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let overrides = Arc::new(overrides);
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let (dir, overrides) = (dir.clone(), overrides.clone());
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 4096];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).to_string();
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/')
                    .to_string();
                let body = overrides
                    .get(&path)
                    .cloned()
                    .or_else(|| std::fs::read(dir.join(&path)).ok());
                let (status, body) = match body {
                    Some(body) => ("200 OK", body),
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

/// A small synthetic repository under `testdata/repo`: one theme and one source. The catalogue of real
/// source definitions lives in its own repository (`github.com/dishant0406/manga-sources`) and is checked
/// there; these tests only need a well-formed repository to read over HTTP.
fn repo_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/repo")
}

/// The files in a folder, or nothing when the folder is not there: a repository may ship no sources at all.
fn files(folder: PathBuf) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default()
}

/// A copy of the repository's themes and sources in a temp folder, with an index built for it.
fn copy_repo(name: &str, key: Option<String>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("akidev-repo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for folder in ["themes", "sources"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        for path in files(repo_dir().join(folder)) {
            std::fs::copy(&path, dir.join(folder).join(path.file_name().unwrap())).unwrap();
        }
    }
    std::fs::copy(repo_dir().join("repo.json"), dir.join("repo.json")).unwrap();
    let index = build_index(&dir, key).unwrap();
    std::fs::write(
        dir.join("index.json"),
        serde_json::to_string_pretty(&index).unwrap() + "\n",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn the_committed_repository_reads_and_its_files_match_their_hashes() {
    let base = serve(repo_dir(), HashMap::new()).await;
    let fetched = fetch_index(&base).await.unwrap();
    assert!(!fetched.signed);
    // Whatever the repository lists (sources come and go), every file it lists reads and matches its hash.
    let theme = &fetched.index.themes[0];
    assert!(
        fetch_file(&base, &theme.path, &theme.sha256)
            .await
            .unwrap()
            .contains("\"theme\": true")
    );
    for source in &fetched.index.sources {
        assert!(
            fetch_file(&base, &source.path, &source.sha256)
                .await
                .is_ok(),
            "{}",
            source.id
        );
    }
}

#[tokio::test]
async fn a_file_that_is_not_what_the_index_says_is_refused() {
    let dir = copy_repo("tamper", None);
    let tampered = HashMap::from([(
        "themes/madara.json".to_string(),
        b"{\"theme\":true}".to_vec(),
    )]);
    let base = serve(dir.clone(), tampered).await;
    let fetched = fetch_index(&base).await.unwrap();
    let theme = &fetched.index.themes[0];
    let error = fetch_file(&base, &theme.path, &theme.sha256)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("hash differs"), "{error}");
    assert!(
        fetch_file(&base, "../etc/passwd", &theme.sha256)
            .await
            .is_err()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_signed_repository_must_carry_a_matching_signature() {
    let (secret, public) = repo::generate_key().unwrap();
    let dir = copy_repo("signed", Some(public.clone()));
    let text = std::fs::read(dir.join("index.json")).unwrap();
    std::fs::write(
        dir.join("index.json.sig"),
        repo::sign(&text, &secret).unwrap(),
    )
    .unwrap();

    let base = serve(dir.clone(), HashMap::new()).await;
    let fetched = fetch_index(&base).await.unwrap();
    assert!(fetched.signed);
    assert_eq!(fetched.index.key.as_deref(), Some(public.as_str()));

    // The index changed after it was signed.
    let mut changed = String::from_utf8(text.clone()).unwrap();
    changed = changed.replace("Test repo", "Somebody else");
    let base = serve(
        dir.clone(),
        HashMap::from([("index.json".to_string(), changed.into_bytes())]),
    )
    .await;
    let error = fetch_index(&base).await.err().unwrap().to_string();
    assert!(error.contains("signature does not match"), "{error}");

    // It names a key but the signature is missing.
    let base = serve(
        dir.clone(),
        HashMap::from([("index.json.sig".to_string(), Vec::new())]),
    )
    .await;
    assert!(fetch_index(&base).await.is_err());
    let _ = std::fs::remove_file(dir.join("index.json.sig"));
    let base = serve(dir.clone(), HashMap::new()).await;
    assert!(
        fetch_index(&base)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("no index.json.sig")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_folder_without_an_index_says_so() {
    let base = serve(std::env::temp_dir(), HashMap::new()).await;
    let error = fetch_index(&format!("{base}nothing-here/"))
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("no index.json"), "{error}");
}
