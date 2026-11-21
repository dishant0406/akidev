//! Plugins: one JSON file a developer shares and a user adds, which brings sources of its own.
//!
//! A plugin holds indexers (a list of torrents), subtitle sources, and catalogues (providers, with the
//! home rows that show them), in the same shape as the signed document (see the README of the akidev-sources repo). Nobody signed it, so it is held to stricter rules: every address
//! must be a public https address, and it can add sources but never replace one that is there.
//!
//! The desktop app keeps plugins in `<data dir>/plugins`. A file `name.json` is switched on and
//! `name.json.off` is switched off. The server never loads them: it uses only the signed document.

use crate::definition::{IndexerDef, MapperDef, ProviderDef, SubtitleDef};
use crate::model::{Kind, Shape};
use crate::screens::{HomeRow, Tab};
use crate::sources::{self, Sources};
use anyhow::{Context, ensure};
use serde::Deserialize;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// The version of the plugin format this build reads.
pub const FORMAT: u64 = 1;

/// A plugin is a few kilobytes; anything much larger is not one.
const MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Plugin {
    pub plugin: u64,
    /// Lower case letters, digits and `-`. It is the name of the file the plugin is kept in.
    pub id: String,
    pub name: String,
    pub version: u64,
    /// Kinds of title the plugin brings, each with a tab. A kind says how its titles behave by its shape.
    #[serde(default)]
    pub kinds: Vec<PluginKind>,
    #[serde(default)]
    pub indexers: Vec<IndexerDef>,
    #[serde(default)]
    pub subtitles: Vec<SubtitleDef>,
    /// Catalogues. Their titles say which provider they came from, so they cannot be mistaken for the
    /// built-in ones.
    #[serde(default)]
    pub providers: Vec<ProviderDef>,
    /// Rows for the home page, each drawn from one of the plugin's own providers.
    #[serde(default)]
    pub home: Vec<HomeRow>,
    /// Id mappers, so the plugin's indexers can be asked by another catalogue's ids, the way the
    /// built-in Kitsu mapper does it. An indexer of the plugin names one in `map`.
    #[serde(default)]
    pub mappers: Vec<MapperDef>,
}

impl PluginKind {
    fn check(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=16).contains(&self.id.len())
                && self.id.bytes().all(|byte| byte.is_ascii_lowercase()),
            "the kind id {:?} must be 1 to 16 lower case letters",
            self.id
        );
        ensure!(
            !Kind::BUILT_IN
                .iter()
                .any(|built_in| built_in.as_str() == self.id),
            "{} is a kind there already is",
            self.id
        );
        ensure!(
            (1..=30).contains(&self.label.chars().count())
                && self
                    .singular
                    .as_ref()
                    .is_none_or(|text| (1..=30).contains(&text.chars().count())),
            "the label of the kind {} must be 1 to 30 characters",
            self.id
        );
        Ok(())
    }
}

/// A kind of title a plugin brings: `documentary`, shaped like a series.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginKind {
    /// 1 to 16 lower case letters. It is the front of the title keys and the address of the tab.
    pub id: String,
    /// The tab and the page heading: "Documentaries".
    pub label: String,
    /// What one title is called: "Documentary". Absent: the label.
    #[serde(default)]
    pub singular: Option<String>,
    /// How its titles behave: `series` (seasons and episodes), `anime` (episodes by number) or `movie`.
    pub shape: Shape,
}

/// Reads and checks a plugin.
pub fn parse(bytes: &[u8]) -> anyhow::Result<Plugin> {
    ensure!(bytes.len() <= MAX_BYTES, "the plugin is too large");
    let document: serde_json::Value =
        serde_json::from_slice(bytes).context("reading the plugin")?;
    // The kinds it brings are made known first, because the rest of the file names them. Any other name is
    // refused while the file is read, so a file that is turned down leaves no kind behind.
    let declared: Vec<PluginKind> = match document.get("kinds") {
        Some(kinds) => serde_json::from_value(kinds.clone()).context("reading its kinds")?,
        None => Vec::new(),
    };
    ensure!(declared.len() <= 4, "too many kinds (4 at most)");
    for kind in &declared {
        kind.check()?;
        Kind::register(
            &kind.id,
            kind.shape,
            kind.singular.as_deref().unwrap_or(&kind.label),
        )?;
    }
    let plugin: Plugin = {
        let _only_known = crate::model::OnlyKnownKinds::begin();
        serde_json::from_value(document).context("reading the plugin")?
    };
    plugin.check()?;
    Ok(plugin)
}

/// Headers a plugin may not set, because they decide where a request goes or how it is framed.
const FORBIDDEN_HEADERS: [&str; 8] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "te",
    "trailer",
    "expect",
];

impl Plugin {
    fn check(&self) -> anyhow::Result<()> {
        ensure!(
            self.plugin == FORMAT,
            "this is plugin format {}, and this app reads format {FORMAT}",
            self.plugin
        );
        ensure!(
            (1..=40).contains(&self.id.len())
                && !self.id.starts_with('-')
                && self
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
            "the id must be 1 to 40 lower case letters, digits or dashes"
        );
        ensure!(
            (1..=60).contains(&self.name.chars().count()),
            "the name must be 1 to 60 characters"
        );
        ensure!(
            !self.indexers.is_empty()
                || !self.mappers.is_empty()
                || !self.subtitles.is_empty()
                || !self.providers.is_empty()
                || !self.kinds.is_empty(),
            "the plugin brings no indexer, no subtitle source and no catalogue"
        );
        // Every kind it uses is one there always is, or one it brings itself.
        let own = |kind: &Kind| {
            Kind::BUILT_IN.contains(kind) || self.kinds.iter().any(|each| each.id == kind.as_str())
        };
        let used = self
            .indexers
            .iter()
            .flat_map(|indexer| indexer.kinds.iter())
            .chain(self.subtitles.iter().flat_map(|source| source.kinds.iter()))
            .chain(
                self.providers
                    .iter()
                    .flat_map(|provider| provider.kinds.iter()),
            )
            .chain(self.home.iter().map(|row| &row.kind));
        for kind in used {
            ensure!(
                own(kind),
                "it uses the kind {kind:?}, which it does not bring"
            );
        }
        ensure!(self.indexers.len() <= 16, "too many indexers (16 at most)");
        ensure!(self.providers.len() <= 4, "too many catalogues (4 at most)");
        ensure!(self.home.len() <= 8, "too many home rows (8 at most)");
        ensure!(self.mappers.len() <= 2, "too many mappers (2 at most)");
        ensure!(
            self.subtitles.len() <= 4,
            "too many subtitle sources (4 at most)"
        );
        for indexer in &self.indexers {
            indexer
                .check()
                .with_context(|| format!("indexer {}", indexer.id))?;
            for name in indexer.request.headers.keys() {
                ensure!(
                    !FORBIDDEN_HEADERS.contains(&name.to_ascii_lowercase().as_str()),
                    "indexer {} sets the header {name}, which a plugin may not",
                    indexer.id
                );
            }
            for host in &indexer.hosts {
                check_public_url(host).with_context(|| format!("indexer {}", indexer.id))?;
            }
        }
        for provider in &self.providers {
            // No dash: a title key is split at the first one after the provider.
            ensure!(
                (1..=24).contains(&provider.id.len())
                    && provider
                        .id
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()),
                "the catalogue id {:?} must be 1 to 24 lower case letters or digits",
                provider.id
            );
            provider
                .check()
                .with_context(|| format!("catalogue {}", provider.id))?;
            check_public_url(&provider.base_url)
                .with_context(|| format!("catalogue {}", provider.id))?;
            for host in &provider.image_hosts {
                check_public_host(host).with_context(|| format!("catalogue {}", provider.id))?;
            }
        }
        for mapper in &self.mappers {
            mapper
                .check()
                .with_context(|| format!("mapper {}", mapper.id))?;
            check_public_url(&mapper.provider.base_url)
                .with_context(|| format!("mapper {}", mapper.id))?;
            for host in &mapper.provider.image_hosts {
                check_public_host(host).with_context(|| format!("mapper {}", mapper.id))?;
            }
        }
        for row in &self.home {
            ensure!(
                row.provider.as_ref().is_some_and(|id| self
                    .providers
                    .iter()
                    .any(|provider| &provider.id == id && provider.kinds.contains(&row.kind))),
                "the row {:?} must name one of the plugin's own catalogues that serves {}",
                row.title,
                row.kind.as_str()
            );
        }
        for source in &self.subtitles {
            source
                .check()
                .with_context(|| format!("subtitle source {}", source.id))?;
            let lookup = source.lookup.iter().map(|lookup| lookup.url.as_str());
            for url in source.hosts.iter().map(String::as_str).chain(lookup) {
                check_public_url(url).with_context(|| format!("subtitle source {}", source.id))?;
            }
            for host in &source.file_hosts {
                check_public_host(host)
                    .with_context(|| format!("subtitle source {}", source.id))?;
            }
        }
        Ok(())
    }
}

/// An address that is https, on a public name, with no login and no port. A plugin is not signed,
/// so it must not be able to point the app at the viewer's own machine or network.
/// (A public name that resolves to a private address is not caught here.)
pub fn check_public_url(url: &str) -> anyhow::Result<()> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| anyhow::anyhow!("\"{url}\" is not an address"))?;
    ensure!(
        parsed.scheme() == "https",
        "\"{url}\" is not an https address"
    );
    ensure!(
        parsed.username().is_empty() && parsed.password().is_none(),
        "\"{url}\" has a login in it"
    );
    ensure!(parsed.port().is_none(), "\"{url}\" names a port");
    check_public_host(parsed.host_str().unwrap_or_default())
}

/// Whether a redirect or an address is to a public name, and not to this machine or a private network.
pub(crate) fn is_public_host(host: &str) -> bool {
    check_public_host(host).is_ok()
}

fn check_public_host(host: &str) -> anyhow::Result<()> {
    let host = host.to_ascii_lowercase();
    let private_ending = [
        ".local",
        ".localhost",
        ".internal",
        ".intranet",
        ".private",
        ".corp",
        ".home",
        ".lan",
        ".localdomain",
        ".home.arpa",
    ];
    ensure!(
        host.contains('.')
            && host.parse::<IpAddr>().is_err()
            && host != "localhost"
            && !private_ending.iter().any(|ending| host.ends_with(ending))
            && host
                .rsplit('.')
                .next()
                .is_some_and(|last| last.bytes().any(|byte| byte.is_ascii_alphabetic())),
        "\"{host}\" is not a public host name"
    );
    Ok(())
}

/// The base document with the plugin's sources added after its own. A source whose id is already
/// taken is refused, so a plugin can add but never replace.
pub fn merge(base: &Sources, plugin: &Plugin) -> anyhow::Result<Sources> {
    let mut merged = base.clone();
    if !plugin.kinds.is_empty() {
        let mut screens = merged.screens_or_built_in();
        for each in &plugin.kinds {
            let singular = each.singular.as_deref().unwrap_or(&each.label);
            let kind = Kind::register(&each.id, each.shape, singular)?;
            screens.tabs.push(Tab {
                kind,
                slug: each.id.clone(),
                label: each.label.clone(),
                tag: singular.to_uppercase(),
            });
        }
        merged.screens = Some(screens);
    }
    for indexer in &plugin.indexers {
        ensure!(
            merged.indexer(&indexer.id).is_none(),
            "an indexer called {} is already there",
            indexer.id
        );
        merged.indexers.push(indexer.clone());
    }
    for mapper in &plugin.mappers {
        ensure!(
            merged.mapper(&mapper.id).is_none(),
            "a mapper called {} is already there",
            mapper.id
        );
        merged.mappers.push(mapper.clone());
    }
    for source in &plugin.subtitles {
        ensure!(
            merged.subtitles.iter().all(|other| other.id != source.id),
            "a subtitle source called {} is already there",
            source.id
        );
        merged.subtitles.push(source.clone());
    }
    for provider in &plugin.providers {
        ensure!(
            merged.provider(&provider.id).is_none(),
            "a catalogue called {} is already there",
            provider.id
        );
        let mut provider = provider.clone();
        provider.qualified = true;
        merged.providers.push(provider);
    }
    if !plugin.home.is_empty() {
        let mut screens = merged.screens_or_built_in();
        screens.home.extend(plugin.home.iter().cloned());
        merged.screens = Some(screens);
    }
    merged.validate()?;
    Ok(merged)
}

/// One plugin as the settings screen lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub enabled: bool,
    pub indexers: usize,
    pub subtitles: usize,
    pub catalogues: usize,
    /// Why it is not in use, when it is switched on and still not working.
    pub problem: Option<String>,
}

fn folder(dir: &Path) -> PathBuf {
    dir.join("plugins")
}

/// Reads the plugins in the folder, puts the switched-on ones to use, and lists them all.
pub fn load(dir: &Path) -> Vec<Installed> {
    let mut files: Vec<(String, bool, PathBuf)> = std::fs::read_dir(folder(dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            if let Some(id) = name.strip_suffix(".json") {
                Some((id.to_string(), true, path))
            } else {
                name.strip_suffix(".json.off")
                    .map(|id| (id.to_string(), false, path.clone()))
            }
        })
        .collect();
    files.sort();

    let mut listed = Vec::new();
    let mut in_use = Vec::new();
    for (file_id, enabled, path) in files {
        match std::fs::read(&path)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| parse(&bytes))
        {
            Ok(plugin) if plugin.id != file_id => listed.push(Installed {
                id: file_id.clone(),
                name: plugin.name.clone(),
                version: plugin.version,
                enabled,
                indexers: plugin.indexers.len(),
                subtitles: plugin.subtitles.len(),
                catalogues: plugin.providers.len(),
                problem: Some(format!("the file must be called {}.json", plugin.id)),
            }),
            Ok(plugin) => {
                listed.push(Installed {
                    id: plugin.id.clone(),
                    name: plugin.name.clone(),
                    version: plugin.version,
                    enabled,
                    indexers: plugin.indexers.len(),
                    subtitles: plugin.subtitles.len(),
                    catalogues: plugin.providers.len(),
                    problem: None,
                });
                if enabled {
                    in_use.push((listed.len() - 1, plugin));
                }
            }
            Err(error) => listed.push(Installed {
                id: file_id.clone(),
                name: file_id,
                version: 0,
                enabled,
                indexers: 0,
                subtitles: 0,
                catalogues: 0,
                problem: Some(format!("{error:#}")),
            }),
        }
    }
    let plugins: Vec<Plugin> = in_use.iter().map(|(_, plugin)| plugin.clone()).collect();
    for ((at, _), result) in in_use.iter().zip(sources::use_plugins(&plugins)) {
        listed[*at].problem = result.err();
    }
    listed
}

/// Adds a plugin, or replaces the one with the same id, and puts it to use. A plugin that cannot
/// be used, or that would stop another one working (a clash over a source's id), is not kept, and
/// what was there before is put back.
pub fn install(dir: &Path, bytes: &[u8]) -> anyhow::Result<Vec<Installed>> {
    let plugin = parse(bytes)?;
    std::fs::create_dir_all(folder(dir))?;
    let on = folder(dir).join(format!("{}.json", plugin.id));
    let off = folder(dir).join(format!("{}.json.off", plugin.id));
    // The old copy, and whether it was switched off. Installing switches it on.
    let was_off = off.exists();
    let before = std::fs::read(if was_off { &off } else { &on }).ok();
    let working_before: Vec<String> = load(dir)
        .into_iter()
        .filter(|each| each.enabled && each.problem.is_none())
        .map(|each| each.id)
        .collect();
    let _ = std::fs::remove_file(&off);
    std::fs::write(&on, bytes)?;
    let listed = load(dir);
    let broken = listed.iter().find(|each| {
        each.enabled
            && each.problem.is_some()
            && (each.id == plugin.id || working_before.contains(&each.id))
    });
    if let Some(broken) = broken {
        let problem = if broken.id == plugin.id {
            broken.problem.clone().unwrap_or_default()
        } else {
            format!("it would stop {} working", broken.name)
        };
        match before {
            Some(old) if was_off => {
                std::fs::remove_file(&on)?;
                std::fs::write(&off, old)?;
            }
            Some(old) => std::fs::write(&on, old)?,
            None => std::fs::remove_file(&on)?,
        }
        load(dir);
        anyhow::bail!(problem);
    }
    Ok(listed)
}

/// Downloads a plugin from an https link and adds it. The link is followed once and never redirected,
/// because a redirect could lead to an address that `check_public_url` would have refused.
pub async fn install_from_link(dir: &Path, url: &str) -> anyhow::Result<Vec<Installed>> {
    check_public_url(url)?;
    download_and_install(dir, url).await
}

/// The download and the install, once the address is known to be allowed. Apart so a test can drive it
/// against a server on this machine, which `check_public_url` rightly refuses.
async fn download_and_install(dir: &Path, url: &str) -> anyhow::Result<Vec<Installed>> {
    let client = reqwest::Client::builder()
        .user_agent(crate::http::USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = crate::http::read_capped(response, MAX_BYTES).await?;
    install(dir, &bytes)
}

/// Switches a plugin on or off.
pub fn set_enabled(dir: &Path, id: &str, enabled: bool) -> anyhow::Result<Vec<Installed>> {
    let on = folder(dir).join(format!("{id}.json"));
    let off = folder(dir).join(format!("{id}.json.off"));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if from.exists() {
        // If both copies are there, the one being switched to wins.
        let _ = std::fs::remove_file(&to);
        std::fs::rename(from, to)?;
    }
    Ok(load(dir))
}

/// Deletes a plugin.
pub fn remove(dir: &Path, id: &str) -> anyhow::Result<Vec<Installed>> {
    for name in [format!("{id}.json"), format!("{id}.json.off")] {
        let _ = std::fs::remove_file(folder(dir).join(name));
    }
    Ok(load(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn indexer(id: &str) -> Value {
        json!({
            "id": id, "name": id, "kinds": ["series"],
            "hosts": ["https://torrents.example.com/api"],
            "request": { "path": "/search", "query": { "q": "{term}" } },
            "response": { "format": "json", "rows": "items",
                          "fields": { "title": "title", "hash": "hash" } }
        })
    }

    fn plugin(id: &str) -> Value {
        json!({ "plugin": 1, "id": id, "name": "Example", "version": 1, "indexers": [indexer("example-torrents")] })
    }

    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("akidev-plugin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_plugin_with_an_indexer_is_read() {
        let plugin = parse(&bytes(&plugin("example"))).unwrap();
        assert_eq!(plugin.indexers.len(), 1);
    }

    #[test]
    fn a_plugin_that_is_not_safe_or_not_whole_is_refused() {
        let refused = |what: &str, change: fn(&mut Value)| {
            let mut value = plugin("example");
            change(&mut value);
            assert!(parse(&bytes(&value)).is_err(), "{what} was accepted");
        };
        refused("a newer format", |p| p["plugin"] = json!(2));
        refused("an id with capitals", |p| p["id"] = json!("Example"));
        refused("an id with a slash", |p| p["id"] = json!("a/b"));
        refused("no name", |p| p["name"] = json!(""));
        refused("nothing in it", |p| p["indexers"] = json!([]));
        refused("a plain http host", |p| {
            p["indexers"][0]["hosts"] = json!(["http://torrents.example.com"]);
        });
        refused("a local host", |p| {
            p["indexers"][0]["hosts"] = json!(["https://localhost/api"]);
        });
        refused("an address on the home network", |p| {
            p["indexers"][0]["hosts"] = json!(["https://192.168.1.5/api"]);
        });
        refused("an address in brackets", |p| {
            p["indexers"][0]["hosts"] = json!(["https://[::1]/api"]);
        });
        refused("a host on the machine's own network", |p| {
            p["indexers"][0]["hosts"] = json!(["https://printer.local/api"]);
        });
        refused("a port", |p| {
            p["indexers"][0]["hosts"] = json!(["https://torrents.example.com:8443/api"]);
        });
        refused("a login in the address", |p| {
            p["indexers"][0]["hosts"] = json!(["https://me:pw@torrents.example.com/api"]);
        });
        refused("a request that cannot be filled in", |p| {
            p["indexers"][0]["request"]["query"] = json!({ "q": "{nothing}" });
        });
        refused("a source that reads no hash", |p| {
            p["indexers"][0]["response"]["fields"] = json!({ "title": "title" });
        });
    }

    #[test]
    fn a_plugin_adds_its_sources_after_the_built_in_ones_and_never_replaces_one() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let base = sources::current();
        let merged = merge(&base, &parse(&bytes(&plugin("example"))).unwrap()).unwrap();
        assert_eq!(merged.indexers.len(), base.indexers.len() + 1);
        assert_eq!(merged.indexers.last().unwrap().id, "example-torrents");

        let mut clash = plugin("example");
        clash["indexers"][0]["id"] = json!(base.indexers[0].id);
        assert!(merge(&base, &parse(&bytes(&clash)).unwrap()).is_err());
    }

    #[test]
    fn a_plugin_can_bring_a_mapper_and_an_indexer_that_asks_through_it() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let base = sources::current();
        // A copy of the built-in Kitsu mapper under another name, and an indexer asked by its ids.
        let document: Value =
            serde_json::from_str(include_str!("../../sources/sources.json")).unwrap();
        let mut mapper = document["mappers"][0].clone();
        assert_eq!(mapper["id"], "kitsu");
        mapper["id"] = json!("mymap");
        let mut example = indexer("anime-by-id");
        example["kinds"] = json!(["anime"]);
        example["map"] = json!("mymap");
        example["request"] = json!({ "path": "/stream/{mymap_kind}/{mymap}.json" });
        let mut brought = plugin("example");
        brought["indexers"] = json!([example]);
        brought["mappers"] = json!([mapper.clone()]);

        let merged = merge(&base, &parse(&bytes(&brought)).unwrap()).unwrap();
        assert_eq!(merged.mapper("mymap").unwrap().id, "mymap");
        assert_eq!(
            merged.indexer("anime-by-id").unwrap().mapper_id(),
            Some("mymap")
        );

        // The mapper alone is enough to make a plugin.
        let mut alone = plugin("example");
        alone.as_object_mut().unwrap().remove("indexers");
        alone["mappers"] = json!([mapper.clone()]);
        assert!(parse(&bytes(&alone)).is_ok());

        // Not one that is already there, and not one on a private address.
        let mut clash = brought.clone();
        clash["mappers"][0]["id"] = json!("kitsu");
        clash["indexers"][0]["map"] = json!("kitsu");
        clash["indexers"][0]["request"] = json!({ "path": "/stream/{kitsu_kind}/{kitsu}.json" });
        assert!(merge(&base, &parse(&bytes(&clash)).unwrap()).is_err());
        let mut private = brought;
        private["mappers"][0]["provider"]["base_url"] = json!("https://127.0.0.1/api");
        assert!(parse(&bytes(&private)).is_err());
    }

    #[test]
    fn installing_switching_off_and_removing_a_plugin_changes_what_is_searched() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("flow");
        let built_in = sources::current().indexers.len();

        let listed = install(&dir, &bytes(&plugin("example"))).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].enabled && listed[0].problem.is_none());
        assert_eq!(sources::current().indexers.len(), built_in + 1);

        let listed = set_enabled(&dir, "example", false).unwrap();
        assert!(!listed[0].enabled);
        assert_eq!(sources::current().indexers.len(), built_in);
        assert!(dir.join("plugins/example.json.off").exists());

        let listed = set_enabled(&dir, "example", true).unwrap();
        assert!(listed[0].enabled);
        assert_eq!(sources::current().indexers.len(), built_in + 1);

        let listed = remove(&dir, "example").unwrap();
        assert!(listed.is_empty());
        assert_eq!(sources::current().indexers.len(), built_in);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_plugin_that_clashes_is_not_kept_and_an_older_copy_survives_a_bad_update() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("clash");
        install(&dir, &bytes(&plugin("example"))).unwrap();

        let mut bad = plugin("example");
        bad["indexers"][0]["id"] = json!(sources::current().indexers[0].id);
        assert!(install(&dir, &bytes(&bad)).is_err());
        let listed = load(&dir);
        assert_eq!(listed.len(), 1);
        assert!(listed[0].problem.is_none());
        assert!(sources::current().indexer("example-torrents").is_some());

        let mut other = plugin("other");
        other["indexers"][0]["id"] = json!("example-torrents");
        assert!(install(&dir, &bytes(&other)).is_err());
        assert!(!dir.join("plugins/other.json").exists());

        remove(&dir, "example").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_listed_with_its_problem_and_does_not_stop_the_others() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("damaged");
        install(&dir, &bytes(&plugin("example"))).unwrap();
        std::fs::write(dir.join("plugins/broken.json"), b"{ not json").unwrap();
        let listed = load(&dir);
        assert_eq!(listed.len(), 2);
        assert!(listed[0].problem.is_some());
        assert!(listed[1].problem.is_none());
        assert!(sources::current().indexer("example-torrents").is_some());
        remove(&dir, "example").unwrap();
        remove(&dir, "broken").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn installing_a_plugin_that_would_stop_another_one_working_is_refused() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("breaks");
        // "zzz" works first. "aaa" is read before it, and brings the same source id.
        let mut first = plugin("zzz");
        first["indexers"][0]["id"] = json!("shared-id");
        install(&dir, &bytes(&first)).unwrap();
        let mut second = plugin("aaa");
        second["indexers"][0]["id"] = json!("shared-id");
        let error = install(&dir, &bytes(&second)).unwrap_err();
        assert!(format!("{error}").contains("stop"), "{error}");
        assert!(!dir.join("plugins/aaa.json").exists());
        assert!(load(&dir).iter().all(|each| each.problem.is_none()));
        assert!(sources::current().indexer("shared-id").is_some());
        remove(&dir, "zzz").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_bad_update_leaves_a_switched_off_plugin_switched_off() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("off");
        install(&dir, &bytes(&plugin("example"))).unwrap();
        set_enabled(&dir, "example", false).unwrap();
        let mut bad = plugin("example");
        bad["indexers"][0]["id"] = json!(sources::current().indexers[0].id);
        assert!(install(&dir, &bytes(&bad)).is_err());
        assert!(dir.join("plugins/example.json.off").exists());
        assert!(!dir.join("plugins/example.json").exists());
        remove(&dir, "example").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_not_named_after_its_plugin_is_listed_with_that_problem() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("named");
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        std::fs::write(dir.join("plugins/my plugin.json"), bytes(&plugin("foo"))).unwrap();
        let listed = load(&dir);
        assert_eq!(listed.len(), 1);
        assert!(listed[0].problem.as_deref().unwrap().contains("foo.json"));
        assert!(sources::current().indexer("example-torrents").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn switching_a_plugin_when_both_copies_exist_keeps_the_one_switched_to() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let dir = scratch("both");
        install(&dir, &bytes(&plugin("example"))).unwrap();
        std::fs::copy(
            dir.join("plugins/example.json"),
            dir.join("plugins/example.json.off"),
        )
        .unwrap();
        set_enabled(&dir, "example", true).unwrap();
        assert!(dir.join("plugins/example.json").exists());
        assert!(!dir.join("plugins/example.json.off").exists());
        remove(&dir, "example").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn private_names_are_not_public() {
        for host in [
            "localhost",
            "127.0.0.1",
            "10.0.0.1",
            "printer.local",
            "wiki.corp",
            "router.home",
            "a.intranet",
        ] {
            assert!(!is_public_host(host), "{host}");
        }
        assert!(is_public_host("torrents.example.com"));
    }

    fn catalogue(id: &str, base: &str) -> Value {
        json!({
            "id": id, "kinds": ["series"], "base_url": base,
            "card": { "id": "id", "title": "name" },
            "list": { "request": { "url": "{base}/list" }, "rows": "items",
                      "paging": { "style": "pool" } },
            "search": { "request": { "url": "{base}/search?q={term}" }, "rows": "items" },
            "detail": { "request": { "url": "{base}/title/{id}" }, "root": "",
                        "episodes": { "rows": "episodes", "id": "id", "season": "season",
                                      "number": "number", "name": "name" } },
            "image_hosts": ["images.example.com"]
        })
    }

    #[test]
    fn a_plugin_catalogue_is_read_and_its_rules_are_kept() {
        let with = |change: fn(&mut Value)| {
            let mut value = json!({ "plugin": 1, "id": "cat", "name": "Cat", "version": 1,
                "providers": [catalogue("mine", "https://catalogue.example.com/api")],
                "home": [{ "title": "Mine", "kind": "series", "sort": "popularity", "provider": "mine" }] });
            change(&mut value);
            parse(&bytes(&value))
        };
        assert!(with(|_| {}).is_ok());
        assert!(with(|p| p["providers"][0]["id"] = json!("my-cat")).is_err());
        assert!(
            with(|p| p["providers"][0]["base_url"] = json!("http://catalogue.example.com"))
                .is_err()
        );
        assert!(with(|p| p["providers"][0]["base_url"] = json!("https://localhost/api")).is_err());
        assert!(with(|p| p["providers"][0]["image_hosts"] = json!(["10.0.0.1"])).is_err());
        assert!(with(|p| p["home"][0]["provider"] = json!("anilist")).is_err());
        assert!(with(|p| p["home"][0]["provider"] = json!(null)).is_err());
        assert!(with(|p| p["home"][0]["kind"] = json!("movie")).is_err());
        assert!(
            with(|p| p["providers"][0]["detail"]["request"]["url"] =
                json!("https://elsewhere.example/{id}"))
            .is_err()
        );
    }

    #[test]
    fn a_plugin_catalogue_cannot_take_the_id_of_a_built_in_one() {
        let _guard = sources::definitions_change_lock().blocking_lock();
        let built_in = sources::current().providers[0].id.clone();
        let plugin = parse(&bytes(
            &json!({ "plugin": 1, "id": "cat", "name": "Cat", "version": 1,
            "providers": [catalogue(&built_in, "https://catalogue.example.com/api")] }),
        ));
        // The built-in ids are not all lower case letters and digits only, so this may fail earlier.
        if let Ok(plugin) = plugin {
            assert!(merge(&sources::current(), &plugin).is_err());
        }
    }

    #[tokio::test]
    async fn a_plugin_catalogue_shows_in_a_row_in_search_and_opens_by_its_own_key() {
        use crate::model::{Filters, Kind};
        use axum::{Router, extract::Path, routing::get};
        let _guard = sources::definitions_change_lock().lock().await;
        let item = json!({ "id": "a-1", "name": "Alpha" });
        let list = item.clone();
        let app = Router::new()
            .route(
                "/list",
                get(move || async move { json!({ "items": [list] }).to_string() }),
            )
            .route(
                "/search",
                get(|| async {
                    json!({ "items": [{ "id": "a-1", "name": "Alpha" }] }).to_string()
                }),
            )
            .route(
                "/title/{id}",
                get(|Path(id): Path<String>| async move {
                    json!({ "id": id, "name": "Alpha",
                            "episodes": [{ "id": "e9", "season": 1, "number": 2, "name": "Two" }] })
                    .to_string()
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // Read from JSON without `parse`, which refuses an address on this machine.
        let plugin: Plugin = serde_json::from_value(json!({
            "plugin": 1, "id": "cat", "name": "Cat", "version": 1,
            "providers": [catalogue("standin", &base)],
            "home": [{ "title": "Community picks", "kind": "series", "sort": "popularity", "provider": "standin" }]
        }))
        .unwrap();
        assert!(sources::use_plugins(&[plugin]).iter().all(Result::is_ok));

        let key = "series-standin~a_2d1";
        let filters = Filters {
            page: 1,
            per_page: 24,
            ..Default::default()
        };
        let page = crate::providers::list_row(Kind::Series, Some("standin"), &filters)
            .await
            .unwrap();
        assert_eq!(page.results[0].id, key);
        let row = sources::screens().home.pop().unwrap();
        assert_eq!(
            (row.title.as_str(), row.provider.as_deref()),
            ("Community picks", Some("standin"))
        );

        let detail = crate::providers::detail(key).await.unwrap();
        assert_eq!(detail.media.id, key);
        assert_eq!(detail.episodes[0].id, format!("{key}-s1e2"));
        assert!(
            crate::providers::detail(&detail.episodes[0].id)
                .await
                .is_some()
        );
        // An id that says it is from a provider that is not there, or from a built-in one, is nobody's.
        assert!(
            crate::providers::detail("series-nobody~a_2d1")
                .await
                .is_none()
        );
        assert!(
            crate::providers::detail("series-anilist~a_2d1")
                .await
                .is_none()
        );

        let found = crate::providers::search(Kind::Series, "alpha")
            .await
            .unwrap();
        assert!(found.results.iter().any(|media| media.id == key));
        sources::use_plugins(&[]);
        assert!(
            sources::screens()
                .home
                .iter()
                .all(|row| row.provider.is_none())
        );
        assert!(crate::providers::detail(key).await.is_none());
    }

    #[test]
    fn a_file_that_names_a_kind_it_does_not_bring_is_refused_and_leaves_no_kind_behind() {
        let mut value = plugin("example");
        value["indexers"][0]["kinds"] = json!(["leftbehindkind"]);
        assert!(parse(&bytes(&value)).is_err());
        assert!(Kind::parse("leftbehindkind").is_none());
        // One it does bring is fine, and known once it has been read.
        let mut brings = plugin("example");
        brings["kinds"] = json!([{ "id": "broughtkind", "label": "Brought", "shape": "movie" }]);
        brings["indexers"][0]["kinds"] = json!(["broughtkind"]);
        assert!(parse(&bytes(&brings)).is_ok());
        assert_eq!(Kind::parse("broughtkind").unwrap().shape(), Shape::Movie);
        // An update that changes the shape is read too.
        brings["kinds"][0]["shape"] = json!("series");
        assert!(parse(&bytes(&brings)).is_ok());
        assert_eq!(Kind::parse("broughtkind").unwrap().shape(), Shape::Series);
    }

    #[test]
    fn a_plugin_cannot_set_the_headers_that_decide_where_a_request_goes() {
        for name in ["Host", "content-length", "Transfer-Encoding", "Connection"] {
            let mut value = plugin("example");
            value["indexers"][0]["request"]["headers"] = json!({ name: "x" });
            assert!(parse(&bytes(&value)).is_err(), "{name}");
        }
        let mut allowed = plugin("example");
        allowed["indexers"][0]["request"]["headers"] = json!({ "x-api-key": "mine" });
        assert!(parse(&bytes(&allowed)).is_ok());
    }

    #[test]
    fn a_plugin_that_brings_a_kind_is_held_to_its_rules() {
        let with = |change: fn(&mut Value)| {
            let mut value = json!({ "plugin": 1, "id": "kinds", "name": "Kinds", "version": 1,
                "kinds": [{ "id": "docs", "label": "Documentaries", "shape": "series" }],
                "providers": [{
                    "id": "docs", "kinds": ["docs"], "base_url": "https://catalogue.example.com/api",
                    "card": { "id": "id", "title": "name" },
                    "list": { "request": { "url": "{base}/list" }, "rows": "items", "paging": { "style": "pool" } },
                    "detail": { "request": { "url": "{base}/title/{id}" }, "root": "" } }] });
            change(&mut value);
            parse(&bytes(&value))
        };
        assert!(with(|_| {}).is_ok());
        assert!(with(|p| p["kinds"][0]["id"] = json!("Docs")).is_err());
        assert!(with(|p| p["kinds"][0]["id"] = json!("movie")).is_err());
        assert!(with(|p| p["kinds"][0]["id"] = json!("a-very-long-kind-name")).is_err());
        assert!(with(|p| p["kinds"][0]["shape"] = json!("shelf")).is_err());
        assert!(with(|p| p["kinds"][0]["label"] = json!("")).is_err());
        // A kind it does not bring is not its to use.
        assert!(with(|p| p["providers"][0]["kinds"] = json!(["music"])).is_err());
        assert!(with(|p| p["kinds"] = json!([])).is_err());
    }

    #[tokio::test]
    async fn a_plugin_kind_gets_a_tab_lists_from_its_own_catalogue_and_keeps_its_shape() {
        use crate::media_id;
        use crate::model::{Filters, Shape};
        use axum::{Router, extract::Path, routing::get};
        let _guard = sources::definitions_change_lock().lock().await;
        let app = Router::new()
            .route("/list", get(|| async { json!({ "items": [{ "id": "d-1", "name": "Planet" }] }).to_string() }))
            .route(
                "/title/{id}",
                get(|Path(id): Path<String>| async move {
                    json!({ "id": id, "name": "Planet",
                            "episodes": [{ "id": "x", "season": 1, "number": 3, "name": "Three" }] })
                    .to_string()
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let mut provider = catalogue("docsprov", &base);
        provider["kinds"] = json!(["docskind"]);
        let plugin: Plugin = serde_json::from_value(json!({
            "plugin": 1, "id": "docs", "name": "Docs", "version": 1,
            "kinds": [{ "id": "docskind", "label": "Documentaries", "singular": "Documentary", "shape": "series" }],
            "providers": [provider]
        }))
        .unwrap();
        assert!(sources::use_plugins(&[plugin]).iter().all(Result::is_ok));

        let kind = Kind::parse("docskind").unwrap();
        assert_eq!(
            (kind.shape(), kind.singular()),
            (Shape::Series, "Documentary")
        );
        let screens = sources::screens();
        assert_eq!(
            screens.tabs.last().map(|tab| tab.label.as_str()),
            Some("Documentaries")
        );
        assert!(screens.kinds().contains(&kind));

        let filters = Filters {
            page: 1,
            per_page: 24,
            ..Default::default()
        };
        let page = crate::providers::list(kind, &filters).await.unwrap();
        let key = "docskind-docsprov~d_2d1";
        assert_eq!(page.results[0].id, key);
        let detail = crate::providers::detail(key).await.unwrap();
        assert_eq!(detail.episodes[0].id, format!("{key}-s1e3"));
        // Its keys follow its shape: a series has seasons, so an anime-style suffix is refused.
        assert!(media_id::parse(&detail.episodes[0].id).is_some());
        assert!(media_id::parse(&format!("{key}-ep3")).is_none());

        // Another plugin cannot bring the same kind with another shape.
        let clash: Plugin = serde_json::from_value(json!({
            "plugin": 1, "id": "clash", "name": "Clash", "version": 1,
            "kinds": [{ "id": "docskind", "label": "Docs again", "shape": "movie" }]
        }))
        .unwrap();
        assert!(merge(&sources::current(), &clash).is_err());

        sources::use_plugins(&[]);
        assert!(sources::screens().tabs.iter().all(|tab| tab.kind != kind));
    }

    #[tokio::test]
    async fn a_plugin_is_downloaded_read_and_installed_and_a_bad_answer_installs_nothing() {
        use axum::{Router, http::StatusCode, response::Redirect, routing::get};
        let _guard = sources::definitions_change_lock().lock().await;
        let good = bytes(&plugin("fromlink"));
        let huge = vec![b' '; MAX_BYTES + 1];
        let app = Router::new()
            .route("/good.json", get(move || async move { good }))
            .route("/missing.json", get(|| async { StatusCode::NOT_FOUND }))
            .route("/huge.json", get(move || async move { huge }))
            .route("/not-json", get(|| async { "<html>sign in</html>" }))
            .route("/go", get(|| async { Redirect::temporary("/good.json") }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = scratch("download");

        for (path, why) in [
            ("missing.json", "an error status"),
            ("huge.json", "more than the size limit"),
            ("not-json", "something that is not a plugin"),
            ("go", "a redirect, which is never followed"),
        ] {
            assert!(
                download_and_install(&dir, &format!("{base}/{path}"))
                    .await
                    .is_err(),
                "{why}"
            );
            assert!(
                !dir.join("plugins").exists() || load(&dir).is_empty(),
                "{why} left a plugin"
            );
        }

        let listed = download_and_install(&dir, &format!("{base}/good.json"))
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (listed[0].id.as_str(), listed[0].problem.as_deref()),
            ("fromlink", None)
        );
        assert!(sources::current().indexer("example-torrents").is_some());
        assert!(dir.join("plugins/fromlink.json").exists());
        remove(&dir, "fromlink").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_link_that_is_not_a_public_https_address_is_not_fetched() {
        let dir = scratch("link");
        for link in [
            "http://example.com/p.json",
            "https://localhost/p.json",
            "https://127.0.0.1/p.json",
            "https://169.254.169.254/latest",
            "not a link",
        ] {
            assert!(install_from_link(&dir, link).await.is_err(), "{link}");
        }
        assert!(!dir.join("plugins").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
