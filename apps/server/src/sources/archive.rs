//! The Internet Archive, as a last-resort source.
//!
//! It is not a torrent index and not a debrid service: it is a public library that stores the
//! file and serves it over plain HTTPS. It is here for one measured reason: it is the only place a
//! regional series can be found when no torrent indexer has it at all. Its own h264 derivative has
//! real range support, so this source joins no swarm and converts nothing. A bounded media head
//! must pass local inspection before a direct URL is returned; playback bytes go to the viewer.
//!
//! It is tried last, so it only ever answers when nothing else can.

use akidev_core::cache;
use akidev_core::http::{USER_AGENT, read_capped};
use akidev_core::releases::{is_bonus, parse_episode, parse_season};
use anyhow::{Context, bail};
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

const TTL: Duration = Duration::from_secs(30 * 60);
/// Maximum unique matching metadata probes across the entire lookup.
const MAX_ITEMS: usize = 6;
/// How many title spellings are searched. The bare-title forms come first.
const MAX_QUERIES: usize = 5;
/// Formats archive.org derives from an upload, best first. A derivative beats the original
/// because the original may be a format no browser plays, while these are h264 in MP4.
const PLAYABLE_FORMATS: [&str; 6] = [
    "h.264 IA",
    "MPEG4",
    "h.264",
    "512Kb MPEG4",
    "Ogg Video",
    "WebM",
];
/// The shortest a file can be and still be an episode rather than a clip. Measured on the items
/// this source returns: real episodes are 2000s and up, the clip was 74s.
const MIN_EPISODE_SECONDS: f64 = 300.;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SECONDS: f64 = 12. * 60. * 60.;
const MEDIA_HEAD_BYTES: usize = 8 * 1024 * 1024;
const PROBE_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(15);

// The endpoint is administrator/configuration-pinned, never a playback request URL.
// Do not follow upstream redirects to destinations outside that configured origin.
static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Archive HTTP client builds")
});

// Only evidence/encoding suffixes are discarded. A bare sequel number and arbitrary
// uploader prose (commentary, reactions, returns, etc.) remain part of the identity.
static SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"^(?P<title>.+?)(?:(?:season|s)[0-9]{1,2}(?:e(?:p)?[0-9]{1,4})?|(?:episode|ep|e)[0-9]{1,4}|(?:19|20)[0-9]{2}|(?:240|360|480|576|720|1080|1440|2160)p|webdl|webrip|bdrip|bluray|h264|x264|h265|x265|hevc|mpeg4|512kb|aac|mp4|mkv|webm|ogv)*$"
).expect("Archive suffix pattern builds")
});

fn normalized(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn identity(text: &str) -> String {
    let text = normalized(text);
    SUFFIX
        .captures(&text)
        .map(|c| c["title"].to_string())
        .unwrap_or(text)
}

fn path_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 1024
        && !name.starts_with('/')
        && !name.contains(['\\', '%', '?', '#', ':'])
        && !name.chars().any(char::is_control)
        && name
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn extra(name: &str) -> bool {
    static EXTRAS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:sample|trailer|teaser|clip|commentary|reaction|review|recap|promo|preview|bonus|extras?)(?:$|[^a-z0-9])"
    ).expect("Archive extras pattern builds")
    });
    is_bonus(name) || EXTRAS.is_match(name)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveFile {
    pub name: String,
    pub size: u64,
    pub format: String,
    pub seconds: Option<f64>,
}

pub struct Found {
    pub title: String,
    pub url: String,
}

/// Whether a search result is plausibly the title that was asked for. Compared with every
/// non-alphanumeric character removed from both sides, which is what makes "India's Got Latent",
/// "Indias Got Latent" and "India s Got Latent" equal: archive.org stores an item under whichever
/// spelling its uploader used.
#[cfg(test)]
pub fn matches_title(name: &str, titles: &[String]) -> bool {
    catalogue_suffix(name, titles).is_some()
}

/// Match the longest catalogue alias before interpreting evidence. The 2049 in
/// "Blade Runner 2049" is title identity, not a release year.
fn catalogue_suffix(name: &str, titles: &[String]) -> Option<String> {
    if name.is_empty() || name.len() > 1024 || extra(name) {
        return None;
    }
    let name = normalized(name);
    let alias = titles
        .iter()
        .map(|title| normalized(title))
        .filter(|title| !title.is_empty() && title.len() <= 1024 && name.starts_with(title))
        .max_by_key(String::len)?;
    let suffix = name.strip_prefix(&alias)?;
    evidence_suffix(suffix).then(|| suffix.to_string())
}

fn evidence_suffix(suffix: &str) -> bool {
    identity(&format!("a{suffix}")) == "a"
}

fn year_agrees(suffix: &str, expected_year: Option<u32>) -> bool {
    // Consume complete evidence tokens so episode 2026 cannot become year 2026.
    static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:season|s)[0-9]{1,2}(?:e(?:p)?[0-9]{1,4})?|(?:episode|ep|e)[0-9]{1,4}|(?P<year>(?:19|20)[0-9]{2})|(?:240|360|480|576|720|1080|1440|2160)p|webdl|webrip|bdrip|bluray|h264|x264|h265|x265|hevc|mpeg4|512kb|aac|mp4|mkv|webm|ogv")
            .expect("Archive evidence token pattern builds")
    });
    expected_year.is_none_or(|wanted| {
        TOKENS.captures_iter(suffix).all(|token| {
            token
                .name("year")
                .is_none_or(|year| year.as_str().parse::<u32>().ok() == Some(wanted))
        })
    })
}

fn matches_identity(name: &str, titles: &[String], expected_year: Option<u32>) -> bool {
    catalogue_suffix(name, titles).is_some_and(|suffix| year_agrees(&suffix, expected_year))
}

fn has_episode_marker(suffix: &str) -> bool {
    static EPISODE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:episode|ep|e)[0-9]{1,4}").expect("Archive explicit episode pattern builds")
    });
    EPISODE.is_match(suffix)
}

fn matches_document(
    name: &str,
    titles: &[String],
    episode: Option<u32>,
    season: Option<u32>,
    expected_year: Option<u32>,
) -> bool {
    matches_identity(name, titles, expected_year)
        && match episode {
            Some(wanted) => identifies_episode(name, wanted, season) != Some(false),
            None => {
                catalogue_suffix(name, titles).is_some_and(|suffix| !has_episode_marker(&suffix))
            }
        }
}

/// The files worth offering for one item, best first. archive.org lists every derivative beside
/// the original, the thumbnails and its own metadata files; only a playable format with a size is
/// a candidate.
pub fn playable_files(metadata: &Value) -> Vec<ArchiveFile> {
    let mut found: Vec<ArchiveFile> = metadata
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(|file| {
                    let format = file.get("format")?.as_str()?.to_string();
                    let name = file.get("name")?.as_str()?.to_string();
                    // Parse integer sizes as integers: float casts silently saturate/round.
                    let value = file.get("size")?;
                    let size = value
                        .as_u64()
                        .or_else(|| value.as_str()?.parse::<u64>().ok())?;
                    let seconds = match file.get("length") {
                        None => None,
                        Some(value) => {
                            let seconds = value
                                .as_f64()
                                .or_else(|| value.as_str()?.parse::<f64>().ok())?;
                            if !seconds.is_finite() || seconds <= 0. || seconds > MAX_SECONDS {
                                return None;
                            }
                            Some(seconds)
                        }
                    };
                    if size == 0 || !path_ok(&name) || !PLAYABLE_FORMATS.contains(&format.as_str())
                    {
                        return None;
                    }
                    Some(ArchiveFile {
                        name,
                        size,
                        format,
                        seconds,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let rank = |format: &str| {
        PLAYABLE_FORMATS
            .iter()
            .position(|f| *f == format)
            .unwrap_or(usize::MAX)
    };
    found.sort_by(|a, b| {
        rank(&a.format)
            .cmp(&rank(&b.format))
            .then(b.size.cmp(&a.size))
    });
    found
}

/// Whether a file has a finite positive episode-length duration. Unknown duration
/// fails closed: byte size by itself does not establish complete media.
pub fn is_episode_length(file: &ArchiveFile) -> bool {
    plausible_length(file, None)
}

fn plausible_length(file: &ArchiveFile, expected_seconds: Option<f64>) -> bool {
    // Size alone cannot distinguish a high-bitrate trailer from an episode.
    // Require metadata duration as well as independent media inspection.
    let Some(seconds) = file.seconds else {
        return false;
    };
    if file.size == 0 || !seconds.is_finite() || seconds <= 0. || seconds > MAX_SECONDS {
        return false;
    }
    match expected_seconds {
        Some(expected) => {
            expected.is_finite()
                && expected > 0.
                && expected <= MAX_SECONDS
                && seconds >= expected * 0.65
                && seconds <= expected * 1.5
        }
        None => seconds >= MIN_EPISODE_SECONDS,
    }
}

/// `Some(true)` when a name identifies the episode asked for, `Some(false)` when it names a
/// different one, `None` when it names no episode and so cannot be checked. The season is only
/// enforced when one is given: an anime's later seasons restart at episode 1.
pub fn identifies_episode(name: &str, episode: u32, season: Option<u32>) -> Option<bool> {
    static EPISODES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:episode|ep|e)([0-9]{1,4})").expect("Archive episode pattern builds")
    });
    static SEASONS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:season|s)([0-9]{1,2})(?:[^0-9]|$)").expect("Archive season pattern builds")
    });
    let compact = normalized(name);
    // Core parsing returns only the first marker; conflicting explicit markers
    // identify a combined file and cannot be served without an episode offset.
    if EPISODES
        .captures_iter(&compact)
        .any(|c| c[1].parse::<u32>().ok() != Some(episode))
        || season.is_some_and(|wanted| {
            SEASONS
                .captures_iter(&compact)
                .any(|c| c[1].parse::<u32>().ok() != Some(wanted))
        })
    {
        return Some(false);
    }
    if let Some(named) = parse_episode(name) {
        if named != episode {
            return Some(false);
        }
        if let (Some(wanted), Some(named_season)) = (season, parse_season(name))
            && named_season != wanted
        {
            return Some(false);
        }
        return Some(true);
    }
    // No episode number: a season alone can only rule a name out, never confirm it.
    if let (Some(wanted), Some(named_season)) = (season, parse_season(name))
        && named_season != wanted
    {
        return Some(false);
    }
    None
}

/// Picks the file to serve, or `None` when the episode cannot be confirmed. Serving the wrong
/// episode silently is worse than serving nothing, so an item is only accepted when its title or
/// the file itself names the requested episode. That is what stops commentary being served as an
/// episode: a news clip holds one video that names no episode.
#[cfg(test)]
pub fn pick_file<'a>(
    item_title: &str,
    files: &'a [ArchiveFile],
    episode: Option<u32>,
    season: Option<u32>,
) -> Option<&'a ArchiveFile> {
    pick_file_with_runtime(item_title, files, episode, season, None)
}

/// The catalog runtime, when known, is seconds (not minutes). It constrains every
/// candidate before ranking; there is no short/unknown-duration fallback.
pub fn pick_file_with_runtime<'a>(
    item_title: &str,
    files: &'a [ArchiveFile],
    episode: Option<u32>,
    season: Option<u32>,
    expected_seconds: Option<f64>,
) -> Option<&'a ArchiveFile> {
    if files.is_empty()
        || item_title.is_empty()
        || item_title.len() > 1024
        || extra(item_title)
        || episode == Some(0)
        || season == Some(0)
    {
        return None;
    }
    let item_identity = identity(item_title);
    files.iter().find(|file| {
        if !path_ok(&file.name)
            || !PLAYABLE_FORMATS.contains(&file.format.as_str())
            || extra(&file.name)
            || !plausible_length(file, expected_seconds)
        {
            return false;
        }
        let stem = file
            .name
            .rsplit('/')
            .next()
            .unwrap_or("")
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(&file.name);
        let stem = stem.strip_suffix(".ia").unwrap_or(stem);
        let file_identity = identity(stem);
        // Bare evidence-only names are allowed when the item's identity is known.
        // A named file, however, must agree; "Other.Show.S02E05" cannot inherit it.
        let evidence_only = SUFFIX.is_match(&format!("a{}", normalized(stem)))
            && identity(&format!("a{}", normalized(stem))) == "a";
        if !evidence_only && file_identity != item_identity {
            return false;
        }
        match episode {
            None => parse_episode(item_title).is_none() && parse_episode(&file.name).is_none(),
            Some(wanted) => {
                let item_ep = identifies_episode(item_title, wanted, season);
                let file_ep = identifies_episode(&file.name, wanted, season);
                item_ep != Some(false)
                    && file_ep != Some(false)
                    && (item_ep == Some(true) || file_ep == Some(true))
                    && season.is_none_or(|wanted| {
                        parse_season(item_title) == Some(wanted)
                            || parse_season(&file.name) == Some(wanted)
                    })
            }
        }
    })
}

/// Catalogue-relative identity and year checks supplement runtime/episode checks.
/// Missing years require an exact alias plus recognized evidence. Evidence-only
/// filenames inherit the already-confirmed item's identity.
#[allow(clippy::too_many_arguments)]
pub fn pick_file_with_identity<'a>(
    item_title: &str,
    titles: &[String],
    files: &'a [ArchiveFile],
    episode: Option<u32>,
    season: Option<u32>,
    expected_seconds: Option<f64>,
    expected_year: Option<u32>,
) -> Option<&'a ArchiveFile> {
    if episode == Some(0)
        || season == Some(0)
        || expected_year.is_some_and(|n| !(1900..=2099).contains(&n) || episode.is_some())
        || !matches_document(item_title, titles, episode, season, expected_year)
    {
        return None;
    }
    files.iter().find(|file| {
        let stem = file.name.rsplit('/').next().unwrap_or("");
        let stem = stem.rsplit_once('.').map_or(stem, |(stem, _)| stem);
        let stem = stem.strip_suffix(".ia").unwrap_or(stem);
        let compact = normalized(stem);
        let suffix =
            catalogue_suffix(stem, titles).or_else(|| evidence_suffix(&compact).then_some(compact));
        if episode.is_none() {
            return suffix.is_some_and(|suffix| {
                year_agrees(&suffix, expected_year) && !has_episode_marker(&suffix)
            }) && path_ok(&file.name)
                && PLAYABLE_FORMATS.contains(&file.format.as_str())
                && !extra(&file.name)
                && plausible_length(file, expected_seconds);
        }
        suffix.is_some_and(|suffix| year_agrees(&suffix, expected_year))
            && pick_file_with_runtime(
                item_title,
                std::slice::from_ref(*file),
                episode,
                season,
                expected_seconds,
            )
            .is_some()
    })
}

pub fn download_url(base: &str, identifier: &str, name: &str) -> String {
    if !identifier_ok(identifier) || !path_ok(name) {
        return String::new();
    }
    format!(
        "{}/download/{}/{}",
        base.trim_end_matches('/'),
        encode(identifier),
        encode(name)
    )
}

/// Percent-encodes a path segment.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// archive.org identifiers are alphanumeric with dots, dashes and underscores.
fn identifier_ok(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier.len() <= 256
        && !matches!(identifier, "." | "..")
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

async fn search_items(
    base: &str,
    term: &str,
    left: Duration,
) -> anyhow::Result<Vec<(String, String)>> {
    let response = HTTP
        .get(format!("{base}/advancedsearch.php"))
        .query(&[
            (
                "q",
                format!(
                    "title:(\"{}\") AND mediatype:movies",
                    term.replace('\\', "\\\\").replace('"', "\\\"")
                ),
            ),
            // `fl[]` repeated: the indexed form is accepted silently and returns rows with no
            // identifier, which made a matched title look like nothing found.
            ("fl[]", "identifier".to_string()),
            ("fl[]", "title".to_string()),
            ("rows", (MAX_ITEMS * 3).to_string()),
            ("output", "json".to_string()),
        ])
        .timeout(left)
        .send()
        .await?
        .error_for_status()?;
    let data: Value = serde_json::from_slice(&read_capped(response, MAX_RESPONSE_BYTES).await?)?;
    let docs = data
        .pointer("/response/docs")
        .and_then(Value::as_array)
        .context("not a search response")?;
    Ok(docs
        .iter()
        .take(MAX_ITEMS * 3)
        .filter_map(|doc| {
            let identifier = doc.get("identifier")?.as_str()?;
            let title = doc.get("title")?.as_str()?;
            (identifier_ok(identifier) && !title.is_empty() && title.len() <= 1024)
                .then(|| (identifier.to_string(), title.to_string()))
        })
        .collect())
}

async fn item_metadata(base: &str, identifier: &str, left: Duration) -> anyhow::Result<Value> {
    let response = HTTP
        .get(format!("{base}/metadata/{}", encode(identifier)))
        .timeout(left)
        .send()
        .await?
        .error_for_status()?;
    let data: Value = serde_json::from_slice(&read_capped(response, MAX_RESPONSE_BYTES).await?)?;
    if !data.is_object() {
        bail!("not a metadata response");
    }
    Ok(data)
}

/// Inspect bytes from the configuration-pinned origin, never an upstream-selected
/// URL. Redirects fail closed, and ffprobe can access only its stdin pipe.
async fn verify_media(
    base: &str,
    identifier: &str,
    file: &ArchiveFile,
    expected_seconds: Option<f64>,
    deadline: Instant,
) -> anyhow::Result<f64> {
    let budget = deadline
        .saturating_duration_since(Instant::now())
        .min(VERIFY_TIMEOUT);
    let inspect = async {
        let url = download_url(base, identifier, &file.name);
        if url.is_empty() || !plausible_length(file, expected_seconds) {
            bail!("invalid media candidate");
        }
        let wanted = file.size.min(MEDIA_HEAD_BYTES as u64);
        let response = HTTP
            .get(url)
            .header(reqwest::header::RANGE, format!("bytes=0-{}", wanted - 1))
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .timeout(budget)
            .send()
            .await?;
        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => {
                let expected = format!("bytes 0-{}/{}", wanted - 1, file.size);
                if response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|h| h.to_str().ok())
                    != Some(expected.as_str())
                {
                    bail!("invalid media range");
                }
            }
            reqwest::StatusCode::OK if file.size <= MEDIA_HEAD_BYTES as u64 => {}
            _ => bail!("media response cannot be inspected"),
        }
        if response
            .headers()
            .contains_key(reqwest::header::CONTENT_ENCODING)
        {
            bail!("encoded media response");
        }
        let head = read_capped(response, MEDIA_HEAD_BYTES).await?;
        if head.len() as u64 != wanted {
            bail!("incomplete media head");
        }
        let mut child = Command::new("ffprobe")
            .args(["-v", "error", "-protocol_whitelist", "pipe", "-probesize", "8388608",
                "-analyzeduration", "5000000",
                "-show_entries", "format=format_name,duration:stream=codec_type,codec_name,profile,pix_fmt,width,height,channels:stream_disposition=attached_pic:frame=media_type",
                "-show_format", "-show_streams", "-show_frames", "-of", "json", "-i", "pipe:0"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .kill_on_drop(true).spawn().context("media inspector unavailable")?;
        let mut stdin = child.stdin.take().context("inspector input unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("inspector output unavailable")?;
        let feed = async move {
            if let Err(error) = stdin.write_all(&head).await
                && error.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(error);
            }
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let read = async move {
            let mut output = Vec::new();
            stdout
                .take(PROBE_OUTPUT_BYTES + 1)
                .read_to_end(&mut output)
                .await?;
            if output.len() as u64 > PROBE_OUTPUT_BYTES {
                bail!("media inspection too large");
            }
            anyhow::Ok(output)
        };
        let stderr = child
            .stderr
            .take()
            .context("inspector errors unavailable")?;
        let errors = async move {
            let mut output = Vec::new();
            stderr
                .take(PROBE_OUTPUT_BYTES + 1)
                .read_to_end(&mut output)
                .await?;
            if output.len() as u64 > PROBE_OUTPUT_BYTES {
                bail!("media inspection too large");
            }
            anyhow::Ok(output)
        };
        let (_, output, errors, status) = tokio::try_join!(
            async { feed.await.map_err(anyhow::Error::from) },
            read,
            errors,
            async { child.wait().await.map_err(anyhow::Error::from) }
        )?;
        if !status.success() || !errors.is_empty() {
            bail!("media inspection failed");
        }
        let text = std::str::from_utf8(&output)?;
        let probe = crate::media::parse_probe(text)?;
        let raw: Value = serde_json::from_str(text)?;
        let decoded_video = raw
            .get("frames")
            .and_then(Value::as_array)
            .is_some_and(|frames| {
                frames
                    .iter()
                    .any(|f| f.get("media_type").and_then(Value::as_str) == Some("video"))
            });
        if !decoded_video
            || !probe.video.as_ref().is_some_and(|v| {
                v.width.is_some_and(|n| n > 0 && n <= 16384)
                    && v.height.is_some_and(|n| n > 0 && n <= 16384)
            })
        {
            bail!("no playable video in media head");
        }
        // Archive metadata describes a derivative, not the bytes we received. This path
        // returns the original URL and has no remux/transcode stage, so require the
        // shared portable baseline even if a particular viewer supports HEVC.
        if crate::media::decide(&probe, false) != crate::media::Delivery::Direct {
            bail!("media is not portable direct playback");
        }
        let seconds = probe.duration.context("media duration unavailable")?;
        let inspected = ArchiveFile {
            seconds: Some(seconds),
            ..file.clone()
        };
        if !plausible_length(&inspected, expected_seconds)
            || !plausible_length(&inspected, file.seconds)
        {
            bail!("media duration mismatch");
        }
        Ok(seconds)
    };
    tokio::time::timeout(budget, inspect)
        .await
        .context("media inspection timed out")?
}

/// Looks the title up directly rather than choosing among indexer results, which is what lets it
/// answer when the indexers found nothing at all.
#[cfg(test)]
pub async fn find(
    base: &str,
    titles: &[String],
    terms: &[String],
    episode: Option<u32>,
    season: Option<u32>,
    deadline: Instant,
) -> Result<Found, String> {
    find_with_runtime(base, titles, terms, episode, season, None, deadline).await
}

/// As `find`, with optional catalog runtime in seconds. `base` must only come
/// from trusted administrator configuration, never from a request parameter.
#[cfg(test)]
pub async fn find_with_runtime(
    base: &str,
    titles: &[String],
    terms: &[String],
    episode: Option<u32>,
    season: Option<u32>,
    expected_seconds: Option<f64>,
    deadline: Instant,
) -> Result<Found, String> {
    find_with_identity(
        base,
        titles,
        terms,
        episode,
        season,
        expected_seconds,
        None,
        deadline,
    )
    .await
}

/// As `find_with_runtime`, retaining the catalogue film year as identity evidence.
/// Supply `None` for series or when the catalogue release year is unknown.
#[allow(clippy::too_many_arguments)]
pub async fn find_with_identity(
    base: &str,
    titles: &[String],
    terms: &[String],
    episode: Option<u32>,
    season: Option<u32>,
    expected_seconds: Option<f64>,
    expected_year: Option<u32>,
    deadline: Instant,
) -> Result<Found, String> {
    let endpoint =
        reqwest::Url::parse(base).map_err(|_| "Invalid Archive configuration.".to_string())?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path() != "/"
    {
        return Err("Invalid Archive configuration.".into());
    }
    if titles.is_empty()
        || titles.len() > 32
        || terms.len() > 64
        || titles
            .iter()
            .chain(terms)
            .any(|s| s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
        || episode == Some(0)
        || season == Some(0)
        || episode.is_some_and(|n| n > 9999)
        || season.is_some_and(|n| n > 99)
        || expected_seconds.is_some_and(|n| !n.is_finite() || n <= 0. || n > MAX_SECONDS)
        || expected_year.is_some_and(|n| !(1900..=2099).contains(&n) || episode.is_some())
    {
        return Err("Invalid Archive lookup.".into());
    }
    let base = endpoint.as_str().trim_end_matches('/');
    tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        find_inner(
            base,
            titles,
            terms,
            episode,
            season,
            expected_seconds,
            expected_year,
            deadline,
        ),
    )
    .await
    .map_err(|_| "Archive lookup timed out.".to_string())?
}

#[allow(clippy::too_many_arguments)]
async fn find_inner(
    base: &str,
    titles: &[String],
    terms: &[String],
    episode: Option<u32>,
    season: Option<u32>,
    expected_seconds: Option<f64>,
    expected_year: Option<u32>,
    deadline: Instant,
) -> Result<Found, String> {
    let left = || deadline.saturating_duration_since(Instant::now());

    // archive.org's phrase search matches the whole title and its items are named after the show,
    // so searching the show name and reading the episode out of the results is both more accurate
    // and far fewer requests than searching per episode. Every spelling is kept: each apostrophe
    // spelling returns a different set of items, and the union is what finds the episodes.
    let mut candidates: Vec<String> = Vec::new();
    for query in titles.iter().chain(terms) {
        if !query.is_empty() && !candidates.contains(query) {
            candidates.push(query.clone());
        }
    }
    // A title-only form has no digits at all.
    let (bare, rest): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|query| !query.chars().any(|c| c.is_ascii_digit()));
    let queries: Vec<String> = bare.into_iter().chain(rest).take(MAX_QUERIES).collect();
    if queries.is_empty() {
        return Err("No title to look up on archive.org.".into());
    }

    let mut failures: Vec<String> = Vec::new();
    // Every (item, file) pair that identifies the episode, so the best can be chosen and not
    // merely the first found.
    let mut found: Vec<(String, String, ArchiveFile)> = Vec::new();
    let mut probed = HashSet::new();

    for query in &queries {
        if left().is_zero() {
            break;
        }
        let docs = cache::cached(
            &format!("archive-org:v2:search:{base}:{query}"),
            |docs: &Vec<(String, String)>| {
                if docs.is_empty() {
                    cache::EMPTY_TTL
                } else {
                    TTL
                }
            },
            || search_items(base, query, left()),
        )
        .await;
        let docs = match docs {
            Ok(docs) => docs,
            Err(error) => {
                failures.push(format!("{query}: {error:#}"));
                continue;
            }
        };

        // Every item is checked: returning on the first match served a 74-second clip for an
        // episode whose real upload was listed second.
        for (identifier, title) in docs
            .iter()
            .filter(|(_, title)| matches_document(title, titles, episode, season, expected_year))
        {
            if left().is_zero() {
                failures.push("deadline reached".into());
                break;
            }
            if probed.contains(identifier) {
                continue;
            }
            if probed.len() >= MAX_ITEMS {
                break;
            }
            probed.insert(identifier.clone());
            let metadata = cache::cached(
                &format!("archive-org:v2:item:{base}:{identifier}"),
                |_| TTL,
                || item_metadata(base, identifier, left()),
            )
            .await;
            let metadata = match metadata {
                Ok(metadata) => metadata,
                Err(error) => {
                    failures.push(format!("{identifier}: {error:#}"));
                    continue;
                }
            };
            // Search indexing can lag or disagree with item metadata. A stated
            // metadata title is additional evidence, not permission to override it.
            if let Some(metadata_title) = metadata.pointer("/metadata/title") {
                let Some(metadata_title) = metadata_title.as_str() else {
                    continue;
                };
                if !matches_document(metadata_title, titles, episode, season, expected_year) {
                    continue;
                }
            }
            let files = playable_files(&metadata);
            if let Some(file) = pick_file_with_identity(
                title,
                titles,
                &files,
                episode,
                season,
                expected_seconds,
                expected_year,
            ) {
                found.push((identifier.clone(), title.clone(), file.clone()));
            }
        }

        // Stop once a real episode has been found. A clip does not stop the search.
        if found.iter().any(|(_, _, file)| is_episode_length(file)) || probed.len() >= MAX_ITEMS {
            break;
        }
    }

    // Selection already enforced identity and runtime. Never promote a rejected clip.
    let chosen = found.iter().max_by(|a, b| {
        a.2.seconds
            .unwrap_or(0.)
            .total_cmp(&b.2.seconds.unwrap_or(0.))
            .then(a.2.size.cmp(&b.2.size))
    });
    if let Some((identifier, title, file)) = chosen
        && verify_media(base, identifier, file, expected_seconds, deadline)
            .await
            .is_ok()
    {
        return Ok(Found {
            title: title.clone(),
            url: download_url(base, identifier, &file.name),
        });
    }
    if !failures.is_empty() {
        // Do not log query text, endpoint URLs or upstream error bodies.
        tracing::warn!(failures = failures.len(), "Archive lookup failed");
    }
    Err("No verified file on archive.org.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(name: &str) -> ArchiveFile {
        ArchiveFile {
            name: name.into(),
            size: 400 << 20,
            format: "h.264 IA".into(),
            seconds: Some(1400.),
        }
    }

    #[test]
    fn three_spellings_of_one_title_are_the_same_title() {
        let titles = vec!["India's Got Latent".to_string()];
        assert!(matches_title("INDIA S GOT LATENT S 2 EP 6", &titles));
        assert!(matches_title("Indias Got Latent (2026)", &titles));
        assert!(!matches_title("Some Other Show", &titles));
        assert!(!matches_title("anything", &[]));
    }

    #[test]
    fn only_a_playable_format_with_a_size_is_offered_best_first() {
        let metadata = json!({ "files": [
            { "name": "a.mkv", "format": "Matroska", "size": "100" },
            { "name": "a.mp4", "format": "MPEG4", "size": "300", "length": "1400" },
            { "name": "a_512kb.mp4", "format": "512Kb MPEG4", "size": "50" },
            { "name": "a.ia.mp4", "format": "h.264 IA", "size": "200" },
            { "name": "empty.mp4", "format": "MPEG4", "size": "0" },
            { "name": "thumb.jpg", "format": "JPEG", "size": "9" },
        ]});
        let names: Vec<String> = playable_files(&metadata)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["a.ia.mp4", "a.mp4", "a_512kb.mp4"]);
    }

    #[test]
    fn a_clip_is_not_an_episode() {
        let mut clip = file("clip.mp4");
        clip.seconds = Some(74.);
        assert!(!is_episode_length(&clip));
        assert!(is_episode_length(&file("ep.mp4")));
        let mut unknown = file("x.mp4");
        unknown.seconds = None;
        unknown.size = 6 << 20;
        assert!(!is_episode_length(&unknown));
        unknown.size = 188 << 20;
        assert!(!is_episode_length(&unknown));
    }

    #[test]
    fn a_name_that_names_another_episode_is_ruled_out() {
        assert_eq!(identifies_episode("Show S 2 EP 5", 5, None), Some(true));
        assert_eq!(identifies_episode("Show EP 6", 5, None), Some(false));
        assert_eq!(identifies_episode("Show S 1 EP 5", 5, Some(2)), Some(false));
        assert_eq!(identifies_episode("Show", 5, None), None);
        assert_eq!(identifies_episode("Show S 1", 5, Some(2)), Some(false));
    }

    #[test]
    fn an_item_that_names_no_episode_needs_a_file_that_does() {
        let files = vec![file("news_package.mp4")];
        // One video proves nothing about which episode it is.
        assert!(pick_file("India's Got Latent Row", &files, Some(5), None).is_none());
        let files = vec![file("Indias_Got_Latent_ep_5.mp4")];
        assert!(pick_file("India's Got Latent", &files, Some(5), None).is_some());
    }

    #[test]
    fn an_item_for_the_right_episode_skips_mislabelled_files() {
        let files = vec![file("show_ep_6.mp4"), file("show_ep_5.mp4")];
        let picked = pick_file("Show EP 5", &files, Some(5), None).unwrap();
        assert_eq!(picked.name, "show_ep_5.mp4");
        assert!(pick_file("Show EP 6", &files[..1], Some(5), None).is_none());
    }

    #[test]
    fn a_bonus_is_not_the_episode_it_is_numbered_as() {
        let files = vec![file("bonus_ep_4.mp4")];
        assert!(pick_file("Show Bonus Ep 04", &files, Some(4), None).is_none());
    }

    #[test]
    fn a_film_takes_the_best_file() {
        let files = vec![file("film.ia.mp4"), file("film.mp4")];
        assert_eq!(
            pick_file("Film", &files, None, None).unwrap().name,
            "film.ia.mp4"
        );
    }

    #[test]
    fn the_download_url_is_encoded() {
        assert!(download_url("https://archive.org", "..", "video.mp4").is_empty());
        assert!(download_url("https://archive.org", "item", "../video.mp4").is_empty());
        assert_eq!(
            download_url("https://archive.org", "igl-s2", "IGL EP 5 (x).mp4"),
            "https://archive.org/download/igl-s2/IGL%20EP%205%20%28x%29.mp4"
        );
    }

    #[test]
    fn fail_closed_public_api_regressions() {
        let titles = vec!["Show".into()];
        for name in [
            "Show commentary EP 5",
            "Show 2 EP 5",
            "Show returns EP 5",
            "Not Show EP 5",
        ] {
            assert!(!matches_title(name, &titles), "{name}");
        }
        assert!(!matches_title("Show", &[]));
        assert!(matches_title("Show (2026) S02E05 [1080p]", &titles));
        let mut clip = file("Show S02E05.mp4");
        clip.seconds = Some(74.);
        assert!(pick_file("Show S02E05", &[clip], Some(5), Some(2)).is_none());
        assert!(pick_file("Show EP 5", &[file("Show EP 5.mp4")], Some(5), Some(2)).is_none());
        assert!(
            pick_file(
                "Show S02E05",
                &[file("Show S02E05 sample.mp4")],
                Some(5),
                Some(2)
            )
            .is_none()
        );
        assert!(pick_file("Show S02E05", &[file("Other S02E05.mp4")], Some(5), Some(2)).is_none());
    }

    #[test]
    fn invalid_metadata_numbers_and_paths_are_rejected() {
        for (key, value) in [
            ("size", "NaN"),
            ("size", "inf"),
            ("size", "1.5"),
            ("size", "18446744073709551616"),
            ("length", "NaN"),
            ("length", "-1"),
        ] {
            let mut item = json!({"name":"Show EP 5.mp4", "format":"MPEG4", "size":"300000000", "length":"1400"});
            item[key] = json!(value);
            assert!(
                playable_files(&json!({"files":[item]})).is_empty(),
                "{key}={value}"
            );
        }
        for name in [
            "../Show.mp4",
            "/Show.mp4",
            "a/../Show.mp4",
            "a\\Show.mp4",
            "Show\u{0000}.mp4",
        ] {
            assert!(playable_files(&json!({"files":[{"name":name,"format":"MPEG4","size":"300000000","length":"1400"}]})).is_empty(), "{name}");
        }
        let mut invalid = file("Show EP 5.mp4");
        invalid.seconds = Some(f64::INFINITY);
        assert!(!is_episode_length(&invalid));
    }

    async fn fixture(title: &str, files: Value) -> (String, tokio::task::JoinHandle<()>) {
        use axum::{Json, Router, routing::get};
        let search = json!({"response":{"docs":[{"identifier":"same-item","title":title}]}});
        let metadata = json!({"files":files});
        let app = Router::new()
            .route(
                "/advancedsearch.php",
                get(move || {
                    let value = search.clone();
                    async move { Json(value) }
                }),
            )
            .route(
                "/download/{id}/{name}",
                get(|| async {
                    include_bytes!("../../tests/fixtures/archive-video.mp4").as_slice()
                }),
            )
            .route(
                "/metadata/{id}",
                get(move || {
                    let value = metadata.clone();
                    async move { Json(value) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (base, task)
    }

    #[tokio::test]
    async fn find_never_promotes_only_clip_and_scopes_cache_to_endpoint() {
        let titles = vec!["Archive Regression Sole Upload".into()];
        let title = "Archive Regression Sole Upload EP 5";
        let (base, task) = fixture(
            title,
            json!([{"name":"EP 5.mp4","format":"MPEG4","size":"300000000","length":"74"}]),
        )
        .await;
        let result = find(
            &base,
            &titles,
            &[],
            Some(5),
            None,
            Instant::now() + Duration::from_secs(2),
        )
        .await;
        task.abort();
        assert!(result.is_err());
        let (base, task) = fixture(
            title,
            json!([{"name":"EP 5.mp4","format":"MPEG4","size":include_bytes!("../../tests/fixtures/archive-video.mp4").len(),"length":"300"}]),
        )
        .await;
        let result = find(
            &base,
            &titles,
            &[],
            Some(5),
            None,
            Instant::now() + Duration::from_secs(2),
        )
        .await;
        task.abort();
        assert!(result.is_ok());
    }

    #[test]
    fn runtime_and_conflicting_markers_are_enforced() {
        let files = [file("Show S02E05.mp4")];
        assert!(
            pick_file_with_runtime("Show S02E05", &files, Some(5), Some(2), Some(1400.)).is_some()
        );
        assert!(
            pick_file_with_runtime("Show S02E05", &files, Some(5), Some(2), Some(3600.)).is_none()
        );
        assert!(
            pick_file_with_runtime("Show S02E05", &files, Some(5), Some(2), Some(f64::NAN))
                .is_none()
        );
        assert_eq!(
            identifies_episode("Show S02E05E06.mp4", 5, Some(2)),
            Some(false)
        );
        let mut unknown = file("Show S02E05.mp4");
        unknown.seconds = None;
        assert!(pick_file("Show S02E05", &[unknown], Some(5), Some(2)).is_none());
        assert!(matches_title("India", &["India".into()]));
        assert!(matches_title("India S02E05", &["India".into()]));
    }

    #[tokio::test]
    async fn find_uses_catalog_identity_not_query_identity() {
        let (base, task) = fixture(
            "Show commentary EP 5",
            json!([{"name":"EP 5.mp4","format":"MPEG4","size":"300000000","length":"1400"}]),
        )
        .await;
        let result = find(
            &base,
            &["Show".into()],
            &["Show commentary".into()],
            Some(5),
            None,
            Instant::now() + Duration::from_secs(2),
        )
        .await;
        task.abort();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn find_caps_http_bodies_and_shares_one_deadline() {
        use axum::{Router, routing::get};
        let app = Router::new().route(
            "/advancedsearch.php",
            get(|| async { " ".repeat(MAX_RESPONSE_BYTES + 1) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        assert!(
            search_items(&base, "Cap", Duration::from_secs(2))
                .await
                .unwrap_err()
                .to_string()
                .contains("too large")
        );
        task.abort();
        let app = Router::new().route(
            "/advancedsearch.php",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                "{}"
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let started = Instant::now();
        let result = find(
            &base,
            &["Deadline one".into(), "Deadline two".into()],
            &[],
            Some(5),
            None,
            started + Duration::from_millis(60),
        )
        .await;
        task.abort();
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[tokio::test]
    async fn actual_http_media_is_required_before_find_returns() {
        use axum::{Router, routing::get};
        let real = include_bytes!("../../tests/fixtures/archive-video.mp4").as_slice();
        for (body, seconds, accepted) in [
            (real.to_vec(), 300., true),
            (b"<html>not video</html>".to_vec(), 300., false),
            (real[..real.len() / 2].to_vec(), 300., false),
            (
                include_bytes!("../../tests/fixtures/archive-clip.mp4").to_vec(),
                300.,
                false,
            ),
            (real.to_vec(), 1400., false),
        ] {
            let size = body.len();
            let app = Router::new().route(
                "/download/item/video.mp4",
                get(move || {
                    let body = body.clone();
                    async move { body }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            let file = ArchiveFile {
                name: "video.mp4".into(),
                size: size as u64,
                format: "MPEG4".into(),
                seconds: Some(seconds),
            };
            let result = verify_media(
                &base,
                "item",
                &file,
                None,
                Instant::now() + Duration::from_secs(5),
            )
            .await;
            task.abort();
            assert_eq!(
                result.is_ok(),
                accepted,
                "size={size}, seconds={seconds}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn actual_codecs_override_playable_archive_metadata() {
        use axum::{Router, routing::get};

        // Generate real, full-length, tiny media instead of mocking ffprobe JSON.
        // Every candidate is advertised as MPEG4, regardless of its actual encoding.
        for (label, video, pixels, audio, container, accepted) in [
            ("portable", "libx264", "yuv420p", "aac", "mp4", true),
            ("hevc", "libx265", "yuv420p", "aac", "mp4", false),
            ("high10", "libx264", "yuv420p10le", "aac", "mp4", false),
            ("ac3", "libx264", "yuv420p", "ac3", "mp4", false),
            ("matroska", "libx264", "yuv420p", "aac", "matroska", false),
        ] {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).unwrap();
            let path = std::env::temp_dir().join(format!(
                "akidev-archive-{}-{:032x}.media",
                std::process::id(),
                u128::from_le_bytes(random)
            ));
            let mut command = Command::new("ffmpeg");
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-n",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:r=1:d=300",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=32000:cl=mono,atrim=duration=1",
                "-t",
                "300",
                "-c:v",
                video,
                "-pix_fmt",
                pixels,
                "-threads",
                "1",
                "-preset",
                "ultrafast",
                "-c:a",
                audio,
                "-b:a",
                "32k",
            ]);
            if video == "libx265" {
                command.args(["-x265-params", "pools=none:frame-threads=1:log-level=error"]);
            }
            if container == "mp4" {
                command.args(["-movflags", "+faststart"]);
            }
            let output = tokio::time::timeout(
                Duration::from_secs(15),
                command
                    .args(["-f", container])
                    .arg(&path)
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .expect("fixture generation timed out")
            .expect("regression requires installed ffmpeg with libx264 and libx265");
            assert!(
                output.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let body = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            let size = body.len();
            assert!(size <= MEDIA_HEAD_BYTES);
            let app = Router::new().route(
                "/download/item/video.mp4",
                get(move || {
                    let body = body.clone();
                    async move { body }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let result = verify_media(
                &base,
                "item",
                &ArchiveFile {
                    name: "video.mp4".into(),
                    size: size as u64,
                    format: "MPEG4".into(),
                    seconds: Some(300.),
                },
                None,
                Instant::now() + Duration::from_secs(5),
            )
            .await;
            task.abort();
            if accepted {
                assert!(result.is_ok(), "{label}: {result:?}");
            } else {
                // Prove rejection comes from compatibility, not corruption, a
                // short duration, missing decoded frames, or ffprobe failures.
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "media is not portable direct playback",
                    "{label}"
                );
            }
        }
    }

    #[tokio::test]
    async fn media_redirects_ranges_body_caps_and_deadlines_fail_closed() {
        use axum::{Router, response::Redirect, routing::get};
        let app = Router::new()
            .route(
                "/download/item/redirect.mp4",
                get(|| async { Redirect::temporary("http://127.0.0.1:1/private") }),
            )
            .route(
                "/download/item/large.mp4",
                get(|| async { vec![0u8; MEDIA_HEAD_BYTES + 1] }),
            )
            .route(
                "/download/item/range.mp4",
                get(|| async {
                    (
                        reqwest::StatusCode::PARTIAL_CONTENT,
                        [("content-range", "bytes 1-20/300000000")],
                        "not a head",
                    )
                }),
            )
            .route(
                "/download/item/slow.mp4",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    "slow"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        for name in ["redirect.mp4", "large.mp4", "range.mp4", "slow.mp4"] {
            let file = ArchiveFile {
                name: name.into(),
                size: 100,
                format: "MPEG4".into(),
                seconds: Some(300.),
            };
            let started = Instant::now();
            assert!(
                verify_media(
                    &base,
                    "item",
                    &file,
                    None,
                    started + Duration::from_millis(100)
                )
                .await
                .is_err()
            );
            assert!(started.elapsed() < Duration::from_millis(500));
        }
        task.abort();
    }

    #[test]
    fn film_year_rejects_wrong_remakes_but_allows_exact_missing_year() {
        let titles = vec!["Dune".into()];
        for (item, name, accepted) in [
            ("Dune (1984)", "Dune 1984.mp4", false),
            ("Dune (2021)", "Dune 1984.mp4", false),
            ("Dune", "1984.mp4", false),
            ("Dune (2021)", "Dune 2021.mp4", true),
            ("Dune", "Dune.mp4", true),
            ("Dune 1080p", "Dune.ia.mp4", true),
            ("Dune returns", "Dune.mp4", false),
        ] {
            assert_eq!(
                pick_file_with_identity(item, &titles, &[file(name)], None, None, None, Some(2021))
                    .is_some(),
                accepted,
                "{item}: {name}"
            );
        }
    }

    #[test]
    fn year_is_parsed_after_longest_numeric_catalogue_alias() {
        let titles = vec!["Blade Runner".into(), "Blade Runner 2049".into()];
        assert!(matches_identity(
            "Blade Runner 2049 (2017)",
            &titles,
            Some(2017)
        ));
        assert!(matches_identity("Blade Runner 2049", &titles, Some(2017)));
        assert!(!matches_identity(
            "Blade Runner 2049 (1982)",
            &titles,
            Some(2017)
        ));
        assert!(
            pick_file_with_identity(
                "Blade Runner 2049 (2017)",
                &titles,
                &[file("Blade.Runner.2049.2017.1080p.mp4")],
                None,
                None,
                None,
                Some(2017)
            )
            .is_some()
        );
        assert!(matches_identity(
            "2049 (2017)",
            &["2049".into()],
            Some(2017)
        ));
        assert!(year_agrees("ep2026", Some(2017)));
    }

    #[tokio::test]
    async fn find_with_identity_rejects_explicit_wrong_remake() {
        for (title, name) in [
            ("Dune 1984", "Dune 1984.mp4"),
            ("Dune 2021", "Dune 1984.mp4"),
        ] {
            let (base, task) = fixture(title, json!([{
                "name":name,"format":"MPEG4",
                "size":include_bytes!("../../tests/fixtures/archive-video.mp4").len(),"length":"300"
            }])).await;
            let result = find_with_identity(
                &base,
                &["Dune".into()],
                &[],
                None,
                None,
                Some(300.),
                Some(2021),
                Instant::now() + Duration::from_secs(2),
            )
            .await;
            task.abort();
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn exact_document_after_six_irrelevant_results_is_probed() {
        use axum::{Json, Router, routing::get};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut docs: Vec<Value> = (0..17)
            .map(|n| {
                json!({
                    "identifier":format!("irrelevant-{n}"),
                    "title": if n % 2 == 0 { "Other Show EP 5" } else { "Show EP 6" }
                })
            })
            .collect();
        docs.push(json!({"identifier":"exact", "title":"Show EP 5"}));
        let search = json!({"response":{"docs":docs}});
        let probes = Arc::new(AtomicUsize::new(0));
        let count = probes.clone();
        let app = Router::new()
            .route("/advancedsearch.php", get(move || { let value = search.clone(); async move { Json(value) } }))
            .route("/metadata/{id}", get(move || {
                count.fetch_add(1, Ordering::SeqCst);
                async { Json(json!({"files":[{"name":"EP 5.mp4","format":"MPEG4",
                    "size":include_bytes!("../../tests/fixtures/archive-video.mp4").len(),"length":"300"}]})) }
            }))
            .route("/download/{id}/{name}", get(|| async {
                include_bytes!("../../tests/fixtures/archive-video.mp4").as_slice()
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = find(
            &base,
            &["Show".into()],
            &[],
            Some(5),
            None,
            Instant::now() + Duration::from_secs(3),
        )
        .await;
        task.abort();
        assert!(result.is_ok());
        assert_eq!(probes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn repeated_identifiers_across_queries_consume_one_probe() {
        use axum::{Json, Router, routing::get};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let probes = Arc::new(AtomicUsize::new(0));
        let count = probes.clone();
        let app = Router::new()
            .route(
                "/advancedsearch.php",
                get(|| async {
                    Json(json!({"response":{"docs":[
                        {"identifier":"duplicate","title":"Show EP 5"},
                        {"identifier":"duplicate","title":"Show EP 5"}
                    ]}}))
                }),
            )
            .route(
                "/metadata/{id}",
                get(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    async { Json(json!({"files":[]})) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = find(
            &base,
            &["Show".into()],
            &["Show EP 5".into()],
            Some(5),
            None,
            Instant::now() + Duration::from_secs(2),
        )
        .await;
        task.abort();
        assert!(result.is_err());
        assert_eq!(probes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_identifier_is_a_plain_token() {
        assert!(identifier_ok("igl-s2_06.v1"));
        assert!(!identifier_ok("../etc"));
        assert!(!identifier_ok("a b"));
        assert!(!identifier_ok(""));
        assert!(!identifier_ok("."));
        assert!(!identifier_ok(".."));
    }
}
