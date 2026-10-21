//! Keeping a chapter's images on the device so it reads without a network.

use anyhow::Context;
use std::path::{Path, PathBuf};

const MAX_IMAGE: usize = 40 * 1024 * 1024;

/// A folder name for one part of the path: readable characters kept, never empty or all dots, and a short hash of
/// the original so two different names never share a folder.
fn safe(part: &str) -> String {
    use sha2::{Digest, Sha256};
    let readable: String = part.chars().take(40).map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let hash: String = Sha256::digest(part.as_bytes()).iter().take(4).map(|byte| format!("{byte:02x}")).collect();
    format!("{readable}-{hash}")
}

/// Where a chapter's pages are kept: `root/<source>/<series>/<chapter>/`.
pub fn folder(root: &Path, source: &str, series: &str, chapter: &str) -> PathBuf {
    root.join(safe(source)).join(safe(series)).join(safe(chapter))
}

/// The files of a downloaded chapter, in page order; empty when nothing is kept.
pub fn pages(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().filter_map(|entry| entry.ok().map(|entry| entry.path())).filter(|path| path.extension().is_some_and(|ext| ext != "txt")).collect();
    files.sort();
    files
}

/// Removes folders left half-written by an interrupted download.
pub fn clear_unfinished(root: &Path) {
    fn walk(dir: &Path) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "part") {
                let _ = std::fs::remove_dir_all(&path);
            } else if path.is_dir() {
                walk(&path);
            }
        }
    }
    walk(root);
}

/// Fetches every image (http or https only, each capped in size) with the headers the source needs and writes
/// them as `0000.ext`, `0001.ext`… into a `.part` folder that replaces `folder` only when all are in, so a stop
/// half way leaves nothing readable and a failed redownload leaves the old copy. Calls `progress(done)` after
/// each image and returns the bytes written.
pub async fn fetch(client: &reqwest::Client, images: &[String], headers: &[(String, String)], folder: &Path, mut progress: impl FnMut(u32)) -> anyhow::Result<u64> {
    let staging = folder.with_extension("part");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let result = async {
        let mut total = 0;
        for (index, url) in images.iter().enumerate() {
            anyhow::ensure!(url.starts_with("http://") || url.starts_with("https://"), "{url} is not a web address");
            let mut request = client.get(url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            let mut response = request.send().await.with_context(|| format!("cannot reach {url}"))?.error_for_status().with_context(|| format!("{url} did not answer"))?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                anyhow::ensure!(bytes.len() + chunk.len() <= MAX_IMAGE, "{url} is larger than a page should be");
                bytes.extend_from_slice(&chunk);
            }
            let ext = Path::new(url.split('?').next().unwrap_or(url)).extension().and_then(|ext| ext.to_str()).filter(|ext| ext.len() <= 4 && ext.bytes().all(|b| b.is_ascii_alphanumeric())).unwrap_or("jpg");
            std::fs::write(staging.join(format!("{index:04}.{ext}")), &bytes)?;
            total += bytes.len() as u64;
            progress(index as u32 + 1);
        }
        let _ = std::fs::remove_dir_all(folder);
        std::fs::rename(&staging, folder)?;
        anyhow::Ok(total)
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Answers each request with the page's path as its body, and 404 for `/missing`.
    async fn serve() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    let read = socket.read(&mut buffer).await.unwrap();
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let (status, body) = if path == "/missing" { ("404 Not Found", String::new()) } else { ("200 OK", path) };
                    let _ = socket.write_all(format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
                });
            }
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn a_chapter_is_kept_page_by_page_and_a_failure_leaves_nothing() {
        let base = serve().await;
        let root = std::env::temp_dir().join(format!("aki-dl-{}", std::process::id()));
        let client = reqwest::Client::new();
        let folder = folder(&root, "mangadex", "a/b", "c1");
        assert_ne!(self::folder(&root, "m", "a/b", "c"), self::folder(&root, "m", "a_b", "c"));
        assert!(self::folder(&root, "..", "", ".").starts_with(&root));
        let mut seen = Vec::new();
        let images = vec![format!("{base}/1.jpg"), format!("{base}/2.png?token=x")];
        let bytes = fetch(&client, &images, &[], &folder, |done| seen.push(done)).await.unwrap();
        assert_eq!((bytes, seen), (("/1.jpg".len() + "/2.png?token=x".len()) as u64, vec![1, 2]));
        let names: Vec<String> = pages(&folder).iter().map(|page| page.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["0000.jpg", "0001.png"]);

        let broken = self::folder(&root, "mangadex", "a/b", "c2");
        assert!(fetch(&client, &[format!("{base}/1.jpg"), format!("{base}/missing")], &[], &broken, |_| {}).await.is_err());
        assert!(!broken.exists() && !broken.with_extension("part").exists());
        assert!(fetch(&client, &["file:///etc/passwd".to_string()], &[], &broken, |_| {}).await.unwrap_err().to_string().contains("not a web address"));
        // A failed redownload leaves the good copy.
        assert!(fetch(&client, &[format!("{base}/missing")], &[], &folder, |_| {}).await.is_err());
        assert_eq!(pages(&folder).len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
