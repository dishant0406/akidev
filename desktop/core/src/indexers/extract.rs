//! Reading releases out of a response, as an indexer definition describes it.

use crate::calendar::now_ms;
use crate::definition::{Format, IndexerDef};
use crate::expression::{Env, Row, bytes, evaluate, matches, pattern, select};
use crate::magnet::{info_hash_from_magnet, to_hex_info_hash};
use crate::model::{Kind, Release};
use anyhow::bail;
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

/// Every release in a response body.
///
/// A row that lacks a title or a readable hash is skipped, because the rest of the pipeline
/// cannot use it. `term` is the title that was searched, for `must_contain_term`.
pub fn releases(
    def: &IndexerDef,
    body: &str,
    kind: Kind,
    term: &str,
    trackers: &[String],
) -> anyhow::Result<Vec<Release>> {
    let response = &def.response;
    let document = match response.format {
        Format::Json => {
            let document: Value =
                serde_json::from_str(body).map_err(|_| anyhow::anyhow!("not a JSON response"))?;
            if !(document.is_object() || document.is_array()) {
                bail!("not a JSON response");
            }
            validate_json(def, &document, kind)?;
            document
        }
        // A blocked or replaced host answers 200 with an HTML page. A real feed is RSS, so
        // that is what is checked: otherwise a block page would read as "no results".
        Format::Rss => {
            if !pattern(r"<rss[\s>]")?.is_match(body) {
                bail!("not an RSS feed");
            }
            Value::Array(rss_items(body))
        }
    };

    let wanted = normalize(term);
    let env = Env {
        now_ms: now_ms(),
        active: &[],
    };
    let mut found: Vec<Release> = Vec::new();
    for outer in select(&document, &response.rows) {
        let rows = match &response.children {
            Some(path) => select(outer, path)
                .into_iter()
                .map(|value| Row {
                    value,
                    parent: Some(outer),
                    kind,
                })
                .collect(),
            None => vec![Row {
                value: outer,
                parent: None,
                kind,
            }],
        };
        for row in rows {
            if response
                .drop
                .iter()
                .any(|condition| matches(condition, row, &env))
            {
                continue;
            }
            let Some(release) = release(def, row, trackers) else {
                continue;
            };
            if response.must_contain_term
                && (wanted.is_empty() || !normalize(&release.title).contains(&wanted))
            {
                continue;
            }
            add(
                &mut found,
                release,
                response.dedupe.as_ref().map(|d| d.avoid.as_str()),
            );
        }
    }
    Ok(found)
}

/// Validate the envelope before permissive extraction can cache failures as empty answers.
fn validate_json(def: &IndexerDef, document: &Value, kind: Kind) -> anyhow::Result<()> {
    if let Some(object) = document.as_object() {
        for key in ["error", "errors"] {
            if let Some(value) = object.get(key)
                && !value.is_null()
                && value != &Value::Bool(false)
                && value != ""
                && value != &serde_json::json!([])
                && value != &serde_json::json!({})
            {
                bail!("provider error response");
            }
        }
        if object.get("success") == Some(&Value::Bool(false)) {
            bail!("provider error response");
        }
        if let Some(status) = object.get("status") {
            let valid = match status {
                Value::String(status) => {
                    matches!(status.to_ascii_lowercase().as_str(), "ok" | "success")
                }
                Value::Number(status) => status
                    .as_u64()
                    .is_some_and(|status| (200..300).contains(&status)),
                _ => false,
            };
            if !valid {
                bail!("provider error response");
            }
        }
    }
    if is_empty_answer(def, document, kind) {
        return Ok(());
    }
    let rows = collection(document, &def.response.rows)?;
    if let Some(children) = &def.response.children {
        for row in rows {
            // Incomplete outer rows are skipped, just like rows without a hash/title.
            // A present child collection must still have the expected shape.
            if row.get(children).is_some() {
                collection(row, children)?;
            }
        }
    }
    Ok(())
}

/// Whether the definition says this body is a successful answer with no rows.
fn is_empty_answer(def: &IndexerDef, document: &Value, kind: Kind) -> bool {
    let conditions = &def.response.empty_when;
    let env = Env {
        now_ms: now_ms(),
        active: &[],
    };
    let body = Row {
        value: document,
        parent: None,
        kind,
    };
    !conditions.is_empty()
        && conditions
            .iter()
            .all(|condition| matches(condition, body, &env))
}

fn collection<'a>(document: &'a Value, path: &str) -> anyhow::Result<Vec<&'a Value>> {
    let mut nodes = vec![document];
    if !path.is_empty() {
        for key in path.split('.') {
            let mut next = Vec::new();
            for node in nodes {
                match (key, node) {
                    ("*", Value::Object(object)) => next.extend(object.values()),
                    ("*", Value::Array(array)) => next.extend(array),
                    (key, Value::Object(object)) => next.push(
                        object
                            .get(key)
                            .ok_or_else(|| anyhow::anyhow!("missing response collection"))?,
                    ),
                    _ => bail!("invalid response collection"),
                }
            }
            nodes = next;
        }
    }
    let mut rows = Vec::new();
    for node in nodes {
        match node {
            Value::Array(array) => rows.extend(array),
            Value::Object(_) if path.split('.').any(|key| key == "*") => rows.push(node),
            _ => bail!("invalid response collection"),
        }
    }
    if rows.iter().any(|row| !row.is_object()) {
        bail!("invalid response row");
    }
    Ok(rows)
}

fn release(def: &IndexerDef, row: Row, trackers: &[String]) -> Option<Release> {
    let fields = &def.response.fields;
    let title = evaluate(&fields.title, row)?;
    if title.trim().is_empty() || title.len() > 4096 || title.chars().any(char::is_control) {
        return None;
    }
    let info_hash = to_hex_info_hash(&evaluate(&fields.hash, row)?)?;
    let magnet = fields
        .magnet
        .as_ref()
        .and_then(|expression| evaluate(expression, row))
        // Only a magnet link for this very hash is used: an indexer's own text is not
        // handed to the torrent engine as an address.
        .filter(|magnet| {
            magnet.starts_with("magnet:?")
                && info_hash_from_magnet(magnet).as_deref() == Some(info_hash.as_str())
        })
        .unwrap_or_default();
    let magnet = super::safe_magnet(&info_hash, &title, &[&magnet], trackers);
    let size_bytes = fields
        .size
        .as_ref()
        .and_then(|expression| evaluate(expression, row))
        .and_then(|text| bytes(&text))
        // A size of 0 means the indexer does not know it.
        .filter(|size| *size > 0);
    let seeders = fields
        .seeders
        .as_ref()
        .and_then(|expression| evaluate(expression, row))
        .and_then(|text| text.trim().parse::<u32>().ok());
    Some(Release {
        title,
        info_hash,
        file_idx: match fields
            .file_idx
            .as_ref()
            .and_then(|expression| evaluate(expression, row))
        {
            // A present but malformed hint is not an unscoped row: playing a different file
            // would violate the addon's explicit selection.
            Some(text) => Some(text.parse::<usize>().ok()?),
            None => None,
        },
        magnet,
        size_bytes,
        seeders,
        source: def.id.clone(),
        sources: vec![def.id.clone()],
    })
}

/// Adds a release, keeping one per info hash when `avoid` is set: the first whose title does
/// not match it, in the place of the first one seen.
fn add(found: &mut Vec<Release>, release: Release, avoid: Option<&str>) {
    let Some(avoid) = avoid.and_then(|text| pattern(text).ok()) else {
        found.push(release);
        return;
    };
    match found.iter().position(|known| {
        known.info_hash == release.info_hash && known.file_idx == release.file_idx
    }) {
        None => found.push(release),
        Some(at) => {
            let prefer = avoid.is_match(&found[at].title) && !avoid.is_match(&release.title);
            super::combine(&mut found[at], &release);
            if prefer {
                found[at].title = release.title;
            }
        }
    }
}

/// Lowercase, with every character that is not a letter, digit or space turned into a space
/// and runs of spaces collapsed.
fn normalize(text: &str) -> String {
    static NOISE: OnceLock<Regex> = OnceLock::new();
    let noise = NOISE.get_or_init(|| Regex::new(r"[^\p{L}\p{N}\s]").unwrap());
    let lowered = text.to_lowercase();
    noise
        .replace_all(&lowered, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The items of an RSS feed, each as an object of what its tags carry: a tag's text under the
/// tag's name, and a tag's attributes under `tag@attribute`. A Torznab attribute, which is
/// `<torznab:attr name="seeders" value="12"/>`, is `torznab:attr@seeders`. The first of a repeated
/// tag wins.
fn rss_items(xml: &str) -> Vec<Value> {
    xml.split("<item>")
        .skip(1)
        .map(|item| {
            let item = item.split("</item>").next().unwrap_or(item);
            let mut tags = Map::new();
            let mut rest = item;
            while let Some(open) = rest.find('<') {
                rest = &rest[open + 1..];
                let Some(end) = rest.find('>') else { break };
                let head = &rest[..end];
                let name = head.split_whitespace().next().unwrap_or("");
                if name.is_empty() || name.starts_with(['/', '!', '?']) {
                    continue;
                }
                for (key, value) in attributes(name, &head[name.len()..]) {
                    tags.entry(key).or_insert(Value::String(value));
                }
                if head.ends_with('/') {
                    continue;
                }
                let body = &rest[end + 1..];
                let Some(close) = body.find(&format!("</{name}>")) else {
                    continue;
                };
                tags.entry(name.to_string())
                    .or_insert_with(|| Value::String(unwrap_text(&body[..close])));
                rest = &body[close..];
            }
            Value::Object(tags)
        })
        .collect()
}

/// The attributes of a tag as `(key, value)`: `tag@attribute`, or for an attribute-style tag
/// (`name="x" value="y"`) `tag@x`.
fn attributes(tag: &str, text: &str) -> Vec<(String, String)> {
    static ATTRIBUTE: OnceLock<Regex> = OnceLock::new();
    let attribute =
        ATTRIBUTE.get_or_init(|| Regex::new(r#"([A-Za-z0-9_:.-]+)\s*=\s*"([^"]*)""#).unwrap());
    let pairs: Vec<(&str, String)> = attribute
        .captures_iter(text)
        .map(|found| (found.get(1).unwrap().as_str(), unwrap_text(&found[2])))
        .collect();
    let named = pairs.iter().find(|(key, _)| *key == "name");
    let valued = pairs.iter().find(|(key, _)| *key == "value");
    match (named, valued) {
        (Some((_, name)), Some((_, value))) => vec![(format!("{tag}@{name}"), value.clone())],
        _ => pairs
            .into_iter()
            .map(|(key, value)| (format!("{tag}@{key}"), value))
            .collect(),
    }
}

/// The text of a tag, with CDATA unwrapped and the common entities decoded.
fn unwrap_text(raw: &str) -> String {
    raw.replace("<![CDATA[", "")
        .replace("]]>", "")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources;
    use serde_json::json;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn aniscraper_reads_the_filename_size_and_seeders_of_a_real_answer() {
        // Saved from a real answer:
        // https://c5541ffce7d3-aniscraper.baby-beamup.club/stream/series/kitsu:48105:1:1.json
        let body = json!({"streams": [
            {"name": "[p2p] Aniscraper 1080P ",
             "description": "🎬 [SubsPlease] Sousou no Frieren - 01 (1080p) [F02B9CEE].mkv\n📜 Season 1  Episode 1\n📏 1.5 GiB | 🌱 116\n⚙️ Nyaa",
             "infoHash": "374ff33f0fae5206c693c7c7e0bd7acc88ef6ad0",
             "fileIdx": 0,
             "sources": ["dht:374ff33f0fae5206c693c7c7e0bd7acc88ef6ad0"],
             "_quality": "1080P",
             "behaviorHints": {"bingeGroup": "nyaa-p2p-1080P-subsplease",
                               "videoSize": 1610612736,
                               "filename": "[SubsPlease] Sousou no Frieren - 01 (1080p) [F02B9CEE].mkv"}},
            {"name": "[p2p] Aniscraper 1080P HEVC",
             "description": "🎬 [Arg0] Frieren: Beyond Journey's End (2023) - S01E01 v3 (WEB 1080p HEVC x265 E-AC-3) [Dual-Audio] | Sousou no Frieren | Frieren at the Funeral.mkv\n📜 Season 1  Episode 1\n📏 3.3 GiB | 🌱 99\n⚙️ Nyaa",
             "infoHash": "7796597611c96c5d0d69b6546eb0480b40ffd54e",
             "fileIdx": 0,
             "sources": ["dht:7796597611c96c5d0d69b6546eb0480b40ffd54e"],
             "_quality": "1080P",
             "behaviorHints": {"bingeGroup": "nyaa-p2p-1080P-arg0",
                               "videoSize": 3543348019i64,
                               "filename": "[Arg0] Frieren: Beyond Journey's End (2023) - S01E01 v3 (WEB 1080p HEVC x265 E-AC-3) [Dual-Audio] | Sousou no Frieren | Frieren at the Funeral.mkv"}}
        ]});
        let found = run("aniscraper", &body, "x");
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].title,
            "[SubsPlease] Sousou no Frieren - 01 (1080p) [F02B9CEE].mkv"
        );
        assert_eq!(
            found[0].info_hash,
            "374ff33f0fae5206c693c7c7e0bd7acc88ef6ad0"
        );
        assert_eq!(found[0].size_bytes, Some(1610612736));
        assert_eq!(found[0].seeders, Some(116));
        assert_eq!(found[0].file_idx, Some(0));
        assert_eq!(found[1].seeders, Some(99));
        assert_eq!(found[1].size_bytes, Some(3543348019));
    }

    #[test]
    fn final_review_torrentio_extract_rank_select() {
        use crate::releases::{VideoFile, pick_video_file_for_release, rank_releases};

        let body = json!({"streams": [
            {"infoHash": A, "title": "India's Got Latent\nIGL_EP_01_ft_RaftaarMusic.mp4\n👤 47 💾 473.25 MB ⚙️ ThePirateBay"},
            {"infoHash": B, "title": "Show\nfallback.mkv\n👤 1 💾 1 GB ⚙️ X", "behaviorHints": {"filename": "S01E01.mkv"}}
        ]});
        // Use the actual extractor and its production magnet/merge helpers.
        let found = run("torrentio", &body, "x");
        assert_eq!(found.len(), 2);
        for (i, title, filename, hash) in [
            (0, "India's Got Latent", "IGL_EP_01_ft_RaftaarMusic.mp4", A),
            (1, "Show", "S01E01.mkv", B),
        ] {
            let ranked = rank_releases(&found, 1, title, &[], Some(1));
            assert_eq!(ranked.len(), 1, "{}", found[i].title);
            assert_eq!(ranked[0].info_hash, hash);
            let files = [VideoFile {
                index: 0,
                name: filename.to_string(),
                length: 900_000_000,
            }];
            assert_eq!(
                pick_video_file_for_release(
                    &files,
                    Some(1),
                    0,
                    Some(1),
                    &ranked[0].title,
                    title,
                    &[],
                    None
                ),
                Some(&files[0]),
                "{filename}"
            );
        }
    }

    #[test]
    fn torrentio_file_hints_survive_dedupe_and_metadata_identity_gates() {
        use crate::releases::{VideoFile, pick_selected_video_file_for_release, rank_releases};

        let body = json!({"streams": [
            {
                "infoHash": A,
                "fileIdx": 0,
                "title": "Show.S01.1080p.WEB-DL.x264\nShow.S01E01.1080p.WEB-DL.x264.mkv\n👤 47 💾 900 MB"
            },
            {
                "infoHash": A,
                "fileIdx": 1,
                "title": "Show.S01.720p.WEB-DL.x264",
                "behaviorHints": {"filename": "Show.S01E01.720p.WEB-DL.x264.mkv"}
            },
            {
                "infoHash": B,
                "fileIdx": 2,
                "description": "Other.Show.S01E02.1080p.WEB-DL.x264\n👤 100"
            }
        ]});
        let found = run("torrentio", &body, "x");
        assert_eq!(
            found.len(),
            3,
            "the same hash's file rows must stay distinct"
        );
        assert_eq!(found[0].info_hash, found[1].info_hash);
        assert_eq!(found[0].file_idx, Some(0));
        assert_eq!(found[1].file_idx, Some(1));
        assert_eq!(
            found[0].title,
            "Show.S01.1080p.WEB-DL.x264 Show.S01E01.1080p.WEB-DL.x264.mkv"
        );
        assert_eq!(
            found[1].title,
            "Show.S01.720p.WEB-DL.x264 Show.S01E01.720p.WEB-DL.x264.mkv"
        );
        assert_ne!(found[0].selection_id(), found[1].selection_id());

        let ranked = rank_releases(&found, 1, "Show", &[], Some(1));
        assert_eq!(ranked.len(), 2);
        let files = [
            VideoFile {
                index: 0,
                name: "Show/S01/Show.S01E01.1080p.WEB-DL.x264.mkv".into(),
                length: 900_000_000,
            },
            VideoFile {
                index: 1,
                name: "Show\\S01\\Show.S01E01.720p.WEB-DL.x264.mkv".into(),
                length: 700_000_000,
            },
            VideoFile {
                index: 2,
                name: "Other.Show/S01/Other.Show.S01E02.1080p.WEB-DL.x264.mkv".into(),
                length: 900_000_000,
            },
            VideoFile {
                index: 3,
                name: "Show/S02/Show.S02E01.1080p.WEB-DL.x264.mkv".into(),
                length: 900_000_000,
            },
        ];
        for release in &ranked {
            let picked = pick_selected_video_file_for_release(
                &files,
                Some(1),
                Some(1),
                &release.title,
                "Show",
                &[],
                None,
                release.file_idx,
            )
            .unwrap();
            assert_eq!(picked.index, release.file_idx.unwrap());
            assert!(picked.name.contains("Show.S01E01"));
        }
        assert!(
            pick_selected_video_file_for_release(
                &files,
                Some(1),
                Some(1),
                "Show.S01.1080p.WEB-DL.x264",
                "Show",
                &[],
                None,
                Some(2),
            )
            .is_none(),
            "a hinted index cannot override file title/episode identity"
        );
        assert!(
            pick_selected_video_file_for_release(
                &files,
                Some(1),
                Some(1),
                "Show.S01.1080p.WEB-DL.x264",
                "Show",
                &[],
                None,
                Some(3),
            )
            .is_none(),
            "a hinted index cannot override the season"
        );
        let conflicts = json!({"streams": [
            {
                "infoHash": A,
                "fileIdx": 1,
                "title": "Other.Show.S01E01.1080p.WEB-DL.x264"
            },
            {
                "infoHash": A,
                "fileIdx": 1,
                "fileIdx": 2,
                "title": "Show.S02E01.1080p.WEB-DL.x264"
            }
        ]});
        assert!(
            rank_releases(&run("torrentio", &conflicts, "x"), 1, "Show", &[], Some(1)).is_empty()
        );
        assert_eq!(run("torrentio", &conflicts, "x").len(), 2);
        let bounds = json!({"streams": [
            {"infoHash": A, "title": "Show.S01E01"},
            {"infoHash": B, "fileIdx": "184467440737095516160", "title": "Show.S01E01"},
            {"infoHash": B, "fileIdx": "not-a-number", "title": "Show.S01E01"},
            {"infoHash": B, "fileIdx": -1, "title": "Show.S01E01"},
            {"infoHash": B, "fileIdx": 1.5, "title": "Show.S01E01"},
            {"infoHash": A, "fileIdx": 0, "title": "Show.S01E01"}
        ]});
        let found = run("torrentio", &bounds, "x");
        assert_eq!(
            found.len(),
            2,
            "a present but unreadable index rejects the row instead of unselecting its file"
        );
        assert!(found.iter().any(|row| row.file_idx.is_none()));
        assert!(found.iter().any(|row| row.file_idx == Some(0)));
    }

    #[test]
    fn json_envelopes_require_the_defined_collections() {
        let sources = sources::current();
        let extract = |id: &str, body: Value| {
            releases(
                sources.indexer(id).unwrap(),
                &body.to_string(),
                Kind::Movie,
                "x",
                &[],
            )
        };
        for id in [
            "knaben",
            "torrentio",
            "yts",
            "apibay",
            "subsplease",
            "eztv",
            "zilean",
        ] {
            assert!(
                extract(id, json!({"error": "rate limited"})).is_err(),
                "{id}"
            );
        }
        for body in [
            json!({}),
            json!({"hits": null}),
            json!({"hits": {}}),
            json!({"hits": "blocked"}),
            json!({"hits": [null]}),
            json!({"hits": [], "status": "error"}),
            json!({"hits": [], "success": false}),
        ] {
            assert!(extract("knaben", body).is_err());
        }
        assert!(extract("knaben", json!({"hits": []})).unwrap().is_empty());
        assert!(extract("apibay", json!([])).unwrap().is_empty());
        assert!(extract("subsplease", json!({})).unwrap().is_empty());
        assert!(
            extract(
                "apibay",
                json!([{"name": "No results returned", "info_hash": "0".repeat(40)}])
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            extract("yts", json!({"status":"ok", "data":{"movie_count":0}}))
                .unwrap()
                .is_empty()
        );
        assert!(extract("yts", json!({"status":"error", "data":{"movie_count":0}})).is_err());
        assert!(
            extract(
                "yts",
                json!({"status":"ok", "data":{"movies":[{"title":"x", "torrents":null}]}})
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_seeders_are_not_reported_as_zero() {
        let found = run(
            "apibay",
            &json!([
                { "name": "missing", "info_hash": A },
                { "name": "zero", "info_hash": B, "seeders": "0" }
            ]),
            "x",
        );
        assert_eq!(
            serde_json::to_value(&found[0]).unwrap()["seeders"],
            Value::Null
        );
        assert_eq!(
            serde_json::to_value(&found[1]).unwrap()["seeders"],
            json!(0)
        );
    }

    fn trackers() -> Vec<String> {
        vec!["udp://t.example:1".into()]
    }

    /// What the embedded definition of `id` reads out of `body`.
    fn run(id: &str, body: &Value, term: &str) -> Vec<Release> {
        run_text(id, &body.to_string(), term)
    }

    fn run_text(id: &str, body: &str, term: &str) -> Vec<Release> {
        let sources = sources::current();
        releases(
            sources.indexer(id).unwrap(),
            body,
            Kind::Series,
            term,
            &trackers(),
        )
        .unwrap()
    }

    #[test]
    fn malformed_and_out_of_bounds_counts_remain_unknown() {
        for value in [
            json!(null),
            json!("-1"),
            json!("1.5"),
            json!("NaN"),
            json!("4294967296"),
            json!("unknown"),
        ] {
            let found = run(
                "apibay",
                &json!([{ "name": "Show", "info_hash": A, "seeders": value }]),
                "x",
            );
            assert_eq!(found[0].seeders, None);
        }
        let found = run(
            "apibay",
            &json!([{ "name": "Show", "info_hash": A, "seeders": "4294967295" }]),
            "x",
        );
        assert_eq!(found[0].seeders, Some(u32::MAX));
    }

    #[test]
    fn subsplease_reads_each_magnet_of_every_episode() {
        let magnet = format!(
            "magnet:?xt=urn:btih:{A}&dn=%5BSubsPlease%5D%20Bocchi%20the%20Rock%21%20-%2012%20%281080p%29.mkv&xl=1448606898"
        );
        let body = json!({
            "Bocchi the Rock! - 12": { "episode": "12", "downloads": [
                { "res": "1080", "magnet": magnet },
                { "res": "720", "magnet": "" },
                { "res": "480" }
            ] },
            "Empty": { "episode": "1" }
        });
        let found = run("subsplease", &body, "Bocchi the Rock!");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].title,
            "[SubsPlease] Bocchi the Rock! - 12 (1080p).mkv"
        );
        assert_eq!(found[0].info_hash, A);
        assert_eq!(found[0].size_bytes, Some(1_448_606_898));
        // The feed reports no seeder counts, and the magnet is the one it sent.
        assert_eq!(found[0].seeders, None);
        assert!(
            found[0]
                .magnet
                .starts_with(&format!("magnet:?xt=urn:btih:{A}&dn="))
        );
        assert_eq!(found[0].source, "subsplease");
        // A magnet that is not one for the release's own hash is not trusted.
        let hostile = json!({ "X": { "downloads": [ { "magnet": format!("http://127.0.0.1/x?xt=urn:btih:{A}&dn=a") } ] } });
        let rebuilt = run("subsplease", &hostile, "x");
        assert_eq!(rebuilt.len(), 1);
        assert!(
            rebuilt[0]
                .magnet
                .starts_with(&format!("magnet:?xt=urn:btih:{A}&dn="))
        );
        // An empty result is an empty list, not an error.
        assert!(run("subsplease", &json!([]), "x").is_empty());
    }

    #[test]
    fn nyaa_reads_an_rss_feed_and_builds_the_magnet() {
        let feed = format!(
            "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Nyaa - Home</title>\
             <item><title><![CDATA[[Group] Show &amp; Co - 01 (1080p)]]></title>\
             <link>https://nyaa.si/download/1.torrent</link><guid isPermaLink=\"true\">https://nyaa.si/view/1</guid>\
             <nyaa:seeders>12</nyaa:seeders><nyaa:infoHash>{A}</nyaa:infoHash><nyaa:size>1.4 GiB</nyaa:size></item>\
             <item><title>No hash</title></item></channel></rss>"
        );
        let found = run_text("nyaa", &feed, "Show");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "[Group] Show & Co - 01 (1080p)");
        assert_eq!(found[0].seeders, Some(12));
        assert_eq!(
            found[0].size_bytes,
            Some((1.4 * (1u64 << 30) as f64).round() as u64)
        );
        assert!(
            found[0]
                .magnet
                .starts_with(&format!("magnet:?xt=urn:btih:{A}&dn="))
        );
        assert!(found[0].magnet.contains("tr=udp%3A%2F%2Ft.example%3A1"));
    }

    #[test]
    fn tokyotosho_reads_the_hash_from_the_magnet_in_its_link() {
        // The feed names the hash in base32 and sends the magnet as the item's link.
        let feed = "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Tokyo Toshokan</title>\
             <item><category>Batch</category>\
             <title>[SubsPlease] Show (01-12) (1080p) [Batch]</title>\
             <link><![CDATA[magnet:?xt=urn:btih:QCQTJORS4NU3BKJHPAYLRRPR2CJ3GRR4&tr=udp://tracker.opentrackr.org:1337/announce]]></link>\
             </item><item><title>No link</title></item></channel></rss>";
        let found = run_text("tokyotosho", feed, "Show");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "[SubsPlease] Show (01-12) (1080p) [Batch]");
        assert_eq!(found[0].source, "tokyotosho");
        assert_eq!(found[0].info_hash.len(), 40);
        assert!(found[0].magnet.starts_with("magnet:?xt=urn:btih:"));
    }

    #[test]
    fn a_page_that_is_not_the_expected_format_is_an_error() {
        let sources = sources::current();
        let html = "<html><body>Access denied</body></html>";
        for id in ["nyaa", "knaben", "subsplease"] {
            let error =
                releases(sources.indexer(id).unwrap(), html, Kind::Series, "x", &[]).unwrap_err();
            assert!(error.to_string().starts_with("not a"), "{id}: {error}");
        }
    }

    #[test]
    fn knaben_drops_adult_rows_whichever_category_lists_it() {
        let body = json!({ "hits": [
            { "title": "Show S01E01", "hash": A, "bytes": 1000, "liveSeeders": 5, "categoryId": [2000000, 2001000] },
            { "title": "Bad", "hash": B, "bytes": 1, "liveSeeders": 9, "categoryId": [3000000, 5001000] },
            { "title": "Bad too", "hash": B, "categoryId": "5000000" },
            { "title": "No hash", "categoryId": [2000000] },
            { "title": "Null seeders", "hash": "cccccccccccccccccccccccccccccccccccccccc", "liveSeeders": null }
        ] });
        let found = run("knaben", &body, "Show");
        let titles: Vec<&str> = found.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Show S01E01", "Null seeders"]);
        assert_eq!(found[0].seeders, Some(5));
        assert_eq!(found[0].size_bytes, Some(1000));
        assert_eq!(found[1].seeders, None);
        assert_eq!(found[1].size_bytes, None);
    }

    #[test]
    fn zilean_reads_hashes_and_drops_adult_rows() {
        let body = json!([
            { "raw_title": "Show S01 1080p", "info_hash": A, "size": "31518434075", "adult": false },
            { "raw_title": "Bad", "info_hash": B, "size": "1", "adult": true },
            { "raw_title": "No hash", "adult": false }
        ]);
        let found = run("zilean", &body, "Show");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Show S01 1080p");
        assert_eq!(found[0].size_bytes, Some(31_518_434_075));
        // The list holds no seeder counts.
        assert_eq!(found[0].seeders, None);
        let anime = run("zilean-anime", &body, "Show");
        assert_eq!(anime.len(), 1);
        assert!(run("zilean-anime", &body, "Other").is_empty());
    }

    #[test]
    fn nekobt_reads_text_seeders_and_skips_hidden_rows() {
        let body = json!({ "data": { "results": [
            { "title": "[Group] Frieren - 01", "infohash": A, "seeders": "24", "filesize": "6107202578", "hidden": false },
            { "title": "[Group] Frieren - 02", "infohash": B, "seeders": "1", "filesize": "1", "hidden": true }
        ] } });
        let found = run("nekobt", &body, "Frieren");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, Some(24));
        assert_eq!(found[0].size_bytes, Some(6_107_202_578));
    }

    #[test]
    fn bangumi_moe_reads_sizes_written_as_text() {
        let body = json!({ "torrents": [
            { "title": "[7ACG] Sousou no Frieren S01", "infoHash": A, "size": "52.02 GB", "seeders": 3 },
            { "title": "Something else", "infoHash": B, "size": "1 GB", "seeders": 9 }
        ] });
        let found = run("bangumi-moe", &body, "Frieren");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, Some(3));
        assert!(found[0].size_bytes.unwrap() > 50_000_000_000);
    }

    #[test]
    fn torrentsdb_reads_a_stremio_stream() {
        let body = json!({ "streams": [{
            "name": "TorrentsDB\n2160p",
            "title": "Breaking Bad S01E05 Gray Matter 2160p\n📅 S01E05 👤 12 💾 5.66 GB ⚙️ limetorrent",
            "infoHash": A,
            "behaviorHints": { "filename": "Breaking Bad S01E05 Gray Matter 2160p.mkv" }
        }] });
        let found = run("torrentsdb", &body, "");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, Some(12));
        assert!(found[0].title.contains("Gray Matter"));
        assert!(found[0].size_bytes.is_some());
    }

    #[test]
    fn prowlarr_reads_what_the_servers_fallback_search_answers() {
        let body = json!({ "results": [
            { "title": "Show S01E01 1080p", "hash": A, "size": 1000, "seeders": 7, "source": "limetorrents" },
            { "title": "Something else", "hash": B, "size": null, "seeders": 0, "source": "dmhy" }
        ] });
        let found = run("prowlarr", &body, "Show");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, Some(7));
        assert_eq!(found[0].size_bytes, Some(1000));
        assert!(
            sources::current().indexer("prowlarr").unwrap().fallback,
            "it must only be asked when the others found nothing"
        );
    }

    #[test]
    fn torrents_csv_keeps_only_rows_that_name_the_searched_title() {
        let body = json!({ "torrents": [
            { "name": "Gyaarah.Gyaarah.S01E01.1080p", "infohash": A, "size_bytes": 2000, "seeders": 71 },
            { "name": "Something else entirely", "infohash": B, "seeders": 500 }
        ] });
        let found = run("torrents-csv", &body, "Gyaarah Gyaarah");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seeders, Some(71));
        assert!(run("torrents-csv", &body, "").is_empty());
    }

    #[test]
    fn torrentio_names_a_row_after_the_show_and_the_file() {
        let body = json!({ "streams": [
            { "infoHash": A,
              "title": "India's Got Latent\nIGL_EP_01_ft_RaftaarMusic.mp4\n👤 47 💾 473.25 MB ⚙️ ThePirateBay" },
            { "infoHash": B,
              "title": "Indias Got Latent (2026)-MovieLinkHub.com-S02 Bouns E01-E01\n👤 3 💾 1.41 GB ⚙️ EXT" },
            { "infoHash": "cccccccccccccccccccccccccccccccccccccccc",
              "title": "Show\nfallback.mkv\n👤 1 💾 1 GB ⚙️ X",
              "behaviorHints": { "filename": "S01E01.mkv" } },
            { "title": "no hash\nfile.mkv" }
        ] });
        let found = run("torrentio", &body, "x");
        assert_eq!(found.len(), 3);
        assert_eq!(
            found[0].title,
            "India's Got Latent IGL_EP_01_ft_RaftaarMusic.mp4"
        );
        assert_eq!(found[0].seeders, Some(47));
        assert_eq!(
            found[0].size_bytes,
            Some((473.25 * (1u64 << 20) as f64).round() as u64)
        );
        // A single name is the release name, not a name plus the summary line.
        assert_eq!(
            found[1].title,
            "Indias Got Latent (2026)-MovieLinkHub.com-S02 Bouns E01-E01"
        );
        assert_eq!(
            found[1].size_bytes,
            Some((1.41 * (1u64 << 30) as f64).round() as u64)
        );
        // The structured file name wins over the second line.
        assert_eq!(found[2].title, "Show S01E01.mkv");
    }

    #[test]
    fn torrentio_reads_description_and_structured_file_size() {
        let body = json!({ "streams": [{
            "infoHash": A,
            "description": "Show S01E01 1080p\n👤 47",
            "behaviorHints": {
                "filename": "Show.S01E01.1080p.mkv",
                "videoSize": 2_000_000_000u64
            },
            "url": "https://credential.example/video?token=[REDACTED]"
        }] });
        let found = run("torrentio", &body, "x");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Show S01E01 1080p Show.S01E01.1080p.mkv");
        assert_eq!(found[0].seeders, Some(47));
        assert_eq!(found[0].size_bytes, Some(2_000_000_000));
        // Only the validated hash reaches playback, never a credential-bearing addon URL.
        assert!(
            found[0]
                .magnet
                .starts_with(&format!("magnet:?xt=urn:btih:{A}&dn="))
        );
    }

    #[test]
    fn torrentio_keeps_the_real_episode_of_a_pack_and_not_its_bonus_clip() {
        let stream = |title: &str| json!({ "infoHash": A, "title": format!("Show\n{title}\n👤 47 💾 1 GB ⚙️ X") });
        let body = json!({ "streams": [
            stream("IGL_Bonus_EP_1_ft. Arpit Bala.mp4"),
            stream("IGL_EP_01.mp4"),
            stream("IGL_EP_02.mp4"),
        ] });
        let found = run("torrentio", &body, "x");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Show IGL_EP_01.mp4");

        // With nothing but bonus clips the first one stands for the torrent.
        let body = json!({ "streams": [stream("Deleted.Scenes.mp4"), stream("Bloopers.mp4")] });
        let found = run("torrentio", &body, "x");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Show Deleted.Scenes.mp4");
    }

    #[test]
    fn yts_makes_a_row_per_torrent_named_like_a_scene_release() {
        let body = json!({ "status": "ok", "data": { "movies": [
            { "title": "Dangal", "year": 2016, "torrents": [
                { "hash": A, "quality": "1080p", "type": "bluray", "seeds": 30, "size_bytes": 2_000_000_000u64 },
                { "hash": B, "quality": "720p", "seeds": 10 },
                { "quality": "480p" }
            ] },
            { "title": "No year", "torrents": [ { "hash": "cccccccccccccccccccccccccccccccccccccccc", "quality": "720p" } ] }
        ] } });
        let found = run("yts", &body, "dangal");
        let titles: Vec<&str> = found.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Dangal (2016) 1080p bluray",
                "Dangal (2016) 720p",
                "No year 720p"
            ]
        );
        assert_eq!(found[0].seeders, Some(30));
        assert_eq!(found[0].size_bytes, Some(2_000_000_000));
        assert_eq!(found[1].size_bytes, None);
        // The year, when there is none, leaves no empty brackets behind.
        assert!(!found[2].title.contains('('));
        // A film nobody has ripped is an empty list.
        assert!(
            run(
                "yts",
                &json!({ "status": "ok", "data": { "movie_count": 0 } }),
                "x"
            )
            .is_empty()
        );
    }

    #[test]
    fn eztv_reads_its_torrent_list() {
        let body = json!({ "torrents": [
            { "filename": "Breaking.Bad.S01E01.720p", "hash": A, "size_bytes": "1000", "seeds": 0 },
            { "filename": "", "hash": B }
        ] });
        let found = run("eztv", &body, "x");
        assert_eq!(found.len(), 1);
        // The size arrives as text here.
        assert_eq!(found[0].size_bytes, Some(1000));
        assert_eq!(found[0].seeders, Some(0));
    }

    #[test]
    fn apibay_reads_numbers_sent_as_text_and_its_no_results_row() {
        let body = json!([
            { "name": "Show S01E01", "info_hash": A.to_uppercase(), "seeders": "27", "size": "1500" },
            { "name": "Zero size", "info_hash": B, "seeders": "0", "size": "0" },
            { "name": "No results returned", "info_hash": "0000000000000000000000000000000000000000" }
        ]);
        let found = run("apibay", &body, "x");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].info_hash, A);
        assert_eq!(found[0].seeders, Some(27));
        assert_eq!(found[0].size_bytes, Some(1500));
        // A size of "0" means unknown, not a release of no bytes.
        assert_eq!(found[1].size_bytes, None);
    }

    #[test]
    fn a_hash_is_read_in_hex_or_base32_and_anything_else_is_skipped() {
        let body = json!([
            { "name": "base32", "info_hash": "vqvqvqvqvqvqvqvqvqvqvqvqvqvqvqvq" },
            { "name": "short", "info_hash": "abc" }
        ]);
        let found = run("apibay", &body, "x");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].info_hash.len(), 40);
    }

    #[test]
    fn animetosho_reads_a_torznab_feed_through_its_attributes() {
        let feed = format!(
            "<?xml version=\"1.0\"?><rss version=\"2.0\" xmlns:torznab=\"http://torznab.com/schemas/2015/feed\"><channel><title>AnimeTosho</title>\
             <item><title>[SubsMix] Bocchi the Rock! - Season 01 (S01) V4 (BD 1080p HEVC)</title>\
             <description><![CDATA[<p>Source: <a href=\"https://nyaa.si/view/1\">Nyaa.si</a></p>]]></description>\
             <enclosure url=\"https://storage.animetosho.net/x.torrent?filename=a%29.torrent\" type=\"application/x-bittorrent\" length=\"25551692518\"/>\
             <torznab:attr name=\"category\" value=\"5070\"/><torznab:attr name=\"category\" value=\"2020\"/>\
             <torznab:attr name=\"size\" value=\"25551692518\"/>\
             <torznab:attr name=\"infohash\" value=\"{A}\"/>\
             <torznab:attr name=\"seeders\" value=\"59\"/><torznab:attr name=\"peers\" value=\"132\"/></item>\
             <item><title>No hash</title><torznab:attr name=\"seeders\" value=\"4\"/></item></channel></rss>"
        );
        let found = run_text("animetosho", &feed, "Bocchi the Rock");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].title,
            "[SubsMix] Bocchi the Rock! - Season 01 (S01) V4 (BD 1080p HEVC)"
        );
        assert_eq!(found[0].info_hash, A);
        assert_eq!(found[0].size_bytes, Some(25_551_692_518));
        assert_eq!(found[0].seeders, Some(59));
        assert_eq!(found[0].source, "animetosho");
    }

    #[test]
    fn rss_attributes_are_read_under_tag_at_attribute() {
        let items = rss_items(
            "<rss><item><title>t</title><enclosure url=\"u\" length=\"5\"/>\
             <guid isPermaLink=\"true\">g</guid><x:attr name=\"a\" value=\"1\"/>\
             <x:attr name=\"a\" value=\"2\"/><x:attr name=\"b\" value=\"&amp;\"/></item></rss>",
        );
        let item = &items[0];
        assert_eq!(item["enclosure@length"], "5");
        assert_eq!(item["enclosure@url"], "u");
        assert_eq!(item["guid@isPermaLink"], "true");
        assert_eq!(item["guid"], "g");
        // The first of a repeated attribute-style tag wins, and its value is decoded.
        assert_eq!(item["x:attr@a"], "1");
        assert_eq!(item["x:attr@b"], "&");
    }
}
