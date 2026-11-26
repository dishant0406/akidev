//! Loads poster and banner images for GPUI.
//!
//! GPUI's default HTTP client fails every request on desktop, so this one fetches with
//! reqwest. Responses are kept on disk, because GPUI's own cache lives in memory only and
//! every launch would otherwise download every poster again.

use anyhow::Context as _;
use futures::future::BoxFuture;
use gpui_kit::http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Cached images not touched for this long are removed at startup.
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

pub struct ImageClient {
    dir: PathBuf,
}

impl ImageClient {
    pub fn new(dir: PathBuf) -> ImageClient {
        let _ = std::fs::create_dir_all(&dir);
        remove_stale(&dir);
        ImageClient { dir }
    }
}

fn remove_stale(dir: &PathBuf) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let age = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok());
        // Files from before images were shrunk have no suffix, and are only taking room.
        let unshrunk = !entry.file_name().to_string_lossy().ends_with("-s");
        if unshrunk || age.is_some_and(|age| age > MAX_AGE) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

impl HttpClient for ImageClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = request.uri().to_string();
        let dir = self.dir.clone();
        Box::pin(async move {
            let bytes = crate::runtime::run(fetch(url.clone(), dir))
                .await
                .context("the image task failed")?
                .inspect_err(|error| tracing::warn!("[images] {url}: {error:#}"))?;
            Ok(Response::builder()
                .status(200)
                .body(AsyncBody::from_bytes(bytes.into()))?)
        })
    }
}

/// The longest sides an image is kept at: twice the size it is drawn at, for a sharp picture on
/// a screen with twice the pixels. A poster is drawn at 176 by 264, a banner at the window's width.
const POSTER_LIMIT: (u32, u32) = (352, 528);
const BANNER_LIMIT: (u32, u32) = (1600, 900);

/// `bytes` as a picture no bigger than it needs to be. What cannot be read, or is already small
/// enough, or would not get smaller, is returned as it is.
fn shrink(bytes: Vec<u8>) -> Vec<u8> {
    use image::{DynamicImage, ImageEncoder as _, codecs::jpeg::JpegEncoder, imageops::FilterType};
    let Ok(picture) = image::load_from_memory(&bytes) else {
        return bytes;
    };
    let (width, height) = (picture.width(), picture.height());
    let (wide, tall) = if height > width {
        POSTER_LIMIT
    } else {
        BANNER_LIMIT
    };
    // A picture with see-through parts would lose them as a JPEG.
    if (width <= wide && height <= tall) || picture.color().has_alpha() {
        return bytes;
    }
    let small = DynamicImage::ImageRgb8(picture.resize(wide, tall, FilterType::Lanczos3).to_rgb8());
    let mut kept = Vec::new();
    let encoded = JpegEncoder::new_with_quality(&mut kept, 88).write_image(
        small.as_bytes(),
        small.width(),
        small.height(),
        small.color().into(),
    );
    if encoded.is_ok() && kept.len() < bytes.len() {
        kept
    } else {
        bytes
    }
}

/// How many pictures are fetched at once. A page of posters asks for a hundred together, and a
/// provider that is asked that fast answers some of them with an error.
const AT_ONCE: usize = 8;
/// Tries at one picture, and how long to wait before each retry.
const TRIES: usize = 3;
const BACKOFF: Duration = Duration::from_millis(500);

fn slots() -> &'static tokio::sync::Semaphore {
    static SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| tokio::sync::Semaphore::new(AT_ONCE))
}

/// Whether these bytes start like a picture. A provider that is throttling can answer with a page
/// of text and a success status; keeping that would leave the tile blank for good.
fn is_picture(bytes: &[u8]) -> bool {
    bytes.len() > 64 && image::guess_format(bytes).is_ok()
}

/// The picture at `url`, tried again when the provider is busy or the connection drops.
async fn download(url: &str) -> anyhow::Result<Vec<u8>> {
    let _slot = slots().acquire().await?;
    let mut last = None;
    for attempt in 0..TRIES {
        if attempt > 0 {
            tokio::time::sleep(BACKOFF * 2u32.pow(attempt as u32 - 1)).await;
        }
        let sent = akidev_core::http::client().get(url).send().await;
        match sent.and_then(|response| response.error_for_status()) {
            Ok(response) => match response.bytes().await {
                Ok(bytes) if is_picture(&bytes) => return Ok(bytes.to_vec()),
                Ok(_) => last = Some(anyhow::anyhow!("the answer is not a picture")),
                Err(error) => last = Some(error.into()),
            },
            Err(error) => {
                // A missing picture will not come back; a busy provider or a dropped connection may.
                let retry = error
                    .status()
                    .is_none_or(|status| status.is_server_error() || status.as_u16() == 429);
                last = Some(error.into());
                if !retry {
                    break;
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("no answer")))
}

async fn fetch(url: String, dir: PathBuf) -> anyhow::Result<Vec<u8>> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let path = dir.join(format!("{:016x}-s", hasher.finish()));

    if let Ok(bytes) = tokio::fs::read(&path).await {
        if is_picture(&bytes) {
            return Ok(bytes);
        }
        // Not a picture (an earlier answer that was not one): it would fail on every launch.
        let _ = tokio::fs::remove_file(&path).await;
    }
    let bytes = download(&url).await?;
    // Shrunk before it is kept, so what is decoded, held and drawn is no bigger than it is shown.
    let bytes = tokio::task::spawn_blocking(move || shrink(bytes)).await?;
    // Written to a temporary name first, so a half-written file is never read back.
    let partial = path.with_extension("part");
    if tokio::fs::write(&partial, &bytes).await.is_ok() {
        let _ = tokio::fs::rename(&partial, &path).await;
    }
    Ok(bytes)
}

/// How much decoded picture is kept, in bytes, once more than `ALWAYS_KEPT` are held. A poster is
/// about 0.7 MB decoded and a banner about 5.7 MB, so a count alone would let the banners of the
/// titles visited pile up. What is not drawn for a while comes back from the disk copy, which is
/// quick, instead of staying decoded in memory and on the graphics card for as long as the app runs.
const BUDGET: usize = 40 * 1024 * 1024;

/// A picture drawn this recently is never dropped, so what is on screen does not flicker or get
/// decoded again every frame, however many there are.
const IN_USE: std::time::Duration = std::time::Duration::from_secs(2);

/// Decoded pictures, with the least recently drawn dropped past `BUDGET`. GPUI's default keeps
/// every picture it has ever drawn.
pub struct PictureCache {
    items: std::collections::HashMap<gpui_kit::Resource, Entry>,
    loads: u64,
}

struct Entry {
    item: gpui_kit::ImageCacheItem,
    /// When it was last drawn.
    drawn: std::time::Instant,
    /// When loading it failed, so it is asked for again after a while instead of staying blank
    /// until the app is restarted.
    failed: Option<std::time::Instant>,
}

/// How long a picture that failed to load is left alone before the next draw asks for it again.
const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(6);

fn weight(item: &gpui_kit::ImageCacheItem) -> usize {
    match item.get() {
        Some(Ok(image)) => image.as_bytes(0).map_or(0, <[u8]>::len),
        _ => 0,
    }
}

impl PictureCache {
    pub fn new(cx: &mut gpui_kit::App) -> gpui_kit::Entity<PictureCache> {
        use gpui_kit::AppContext as _;
        let cache = cx.new(|_| PictureCache {
            items: Default::default(),
            loads: 0,
        });
        cx.observe_release(&cache, |cache, cx| {
            for (_, entry) in std::mem::take(&mut cache.items) {
                if let Some(Ok(image)) = entry.item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        cache
    }

    /// Drops the pictures drawn longest ago until what is left fits the budget.
    fn trim(&mut self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
        let mut total: usize = self.items.values().map(|entry| weight(&entry.item)).sum();
        if total <= BUDGET {
            return;
        }
        let mut by_age: Vec<_> = self
            .items
            .iter()
            .filter(|(_, entry)| entry.drawn.elapsed() > IN_USE)
            .map(|(key, entry)| (entry.drawn, key.clone()))
            .collect();
        by_age.sort_unstable_by_key(|(at, _)| *at);
        for (_, key) in by_age {
            if total <= BUDGET {
                break;
            }
            if let Some(entry) = self.items.remove(&key) {
                total -= weight(&entry.item);
                if let Some(Ok(image)) = entry.item.get() {
                    cx.drop_image(image, Some(window));
                }
            }
        }
    }
}

impl gpui_kit::ImageCache for PictureCache {
    fn load(
        &mut self,
        resource: &gpui_kit::Resource,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) -> Option<Result<std::sync::Arc<gpui_kit::RenderImage>, gpui_kit::ImageCacheError>> {
        self.loads += 1;
        let now = std::time::Instant::now();
        // A picture that failed is asked for again once it has been left alone a while.
        if self
            .items
            .get(resource)
            .is_some_and(|entry| entry.failed.is_some_and(|at| at.elapsed() >= RETRY_AFTER))
        {
            self.items.remove(resource);
        }
        let new = !self.items.contains_key(resource);
        let entry = self.items.entry(resource.clone()).or_insert_with(|| Entry {
            item: gpui_kit::ImageCacheItem::new(resource, cx),
            drawn: now,
            failed: None,
        });
        entry.drawn = now;
        let loaded = entry.item.use_image(window);
        if matches!(loaded, Some(Err(_))) && entry.failed.is_none() {
            entry.failed = Some(now);
            // Nothing else would draw it again, so ask for a frame when it is time to retry.
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(RETRY_AFTER).await;
                    let _ = cx.update(|window, _| window.refresh());
                })
                .detach();
        }
        // Checked when a picture is added, and now and then as they finish loading.
        if new || self.loads.is_multiple_of(256) {
            self.trim(window, cx);
        }
        loaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bytes_that_start_like_a_picture_count_as_one() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.resize(200, 0);
        assert!(is_picture(&png));
        assert!(!is_picture(b"<html>Too many requests, slow down</html>"));
        assert!(!is_picture(&[]));
        let mut text = b"{\"error\":\"rate limited\"}".to_vec();
        text.resize(200, b' ');
        assert!(!is_picture(&text));
    }

    #[tokio::test]
    async fn a_kept_file_that_is_not_a_picture_is_thrown_away_instead_of_failing_for_ever() {
        let dir = std::env::temp_dir().join(format!("akidev-images-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Nothing listens at this address, so after the bad file is dropped the fetch fails fast.
        let url = "http://127.0.0.1:1/poster.jpg".to_string();
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let path = dir.join(format!("{:016x}-s", hasher.finish()));
        std::fs::write(&path, b"<html>blocked</html>").unwrap();
        assert!(fetch(url, dir.clone()).await.is_err());
        assert!(
            !path.exists(),
            "the bad file is removed so the next try starts clean"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
