//! Keeping a chapter's images on the device so it reads without a network.

use anyhow::Context;
use std::path::{Path, PathBuf};

/// Where a chapter's pages are kept: `root/<source>/<series>/<chapter>/`, each part made safe as a folder name.
pub fn folder(root: &Path, source: &str, series: &str, chapter: &str) -> PathBuf {
    let safe = |part: &str| part.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect::<String>();
    root.join(safe(source)).join(safe(series)).join(safe(chapter))
}

/// The files of a downloaded chapter, in page order; empty when nothing is kept.
pub fn pages(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().filter_map(|entry| entry.ok().map(|entry| entry.path())).collect();
    files.sort();
    files
}

/// Fetches every image with the headers the source needs and writes them as `0000.ext`, `0001.ext`…
/// Calls `progress(done)` after each. Returns the bytes written. A half-written chapter is removed on failure.
pub async fn fetch(client: &reqwest::Client, images: &[String], headers: &[(String, String)], folder: &Path, mut progress: impl FnMut(u32)) -> anyhow::Result<u64> {
    std::fs::create_dir_all(folder)?;
    let result = async {
        let mut total = 0;
        for (index, url) in images.iter().enumerate() {
            let mut request = client.get(url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            let response = request.send().await.with_context(|| format!("cannot reach {url}"))?.error_for_status().with_context(|| format!("{url} did not answer"))?;
            let bytes = response.bytes().await?;
            let ext = Path::new(url.split('?').next().unwrap_or(url)).extension().and_then(|ext| ext.to_str()).filter(|ext| ext.len() <= 4).unwrap_or("jpg");
            std::fs::write(folder.join(format!("{index:04}.{ext}")), &bytes)?;
            total += bytes.len() as u64;
            progress(index as u32 + 1);
        }
        anyhow::Ok(total)
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(folder);
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
        let mut seen = Vec::new();
        let images = vec![format!("{base}/1.jpg"), format!("{base}/2.png?token=x")];
        let bytes = fetch(&client, &images, &[], &folder, |done| seen.push(done)).await.unwrap();
        assert_eq!((bytes, seen), (("/1.jpg".len() + "/2.png?token=x".len()) as u64, vec![1, 2]));
        let names: Vec<String> = pages(&folder).iter().map(|page| page.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["0000.jpg", "0001.png"]);

        let broken = self::folder(&root, "mangadex", "a/b", "c2");
        assert!(fetch(&client, &[format!("{base}/1.jpg"), format!("{base}/missing")], &[], &broken, |_| {}).await.is_err());
        assert!(!broken.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
