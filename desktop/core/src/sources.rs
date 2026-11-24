//! Where titles and releases come from, as data.
//!
//! Indexers move, change their address or die often, so what is searched and how is a JSON
//! document (`sources.json` in the akidev-sources repo, downloaded by `core/build.rs` when missing) rather than code. The app ships with
//! one copy and fetches a newer one in the background. A downloaded copy is used only if it
//! carries a valid signature, a higher version than the one in use, and passes `validate`;
//! otherwise the app keeps what it has. Nothing here runs downloaded code: a definition only
//! says which requests to make and which fields to read.

use crate::definition::{IndexerDef, MapperDef, ProviderDef, SubtitleDef};
use crate::model::Kind;
use crate::plugin::Plugin;
use crate::screens::Screens;
use crate::services::ServiceDef;
use anyhow::{Context, ensure};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

/// The newest version of the definition format this build understands. It reads that and the ones
/// before it. A document with a higher number is ignored, so a build is never fed definitions it would
/// misread. An app that knows only schema 1 refuses a document of schema 2, which is why the two are
/// published as two feeds (see `refresh`).
pub const SCHEMA: u64 = 2;

/// Where the schema 2 document is, under the feed: the same files as the schema 1 feed, in `v2/`.
const FEED_V2: &str = "v2";

/// Where the signed document is published.
const FEED_URL: &str = "https://sources.akidev.live";

/// The key the published document is signed with. The matching private key is kept by
/// whoever publishes, never in this repository.
const PUBLIC_KEY: &str = "e0776a868d11f70f94e6aaee3a28205cdc15002c246e97811b3100c4d4a04bbe";

const EMBEDDED: &str = include_str!("../../sources/sources.json");

/// A document is a few tens of kilobytes; anything much larger is not one.
const MAX_BYTES: usize = 1 << 20;

const MAX_VERSION: u64 = 1 << 40;

#[derive(Debug, Clone, Deserialize)]
pub struct Sources {
    pub schema: u64,
    /// Rises with every publish. A document that does not beat the one in use is ignored,
    /// so an old copy replayed from a cache cannot take definitions back.
    pub version: u64,
    /// Public trackers added to every magnet. Many indexers return none.
    pub trackers: Vec<String>,
    pub indexers: Vec<IndexerDef>,
    pub providers: Vec<ProviderDef>,
    /// Where the ids an indexer wants are found. Absent in a document from before there were any: the
    /// built-in ones are used.
    #[serde(default)]
    pub mappers: Vec<MapperDef>,
    /// Where subtitle files come from. Absent in a document from before there were any.
    #[serde(default)]
    pub subtitles: Vec<SubtitleDef>,
    /// Services that turn a release into a playable link. Only a plugin brings one: a signed document
    /// that did is refused, and in use this holds the plugins' own.
    #[serde(default)]
    pub services: Vec<ServiceDef>,
    /// Which providers answer for each kind, in order.
    pub chains: BTreeMap<Kind, Vec<String>>,
    /// What the screens show. Absent in a document from before there were any: the built-in ones are used.
    #[serde(default)]
    pub screens: Option<Screens>,
    /// The oldest version of the app that may run (`0.1.17`). An older one shows an update screen and
    /// nothing else. Absent: any version may.
    #[serde(default)]
    pub min_version: Option<String>,
    /// What the update screen says. Absent: a default.
    #[serde(default)]
    pub min_version_message: Option<String>,
}

impl Sources {
    /// The document's screens, or the built-in ones when it predates them.
    pub(crate) fn screens_or_built_in(&self) -> Screens {
        self.screens.clone().unwrap_or_else(|| {
            built_in()
                .screens
                .clone()
                .expect("the embedded definitions have screens")
        })
    }

    /// A mapper by id.
    pub fn mapper(&self, id: &str) -> Option<&MapperDef> {
        self.mappers.iter().find(|mapper| mapper.id == id)
    }

    pub fn indexer(&self, id: &str) -> Option<&IndexerDef> {
        self.indexers.iter().find(|indexer| indexer.id == id)
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderDef> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    /// The hosts the providers keep their pictures on.
    pub fn image_hosts(&self) -> impl Iterator<Item = &str> {
        self.providers
            .iter()
            .flat_map(|provider| provider.image_hosts.iter().map(String::as_str))
    }

    pub fn chain(&self, kind: Kind) -> &[String] {
        self.chains
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Rejects a document that parses but would break a search: the cheapest place to
    /// notice a typo is before it is installed.
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=SCHEMA).contains(&self.schema),
            "definitions use format {}, this app reads up to {SCHEMA}",
            self.schema
        );
        // A version near the top of the range could never be replaced.
        ensure!(self.version <= MAX_VERSION, "the version is out of range");
        ensure!(self.trackers.len() <= 32, "too many trackers");
        ensure!(
            (1..=64).contains(&self.indexers.len()),
            "the number of indexers is out of range"
        );
        ensure!(
            (1..=64).contains(&self.providers.len()),
            "the number of providers is out of range"
        );
        for (index, indexer) in self.indexers.iter().enumerate() {
            ensure!(
                self.indexers[..index]
                    .iter()
                    .all(|other| other.id != indexer.id),
                "indexer {} appears twice",
                indexer.id
            );
            indexer
                .check()
                .with_context(|| format!("indexer {}", indexer.id))?;
        }
        ensure!(self.services.len() <= 8, "too many services");
        for (index, service) in self.services.iter().enumerate() {
            service
                .check()
                .with_context(|| format!("service {}", service.id))?;
            ensure!(
                self.services[..index]
                    .iter()
                    .all(|other| other.id != service.id),
                "two services are called {}",
                service.id
            );
        }
        ensure!(self.mappers.len() <= 8, "too many mappers");
        for (index, mapper) in self.mappers.iter().enumerate() {
            ensure!(
                self.mappers[..index]
                    .iter()
                    .all(|other| other.id != mapper.id),
                "mapper {} appears twice",
                mapper.id
            );
            mapper
                .check()
                .with_context(|| format!("mapper {}", mapper.id))?;
        }
        for indexer in &self.indexers {
            if let Some(id) = indexer.mapper_id() {
                ensure!(
                    self.mapper(id).is_some_and(|mapper| indexer
                        .kinds
                        .iter()
                        .any(|kind| mapper.kinds.contains(kind))),
                    "indexer {} is asked through a mapper {id} that is not there or maps none of its kinds",
                    indexer.id
                );
            }
        }
        for (index, provider) in self.providers.iter().enumerate() {
            ensure!(
                self.providers[..index]
                    .iter()
                    .all(|other| other.id != provider.id),
                "provider {} appears twice",
                provider.id
            );
            provider
                .check()
                .with_context(|| format!("provider {}", provider.id))?;
        }
        // A version nobody can read must not lock anyone out, so the document that carries it is refused.
        if let Some(min) = &self.min_version {
            ensure!(
                crate::update::Version::parse(min).is_some(),
                "min_version {min:?} is not major.minor.patch"
            );
        }
        ensure!(
            self.min_version_message
                .as_ref()
                .is_none_or(|message| message.chars().count() <= 300),
            "min_version_message is too long"
        );
        ensure!(self.subtitles.len() <= 16, "too many subtitle sources");
        for (index, source) in self.subtitles.iter().enumerate() {
            ensure!(
                self.subtitles[..index]
                    .iter()
                    .all(|other| other.id != source.id),
                "subtitle source {} appears twice",
                source.id
            );
            source
                .check()
                .with_context(|| format!("subtitle source {}", source.id))?;
        }
        if let Some(screens) = &self.screens {
            screens.check().context("the screens")?;
            for row in &screens.home {
                if let Some(id) = &row.provider {
                    ensure!(
                        self.provider(id)
                            .is_some_and(|provider| provider.kinds.contains(&row.kind)),
                        "the row {:?} names a provider that does not serve {}",
                        row.title,
                        row.kind.as_str()
                    );
                }
            }
        }
        for kind in Kind::BUILT_IN {
            let chain = self.chain(kind);
            ensure!(
                (1..=8).contains(&chain.len()),
                "the providers for {} are out of range",
                kind.as_str()
            );
            ensure!(
                chain
                    .iter()
                    .enumerate()
                    .all(|(at, id)| !chain[..at].contains(id)),
                "{} names a provider twice",
                kind.as_str()
            );
            for id in chain {
                let provider = self.provider(id);
                ensure!(
                    provider.is_some(),
                    "{} names an unknown provider {id}",
                    kind.as_str()
                );
                ensure!(
                    provider.is_some_and(|provider| provider.kinds.contains(&kind)),
                    "provider {id} does not serve {}",
                    kind.as_str()
                );
            }
        }
        Ok(())
    }
}

/// Parses and validates a document. Used for the embedded copy, a downloaded one and the
/// publishing tool, so all three accept exactly the same thing.
pub fn parse(bytes: &[u8]) -> anyhow::Result<Sources> {
    ensure!(bytes.len() <= MAX_BYTES, "the document is too large");
    let sources: Sources = serde_json::from_slice(bytes).context("reading the definitions")?;
    // Only a plugin brings code. A signed document that did would hand it to every app.
    ensure!(
        sources
            .indexers
            .iter()
            .all(|indexer| indexer.code.is_none())
            && sources
                .providers
                .iter()
                .chain(sources.mappers.iter().map(|mapper| &mapper.provider))
                .all(|provider| provider.code.is_none()),
        "a signed document cannot bring code"
    );
    ensure!(
        sources.services.is_empty(),
        "a signed document cannot bring a service"
    );
    sources.validate()?;
    Ok(sources)
}

/// Bytes from hex text. Only hex digits are accepted, so no input can make this slice
/// through a character.
pub fn decode_hex(text: &str) -> anyhow::Result<Vec<u8>> {
    let text = text.trim();
    ensure!(
        text.len().is_multiple_of(2) && text.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "not hex"
    );
    // Every byte is an ASCII digit here, so slicing in twos cannot split a character.
    (0..text.len())
        .step_by(2)
        .map(|at| Ok(u8::from_str_radix(&text[at..at + 2], 16)?))
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Checks a signature made with the publisher's key.
pub fn verify(bytes: &[u8], signature_hex: &str) -> anyhow::Result<()> {
    verify_with(PUBLIC_KEY, bytes, signature_hex)
}

fn verify_with(key_hex: &str, bytes: &[u8], signature_hex: &str) -> anyhow::Result<()> {
    let key: [u8; 32] = decode_hex(key_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("bad key"))?;
    let signature: [u8; 64] = decode_hex(signature_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("bad signature length"))?;
    VerifyingKey::from_bytes(&key)?
        .verify_strict(bytes, &Signature::from_bytes(&signature))
        .map_err(|_| anyhow::anyhow!("the signature does not match"))
}

/// What is in use: the document (built in, or downloaded and newer), the plugins switched on, and the
/// two put together, which is what a search reads.
struct InUse {
    document: Arc<Sources>,
    plugins: Vec<Plugin>,
    merged: Arc<Sources>,
}

fn in_use() -> &'static RwLock<InUse> {
    static IN_USE: OnceLock<RwLock<InUse>> = OnceLock::new();
    IN_USE.get_or_init(|| {
        let document =
            Arc::new(parse(EMBEDDED.as_bytes()).expect("the embedded definitions are valid"));
        RwLock::new(InUse {
            merged: document.clone(),
            document,
            plugins: Vec::new(),
        })
    })
}

/// The document with the plugins added, each in turn. A plugin that cannot be added (it clashes with
/// a source that is there) is left out, and its problem is the answer for it.
fn put_together(
    document: &Arc<Sources>,
    plugins: &[Plugin],
) -> (Arc<Sources>, Vec<Plugin>, Vec<Result<(), String>>) {
    let mut merged = (**document).clone();
    let mut kept = Vec::new();
    let mut results = Vec::new();
    for plugin in plugins {
        match crate::plugin::merge(&merged, plugin) {
            Ok(next) => {
                merged = next;
                kept.push(plugin.clone());
                results.push(Ok(()));
            }
            Err(error) => results.push(Err(format!("{error:#}"))),
        }
    }
    (Arc::new(merged), kept, results)
}

/// Held while the sources in use are changed, so two changes do not build on the same old state. The
/// work of putting them together is done under this and not under the lock that every search reads
/// through, which is only taken to swap the result in.
fn changing() -> std::sync::MutexGuard<'static, ()> {
    static CHANGING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    CHANGING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Puts these plugins to use in place of the ones that were. Gives, for each, whether it could be.
pub(crate) fn use_plugins(plugins: &[Plugin]) -> Vec<Result<(), String>> {
    let _changing = changing();
    let document = in_use().read().unwrap().document.clone();
    let (merged, kept, results) = put_together(&document, plugins);
    let mut slot = in_use().write().unwrap();
    slot.merged = merged;
    slot.plugins = kept;
    results
}

/// The document built into this build.
fn built_in() -> &'static Sources {
    static BUILT_IN: OnceLock<Sources> = OnceLock::new();
    BUILT_IN.get_or_init(|| parse(EMBEDDED.as_bytes()).expect("the embedded definitions are valid"))
}

/// The screens in use: the document's own, or the built-in ones when it predates them.
pub fn screens() -> Screens {
    current().screens_or_built_in()
}

/// The hosts a picture may come from: those the document in use names, and those the built-in one
/// names. A document published before the providers named any must not leave the site with no pictures.
pub fn image_hosts() -> Vec<String> {
    let current = current();
    current
        .image_hosts()
        .chain(built_in().image_hosts())
        .map(str::to_string)
        .collect()
}

/// The definitions in use right now.
pub fn current() -> Arc<Sources> {
    in_use().read().unwrap().merged.clone()
}

/// Makes a downloaded document the one in use, if it is newer. Returns whether it was.
fn adopt(sources: Sources) -> bool {
    let _changing = changing();
    let (document, plugins) = {
        let slot = in_use().read().unwrap();
        (slot.document.clone(), slot.plugins.clone())
    };
    if !newer(&sources, &document) {
        return false;
    }
    let document = Arc::new(sources);
    let (merged, kept, results) = put_together(&document, &plugins);
    for problem in results.iter().filter_map(|result| result.as_ref().err()) {
        tracing::warn!("[sources] a plugin no longer fits the new document: {problem}");
    }
    let mut slot = in_use().write().unwrap();
    slot.document = document;
    slot.merged = merged;
    slot.plugins = kept;
    true
}

/// Held by a test that changes the definitions in use, and by tests whose cache keys depend on
/// them, so the two cannot run at the same time.
#[cfg(test)]
pub(crate) fn definitions_change_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

const DOCUMENT: &str = "sources.json";
const SIGNATURE: &str = "sources.sig";

/// What the update screen says when `running` (this build's version) is older than the oldest the
/// published document allows, or `None` when it may run. A document that cannot be read says nothing,
/// so a failure never locks anyone out.
pub fn update_required(running: &str) -> Option<String> {
    required(&current(), running)
}

fn required(sources: &Sources, running: &str) -> Option<String> {
    let running = crate::update::Version::parse(running)?;
    let min = crate::update::Version::parse(sources.min_version.as_deref()?)?;
    (running < min).then(|| {
        sources.min_version_message.clone().unwrap_or_else(|| {
            "This version of Akidev can no longer be used. Update to carry on.".into()
        })
    })
}

/// Uses the copy saved by an earlier run, when it is still valid and newer than the one
/// built in. It is checked again on every start, so a damaged file does no harm.
pub fn load_saved(dir: &Path) {
    load_saved_with(PUBLIC_KEY, dir)
}

fn load_saved_with(key: &str, dir: &Path) {
    let read = || -> anyhow::Result<Sources> {
        let bytes = std::fs::read(dir.join(DOCUMENT))?;
        verify_with(key, &bytes, &std::fs::read_to_string(dir.join(SIGNATURE))?)?;
        parse(&bytes)
    };
    match read() {
        Ok(sources) => {
            tracing::info!("[sources] saved copy, version {}", sources.version);
            adopt(sources);
        }
        Err(error) if dir.join(DOCUMENT).exists() => {
            tracing::warn!("[sources] ignoring the saved copy: {error:#}")
        }
        Err(_) => {}
    }
}

/// Writes a file so that a crash leaves the old one or the new one, never half of one.
fn save(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// How often a running app looks for a newer document.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// After a failure (offline, a half-published pair of files) it looks again sooner.
const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Looks for a newer document now and then for as long as the app runs. A failure (offline,
/// the host down) only means the definitions in use stay in use.
pub async fn keep_fresh(dir: std::path::PathBuf) {
    loop {
        let wait = match refresh(&dir).await {
            Ok(_) => CHECK_EVERY,
            Err(error) => {
                tracing::warn!("[sources] not updated: {error:#}");
                RETRY_AFTER
            }
        };
        tokio::time::sleep(wait).await;
    }
}

/// Fetches the published document and, if it is genuine and newer, starts using it and saves
/// it for the next run. Returns the version now in use.
pub async fn refresh(dir: &Path) -> anyhow::Result<u64> {
    refresh_from(FEED_URL, PUBLIC_KEY, dir).await
}

/// Whether `candidate` should replace `in_use`: a higher version, or the same version in a newer schema (the
/// two feeds carry one version number, and a build that could only reach the schema 1 feed for a while
/// should move to schema 2 when it can).
fn newer(candidate: &Sources, in_use: &Sources) -> bool {
    candidate.version > in_use.version
        || (candidate.version == in_use.version && candidate.schema > in_use.schema)
}

/// The document of one feed, the signature checked and the document read.
async fn fetch_signed(feed: &str, key: &str) -> anyhow::Result<(Vec<u8>, String, Sources)> {
    let client = crate::http::client();
    let fetch = |name: &'static str| async move {
        let response = client
            .get(format!("{feed}/{name}"))
            .send()
            .await?
            .error_for_status()?;
        anyhow::Ok(crate::http::read_capped(response, MAX_BYTES).await?)
    };
    let (document, signature) =
        futures::future::try_join(fetch(DOCUMENT), fetch(SIGNATURE)).await?;
    let signature = std::str::from_utf8(&signature)
        .context("the signature is not text")?
        .to_string();
    verify_with(key, &document, &signature)?;
    let sources = parse(&document)?;
    Ok((document, signature, sources))
}

/// Looks first in the schema 2 feed, which only builds that know schema 2 read, and in the schema 1 feed
/// when there is nothing newer there: nothing at all, or something that is not genuine, not readable or not
/// newer than what is in use. Both are signed by the same key and published with one version number.
async fn refresh_from(feed: &str, key: &str, dir: &Path) -> anyhow::Result<u64> {
    let in_use = current();
    let (document, signature, sources) = match fetch_signed(&format!("{feed}/{FEED_V2}"), key).await
    {
        Ok(found) if newer(&found.2, &in_use) => found,
        // A genuine schema 2 document that is not newer: the schema 1 feed may still be.
        Ok(_) => match fetch_signed(feed, key).await {
            Ok(found) if newer(&found.2, &in_use) => found,
            _ => return Ok(in_use.version),
        },
        Err(error) => {
            // No schema 2 feed yet is the usual case (404). Anything else is worth a line in the log.
            let missing = error
                .downcast_ref::<reqwest::Error>()
                .and_then(reqwest::Error::status)
                == Some(reqwest::StatusCode::NOT_FOUND);
            if missing {
                tracing::debug!("[sources] no schema 2 feed: using schema 1");
            } else {
                tracing::warn!(
                    "[sources] the schema 2 feed is not usable ({error:#}): using schema 1"
                );
            }
            let found = fetch_signed(feed, key).await?;
            if !newer(&found.2, &in_use) {
                return Ok(in_use.version);
            }
            found
        }
    };
    let version = sources.version;
    // Saved before it is used, so what is in use is always what the next run will find.
    std::fs::create_dir_all(dir)?;
    save(&dir.join(DOCUMENT), &document)?;
    save(&dir.join(SIGNATURE), signature.as_bytes())?;
    adopt(sources);
    tracing::info!("[sources] updated to version {version}");
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{Value, json};

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn key_hex() -> String {
        encode_hex(signing_key().verifying_key().as_bytes())
    }

    fn embedded_json() -> Value {
        serde_json::from_str(EMBEDDED).unwrap()
    }

    /// The embedded document with `change` applied, as bytes.
    fn changed(change: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut document = embedded_json();
        change(&mut document);
        serde_json::to_vec(&document).unwrap()
    }

    #[test]
    fn the_embedded_definitions_are_valid_and_complete() {
        let sources = parse(EMBEDDED.as_bytes()).unwrap();
        assert!(!sources.indexers.is_empty());
        assert!(!sources.providers.is_empty());
        assert!(!sources.trackers.is_empty());
        let mut older = embedded_json();
        older["indexers"][5]["response"]["fields"]
            .as_object_mut()
            .unwrap()
            .remove("file_idx");
        assert!(parse(&serde_json::to_vec(&older).unwrap()).is_ok());
        assert!(
            sources
                .mapper("kitsu")
                .unwrap()
                .provider
                .base_url
                .starts_with("https://")
        );
        assert!(
            sources
                .providers
                .iter()
                .all(|provider| provider.id != "anime-kitsu")
        );
        for kind in Kind::BUILT_IN {
            assert!(!sources.chain(kind).is_empty());
        }
        // The public key is a real one and not a placeholder.
        assert!(PUBLIC_KEY.bytes().any(|byte| byte != b'0'));
        assert!(
            VerifyingKey::from_bytes(&decode_hex(PUBLIC_KEY).unwrap().try_into().unwrap()).is_ok()
        );
    }

    #[test]
    fn the_embedded_providers_name_the_image_hosts_the_server_always_allowed() {
        let sources = parse(EMBEDDED.as_bytes()).unwrap();
        let mut hosts: Vec<&str> = sources.image_hosts().collect();
        hosts.sort_unstable();
        assert_eq!(
            hosts,
            [
                "anilist.co",
                "kitsu.app",
                "media-amazon.com",
                "metahub.space",
                "tmdb.org",
                "tvmaze.com",
                "yts.gg",
                "yts.mx",
            ]
        );
        let with_a_path = changed(|d| d["providers"][0]["image_hosts"] = json!(["anilist.co/a"]));
        assert!(parse(&with_a_path).is_err());
    }

    #[test]
    fn the_embedded_screens_are_what_the_apps_have_always_shown() {
        let screens = parse(EMBEDDED.as_bytes()).unwrap().screens.unwrap();
        let tabs: Vec<_> = screens
            .tabs
            .iter()
            .map(|tab| (tab.kind, tab.slug.as_str(), tab.label.as_str()))
            .collect();
        assert_eq!(
            tabs,
            [
                (Kind::Anime, "anime", "Anime"),
                (Kind::Series, "series", "Series"),
                (Kind::Movie, "films", "Films"),
            ]
        );
        let sorts: Vec<_> = screens
            .sorts
            .iter()
            .map(|option| (option.sort.as_str(), option.label.as_str()))
            .collect();
        assert_eq!(
            sorts,
            [
                ("popularity", "Popular"),
                ("trending", "Trending"),
                ("score", "Top rated"),
                ("recent", "Newest"),
                ("title", "A–Z"),
            ]
        );
        let rows: Vec<_> = screens
            .home
            .iter()
            .map(|row| {
                (
                    row.title.as_str(),
                    row.kind,
                    row.sort.as_str(),
                    row.genre.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("Trending Anime", Kind::Anime, "trending", None),
                ("New Releases", Kind::Movie, "recent", None),
                ("Popular Series", Kind::Series, "popularity", None),
                ("Newly Released Anime", Kind::Anime, "recent", None),
                ("Most Watched Films", Kind::Movie, "popularity", None),
                ("New Series", Kind::Series, "recent", None),
                ("Top Rated Anime", Kind::Anime, "score", None),
                ("Action Films", Kind::Movie, "popularity", Some("Action")),
                ("Drama Series", Kind::Series, "score", Some("Drama")),
                ("Romance Anime", Kind::Anime, "popularity", Some("Romance")),
            ]
        );
        assert_eq!(screens.genres(Some(Kind::Anime)).len(), 18);
        assert_eq!(screens.genres(Some(Kind::Anime))[7], "Mahou Shoujo");
        assert_eq!(
            screens.genres(Some(Kind::Series)),
            screens.genres(Some(Kind::Movie))
        );
        assert_eq!(screens.genres(Some(Kind::Movie)).len(), 19);
        assert_eq!(screens.genres(None).len(), 10);
    }

    #[test]
    fn a_document_from_before_image_hosts_leaves_the_built_in_hosts_allowed() {
        let older = changed(|d| {
            for provider in d["providers"].as_array_mut().unwrap() {
                provider.as_object_mut().unwrap().remove("image_hosts");
            }
        });
        let older = parse(&older).unwrap();
        assert_eq!(older.image_hosts().count(), 0);
        // What is in use is the built-in document here, so the union is its hosts.
        assert!(image_hosts().iter().any(|host| host == "anilist.co"));
        assert!(image_hosts().iter().any(|host| host == "metahub.space"));
    }

    #[test]
    fn an_indexer_asked_through_a_mapper_that_is_not_there_or_maps_none_of_its_kinds_is_refused() {
        let refused = |what: &str, change: fn(&mut Value)| {
            assert!(parse(&changed(change)).is_err(), "{what} was accepted");
        };
        refused("an unknown mapper", |d| {
            for indexer in d["indexers"].as_array_mut().unwrap() {
                if indexer["id"] == "torrentio" {
                    indexer["map"] = json!("nothing");
                }
            }
        });
        refused("a mapper that maps none of the indexer's kinds", |d| {
            d["mappers"][0]["kinds"] = json!(["anime"]);
            for indexer in d["indexers"].as_array_mut().unwrap() {
                if indexer["id"] == "yts" {
                    indexer["map"] = json!("kitsu");
                }
            }
        });
        refused("a mapper with a capital in its id", |d| {
            d["mappers"][0]["id"] = json!("Kitsu")
        });
        refused("two mappers with one id", |d| {
            let first = d["mappers"][0].clone();
            d["mappers"].as_array_mut().unwrap().push(first);
        });
    }

    #[test]
    fn a_mapper_cannot_take_the_name_of_a_hole_a_request_already_has() {
        for name in ["term", "imdb", "season", "episode", "imdb_or_term"] {
            let bad = changed(|d| d["mappers"][0]["id"] = json!(name));
            assert!(parse(&bad).is_err(), "{name}");
        }
    }

    #[test]
    fn a_document_from_before_screens_is_still_accepted() {
        let older = changed(|d| {
            d.as_object_mut().unwrap().remove("screens");
        });
        assert!(parse(&older).unwrap().screens.is_none());
        assert!(!screens().tabs.is_empty());
    }

    #[test]
    fn screens_that_would_show_nothing_or_break_a_page_are_refused() {
        let refused = |what: &str, change: fn(&mut Value)| {
            assert!(parse(&changed(change)).is_err(), "{what} was accepted");
        };
        refused("no tabs", |d| d["screens"]["tabs"] = json!([]));
        refused("a slug with a slash", |d| {
            d["screens"]["tabs"][0]["slug"] = json!("a/b")
        });
        refused("a repeated slug", |d| {
            d["screens"]["tabs"][1]["slug"] = json!("anime")
        });
        refused("a repeated label", |d| {
            d["screens"]["tabs"][1]["label"] = json!("Anime")
        });
        refused("a slug that is a page of the site", |d| {
            d["screens"]["tabs"][1]["slug"] = json!("search")
        });
        refused("a tab with no tag", |d| {
            d["screens"]["tabs"][0]["tag"] = json!("")
        });
        refused("no attribution", |d| {
            d["screens"]["about"]["attribution"] = json!("")
        });
        refused("an empty credit name", |d| {
            d["screens"]["about"]["catalogues"] = json!([""])
        });
        refused("no sorts", |d| d["screens"]["sorts"] = json!([]));
        refused("an unknown sort", |d| {
            d["screens"]["sorts"][0]["sort"] = json!("loudest")
        });
        refused("genres for no kind", |d| {
            d["screens"]["genres"]["books"] = json!(["Poetry"])
        });
        refused("no home rows", |d| d["screens"]["home"] = json!([]));
        refused("a row for a kind with no tab", |d| {
            d["screens"]["tabs"] = json!([{ "kind": "anime", "slug": "anime", "label": "Anime" }]);
        });
    }

    #[test]
    fn a_document_that_would_break_a_search_is_refused() {
        let refused = |what: &str, bytes: Vec<u8>| {
            assert!(parse(&bytes).is_err(), "{what} was accepted");
        };
        refused("a newer format", changed(|d| d["schema"] = json!(3)));
        refused(
            "a repeated indexer id",
            changed(|d| d["indexers"][1]["id"] = json!("subsplease")),
        );
        refused(
            "an indexer with no host",
            changed(|d| d["indexers"][0]["hosts"] = json!([])),
        );
        refused(
            "a host that is not http",
            changed(|d| d["indexers"][0]["hosts"] = json!(["ftp://x"])),
        );
        refused(
            "a bad pattern",
            changed(|d| d["indexers"][4]["response"]["dedupe"]["avoid"] = json!("(unclosed")),
        );
        refused(
            "a misspelt hole",
            changed(|d| d["indexers"][1]["request"]["query"]["q"] = json!("{tem}")),
        );
        refused(
            "a chain naming nobody",
            changed(|d| d["chains"]["anime"] = json!(["nobody"])),
        );
        refused(
            "a kind with no provider",
            changed(|d| d["chains"]["movie"] = json!([])),
        );
        refused(
            "a provider address that is not http",
            changed(|d| d["providers"][2]["base_url"] = json!("tvmaze")),
        );
        refused("no indexers", changed(|d| d["indexers"] = json!([])));
        refused(
            "an answer reused for ever",
            changed(|d| d["providers"][1]["list"]["ttl_secs"] = json!(u64::MAX)),
        );
        refused(
            "a request that leaves the service",
            changed(|d| {
                d["providers"][2]["search"]["request"]["url"] = json!("https://evil.example/{term}")
            }),
        );
        refused(
            "a request that lets the term change the host",
            changed(|d| d["providers"][2]["search"]["request"]["url"] = json!("{base}{term}")),
        );
        refused(
            "a base address with braces",
            changed(|d| d["providers"][2]["base_url"] = json!("https://x.example/{")),
        );
        refused(
            "pages too small to read in a few requests",
            changed(|d| d["providers"][1]["list"]["paging"]["size"] = json!(1)),
        );
        refused(
            "a page of no rows",
            changed(|d| d["providers"][1]["list"]["paging"]["per_page"] = json!(0)),
        );
        refused(
            "a window of something that cannot be asked",
            changed(|d| d["providers"][1]["list"]["paging"]["window"]["when"] = json!(["gnere"])),
        );
        refused(
            "days that overflow",
            changed(|d| {
                d["providers"][2]["list"]["drop"][1]["after_today"]["grace_days"] =
                    json!(99_999_999)
            }),
        );
        refused(
            "a search that counts pages of no known size",
            changed(|d| {
                d["providers"][3]["search"]
                    .as_object_mut()
                    .unwrap()
                    .remove("per_page");
            }),
        );
        refused(
            "a list that needs the genre nobody asked for",
            changed(|d| d["providers"][2]["list"]["request"]["url"] = json!("{base}/{genre}")),
        );
        refused(
            "a provider twice in a chain",
            changed(|d| d["chains"]["series"] = json!(["cinemeta", "cinemeta"])),
        );
        refused(
            "a bad pattern in a related list",
            changed(|d| {
                d["providers"][0]["detail"]["related"]["drop"][0]["path"] =
                    json!({ "regex": { "from": "type", "pattern": "(" } })
            }),
        );
        refused(
            "an endless rest",
            changed(|d| d["indexers"][0]["cooldown_secs"] = json!(u64::MAX)),
        );
        refused(
            "an endless request interval",
            changed(|d| d["indexers"][0]["min_interval_secs"] = json!(3601)),
        );
        refused(
            "a zero daily request limit",
            changed(|d| d["indexers"][0]["max_requests_per_day"] = json!(0)),
        );
        refused(
            "a version that cannot be replaced",
            changed(|d| d["version"] = json!(u64::MAX)),
        );
        refused(
            "a provider on the wrong kind",
            changed(|d| d["chains"]["series"] = json!(["yts"])),
        );
        refused("not JSON", b"nope".to_vec());
        refused("an oversized document", vec![b' '; MAX_BYTES + 1]);
    }

    #[test]
    fn fields_a_newer_format_adds_are_ignored() {
        let bytes = changed(|d| {
            d["something_new"] = json!(true);
            d["indexers"][0]["something_new"] = json!(1);
        });
        assert!(parse(&bytes).is_ok());
    }

    #[test]
    fn a_signature_is_valid_only_for_the_exact_document_and_key() {
        let document = b"the document";
        let signature = encode_hex(&signing_key().sign(document).to_bytes());
        assert!(verify_with(&key_hex(), document, &signature).is_ok());
        assert!(verify_with(&key_hex(), b"the document.", &signature).is_err());
        assert!(verify_with(&key_hex(), document, &signature[2..]).is_err());
        assert!(verify_with(&key_hex(), document, "not hex").is_err());
        // Text that is not ASCII is refused, not sliced through.
        assert!(verify_with(&key_hex(), document, "a€").is_err());
        assert!(decode_hex("é9").is_err());
        let other = SigningKey::from_bytes(&[8; 32]);
        let forged = encode_hex(&other.sign(document).to_bytes());
        assert!(verify_with(&key_hex(), document, &forged).is_err());
    }

    /// Serves a signed document, and counts nothing else: only the two files.
    async fn feed(document: Vec<u8>, signature: String) -> String {
        use axum::{Router, routing::get};
        let app = Router::new()
            .route("/sources.json", get(move || async move { document }))
            .route("/sources.sig", get(move || async move { signature }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    /// Serves two feeds: schema 1 at the root and, when there is one, schema 2 under `v2/`.
    async fn feeds(older: (Vec<u8>, String), newer: Option<(Vec<u8>, String)>) -> String {
        use axum::{Router, routing::get};
        let mut app = Router::new()
            .route("/sources.json", get(move || async move { older.0 }))
            .route("/sources.sig", get(move || async move { older.1 }));
        if let Some((document, signature)) = newer {
            app = app
                .route("/v2/sources.json", get(move || async move { document }))
                .route("/v2/sources.sig", get(move || async move { signature }));
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    fn signed(document: Vec<u8>) -> (Vec<u8>, String) {
        let signature = encode_hex(&signing_key().sign(&document).to_bytes());
        (document, signature)
    }

    #[test]
    fn schema_1_and_2_documents_are_read_and_a_newer_one_is_not() {
        for schema in [1, 2] {
            assert!(
                parse(&changed(|d| d["schema"] = json!(schema))).is_ok(),
                "{schema}"
            );
        }
        assert!(parse(&changed(|d| d["schema"] = json!(3))).is_err());
        assert!(parse(&changed(|d| d["schema"] = json!(0))).is_err());
    }

    #[tokio::test]
    async fn the_schema_2_feed_is_used_when_there_is_one_and_the_schema_1_feed_when_there_is_not_or_it_is_bad()
     {
        let _held = definitions_change_lock().lock().await;
        let in_use = current().version;
        let at = |plus: u64, schema: u64, trackers: &str| {
            let trackers = trackers.to_string();
            changed(move |d| {
                d["version"] = json!(in_use + plus);
                d["schema"] = json!(schema);
                d["trackers"] = json!([trackers]);
            })
        };
        let key = key_hex();

        // Only the schema 1 feed is published: it is used.
        let dir = temp_dir("feeds-1");
        let base = feeds(signed(at(100, 1, "udp://one.example:1")), None).await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 100);
        assert_eq!(current().trackers, ["udp://one.example:1"]);

        // Both are published with one version: the schema 2 one is used.
        let base = feeds(
            signed(at(200, 1, "udp://old.example:1")),
            Some(signed(at(200, 2, "udp://new.example:2"))),
        )
        .await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 200);
        assert_eq!(current().trackers, ["udp://new.example:2"]);
        assert!(
            std::fs::read_to_string(dir.join(DOCUMENT))
                .unwrap()
                .contains("\"schema\": 2")
                || std::fs::read_to_string(dir.join(DOCUMENT))
                    .unwrap()
                    .contains("\"schema\":2")
        );

        // The schema 1 feed was all there was for a while, at version N. When the schema 2 feed appears at the
        // same version, a build moves to it: the same version in a newer schema counts as newer.
        let base = feeds(signed(at(250, 1, "udp://same.example:1")), None).await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 250);
        assert_eq!(current().schema, 1);
        let base = feeds(
            signed(at(250, 1, "udp://same.example:1")),
            Some(signed(at(250, 2, "udp://same.example:2"))),
        )
        .await;
        refresh_from(&base, &key, &dir).await.unwrap();
        assert_eq!(
            (current().schema, current().trackers.clone()),
            (2, vec!["udp://same.example:2".to_string()])
        );

        // A genuine schema 2 feed that is older than what is in use does not hide a newer schema 1 feed.
        let base = feeds(
            signed(at(260, 1, "udp://root-newer.example:1")),
            Some(signed(at(240, 2, "udp://v2-stale.example:2"))),
        )
        .await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 260);
        assert_eq!(current().trackers, ["udp://root-newer.example:1"]);

        // A schema 2 feed that is not genuine (signed by another key) is left alone, and the schema 1 one is used.
        let other = SigningKey::from_bytes(&[9; 32]);
        let forged = {
            let document = at(300, 2, "udp://forged.example:3");
            let signature = encode_hex(&other.sign(&document).to_bytes());
            (document, signature)
        };
        let base = feeds(signed(at(300, 1, "udp://honest.example:3")), Some(forged)).await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 300);
        assert_eq!(current().trackers, ["udp://honest.example:3"]);

        // A schema 2 feed that holds a format this build does not read is left alone too.
        let base = feeds(
            signed(at(400, 1, "udp://fallback.example:4")),
            Some(signed(at(400, 3, "udp://future.example:4"))),
        )
        .await;
        assert_eq!(refresh_from(&base, &key, &dir).await.unwrap(), in_use + 400);
        assert_eq!(current().trackers, ["udp://fallback.example:4"]);

        // Neither feed is there: nothing changes and it is an error, as before.
        let nothing = feeds(signed(at(500, 1, "udp://x.example:5")), None).await;
        let broken = nothing.replace("127.0.0.1", "127.0.0.1:1/nowhere");
        assert!(refresh_from(&broken, &key, &dir).await.is_err());
        assert_eq!(current().version, in_use + 400);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("akidev-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn a_newer_signed_document_is_adopted_saved_and_reloaded_but_never_rolled_back() {
        let _held = definitions_change_lock().lock().await;
        let dir = temp_dir("sources");
        let in_use = current().version;

        // A genuine, newer document is used and saved.
        let newer = changed(|d| {
            d["version"] = json!(in_use + 10);
            d["trackers"] = json!(["udp://new.example:1"]);
        });
        let signed = encode_hex(&signing_key().sign(&newer).to_bytes());
        let base = feed(newer.clone(), signed.clone()).await;
        assert_eq!(
            refresh_from(&base, &key_hex(), &dir).await.unwrap(),
            in_use + 10
        );
        assert_eq!(current().trackers, ["udp://new.example:1"]);
        assert!(dir.join(DOCUMENT).exists());

        // An older one that is genuine is not taken: a replayed copy cannot undo an update.
        let older = changed(|d| d["version"] = json!(in_use + 5));
        let signed_older = encode_hex(&signing_key().sign(&older).to_bytes());
        let base = feed(older, signed_older).await;
        assert_eq!(
            refresh_from(&base, &key_hex(), &dir).await.unwrap(),
            in_use + 10
        );

        // A document altered after signing is refused, and nothing changes.
        let forged = changed(|d| d["version"] = json!(in_use + 99));
        let base = feed(forged, signed).await;
        assert!(refresh_from(&base, &key_hex(), &dir).await.is_err());
        assert_eq!(current().version, in_use + 10);

        // The saved copy is used by the next run, and a damaged one is ignored.
        load_saved_with(&key_hex(), &dir);
        assert_eq!(current().version, in_use + 10);
        std::fs::write(dir.join(DOCUMENT), b"damaged").unwrap();
        load_saved_with(&key_hex(), &dir);
        assert_eq!(current().version, in_use + 10);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn with_minimum(min: Option<&str>, message: Option<&str>) -> Sources {
        let mut sources = parse(EMBEDDED.as_bytes()).unwrap();
        sources.min_version = min.map(str::to_string);
        sources.min_version_message = message.map(str::to_string);
        sources
    }

    #[test]
    fn a_build_older_than_the_published_minimum_must_update_and_a_newer_or_equal_one_runs() {
        let sources = with_minimum(Some("0.1.17"), Some("Please update."));
        assert_eq!(
            required(&sources, "0.1.16").as_deref(),
            Some("Please update.")
        );
        assert_eq!(
            required(&sources, "0.1.9").as_deref(),
            Some("Please update.")
        );
        assert_eq!(required(&sources, "0.1.17"), None);
        assert_eq!(required(&sources, "0.2.0"), None);
        assert_eq!(required(&sources, "1.0.0"), None);
        // Versions compare by number: 0.1.9 is older than 0.1.10.
        assert!(required(&with_minimum(Some("0.1.10"), None), "0.1.9").is_some());
    }

    #[test]
    fn nothing_locks_anyone_out_unless_the_document_clearly_says_so() {
        // No minimum, or this build's version cannot be read: it runs.
        assert_eq!(required(&with_minimum(None, None), "0.0.1"), None);
        assert_eq!(required(&with_minimum(Some("0.1.17"), None), "dev"), None);
        // The default message is used when there is none.
        assert!(required(&with_minimum(Some("9.0.0"), None), "0.1.1").is_some());
        // A minimum nobody can read makes the whole document unusable, so the old one stays in use.
        assert!(with_minimum(Some("soon"), None).validate().is_err());
        assert!(
            with_minimum(Some("0.1.17"), Some(&"x".repeat(301)))
                .validate()
                .is_err()
        );
        assert!(
            with_minimum(Some("0.1.17"), Some("Update."))
                .validate()
                .is_ok()
        );
        // The built-in document has no minimum, so it never locks anyone out.
        assert_eq!(update_required("0.0.1"), None);
    }
}
