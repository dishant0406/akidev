//! Repositories: a folder of plugins somewhere on the web (a GitHub repository, usually) with an
//! `index.json` that says what is in it.
//!
//! ```text
//! index.json          the index (this file)
//! index.json.sig      optional: a signature over index.json, hex, by the key the index names
//! themes/madara.json  a theme (a base definition other sources extend)
//! sources/xyz.json    a source
//! ```
//!
//! The index names every file with its version and the SHA-256 of its content, so a file that is not
//! what the index says is refused. Trust in the index itself comes from the person adding the
//! repository; a signed repository additionally cannot change hands without the app noticing (the app
//! remembers the key the first time).

use crate::http::{client, read_capped};
use akidev_core::sources::{decode_hex, encode_hex};
use anyhow::{Context, bail, ensure};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

pub const SCHEMA: u32 = 1;
const MAX_INDEX: usize = 1024 * 1024;
const MAX_FILE: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Index {
    pub schema: u32,
    pub name: String,
    /// The repository's public key (hex), when `index.json.sig` signs the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default)]
    pub sources: Vec<IndexSource>,
    #[serde(default)]
    pub themes: Vec<IndexTheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexSource {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub lang: Vec<String>,
    #[serde(default)]
    pub nsfw: bool,
    /// The oldest app version that can run this source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app: Option<String>,
    /// Themes this source extends: installed with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexTheme {
    pub id: String,
    pub version: u64,
    pub path: String,
    pub sha256: String,
}

fn plain(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A path inside the repository: relative, no `..`, no address.
fn inside(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.contains("://") && !path.contains('\\') && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn is_hex_hash(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Index {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.schema == SCHEMA, "this repository is schema {}, and this app reads schema {SCHEMA}", self.schema);
        ensure!(!self.name.trim().is_empty() && self.name.len() <= 80, "the repository has no usable name");
        ensure!(self.sources.len() <= 500 && self.themes.len() <= 100, "the repository lists too much");
        if let Some(key) = &self.key {
            ensure!(decode_hex(key).is_ok_and(|bytes| bytes.len() == 32), "the repository's key is not a 32-byte hex key");
        }
        let mut seen = std::collections::HashSet::new();
        for source in &self.sources {
            ensure!(plain(&source.id), "source id {:?} is not lowercase letters, digits and dashes", source.id);
            ensure!(seen.insert(source.id.as_str()), "source {} is listed twice", source.id);
            ensure!(inside(&source.path), "source {}: the path must stay inside the repository", source.id);
            ensure!(is_hex_hash(&source.sha256), "source {}: sha256 must be 64 hex digits", source.id);
            for theme in &source.requires {
                ensure!(self.themes.iter().any(|have| &have.id == theme), "source {} needs theme {theme}, which the repository does not list", source.id);
            }
        }
        let mut seen = std::collections::HashSet::new();
        for theme in &self.themes {
            ensure!(plain(&theme.id), "theme id {:?} is not lowercase letters, digits and dashes", theme.id);
            ensure!(seen.insert(theme.id.as_str()), "theme {} is listed twice", theme.id);
            ensure!(inside(&theme.path), "theme {}: the path must stay inside the repository", theme.id);
            ensure!(is_hex_hash(&theme.sha256), "theme {}: sha256 must be 64 hex digits", theme.id);
        }
        Ok(())
    }
}

/// Where a repository's files are, as a folder address ending in `/`, from what a person pastes:
/// `https://github.com/owner/repo` (or `.../tree/branch[/folder]`), a `raw.githubusercontent.com`
/// address, or any other `https` folder. `http` is accepted only for this machine, for development.
pub fn base_of(link: &str) -> anyhow::Result<String> {
    let link = link.trim();
    let with_scheme = if link.contains("://") { link.to_string() } else { format!("https://{link}") };
    let url = Url::parse(&with_scheme).with_context(|| format!("{link} is not a web address"))?;
    let host = url.host_str().context("the address has no host")?.to_lowercase();
    let loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]" | "::1");
    ensure!(url.scheme() == "https" || (url.scheme() == "http" && loopback), "a repository must be served over https");
    ensure!(url.username().is_empty() && url.password().is_none(), "the address must not carry a login");
    let segments: Vec<&str> = url.path_segments().map(|parts| parts.filter(|part| !part.is_empty()).collect()).unwrap_or_default();
    let mut base = match host.as_str() {
        "github.com" | "www.github.com" => {
            ensure!(segments.len() >= 2, "a GitHub address looks like github.com/owner/repository");
            let (owner, repo) = (segments[0], segments[1].trim_end_matches(".git"));
            let (branch, folder) = match segments.get(2) {
                Some(&"tree") | Some(&"blob") => (segments.get(3).copied().unwrap_or("HEAD"), segments.get(4..).unwrap_or_default().to_vec()),
                Some(_) => bail!("a GitHub address looks like github.com/owner/repository (optionally /tree/branch)"),
                None => ("HEAD", Vec::new()),
            };
            let mut base = format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/");
            for part in folder {
                base.push_str(part);
                base.push('/');
            }
            base
        }
        _ => {
            let mut parts = segments;
            if parts.last().is_some_and(|last| *last == "index.json") {
                parts.pop();
            }
            let mut base = format!("{}://{}", url.scheme(), url.host_str().unwrap());
            if let Some(port) = url.port() {
                base.push_str(&format!(":{port}"));
            }
            base.push('/');
            for part in parts {
                base.push_str(part);
                base.push('/');
            }
            base
        }
    };
    if !base.ends_with('/') {
        base.push('/');
    }
    Ok(base)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    encode_hex(&Sha256::digest(bytes))
}

pub fn verify(bytes: &[u8], signature_hex: &str, key_hex: &str) -> anyhow::Result<()> {
    let key: [u8; 32] = decode_hex(key_hex)?.try_into().map_err(|_| anyhow::anyhow!("a key is 32 bytes"))?;
    let signature: [u8; 64] = decode_hex(signature_hex.trim())?.try_into().map_err(|_| anyhow::anyhow!("a signature is 64 bytes"))?;
    VerifyingKey::from_bytes(&key)?.verify(bytes, &Signature::from_bytes(&signature)).map_err(|_| anyhow::anyhow!("the signature does not match the index"))
}

/// A new key pair as (secret, public), both hex.
pub fn generate_key() -> anyhow::Result<(String, String)> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|error| anyhow::anyhow!("no randomness: {error}"))?;
    let key = SigningKey::from_bytes(&secret);
    Ok((encode_hex(&secret), encode_hex(key.verifying_key().as_bytes())))
}

pub fn sign(bytes: &[u8], secret_hex: &str) -> anyhow::Result<String> {
    let secret: [u8; 32] = decode_hex(secret_hex.trim())?.try_into().map_err(|_| anyhow::anyhow!("a secret key is 32 bytes"))?;
    Ok(encode_hex(&SigningKey::from_bytes(&secret).sign(bytes).to_bytes()))
}

/// The public half of a secret key (hex).
pub fn public_of(secret_hex: &str) -> anyhow::Result<String> {
    let secret: [u8; 32] = decode_hex(secret_hex.trim())?.try_into().map_err(|_| anyhow::anyhow!("a secret key is 32 bytes"))?;
    Ok(encode_hex(SigningKey::from_bytes(&secret).verifying_key().as_bytes()))
}

/// `a.b.c` against `x.y.z`: is the app at least the version a source asks for?
pub fn app_at_least(app: &str, wanted: &str) -> bool {
    let parts = |text: &str| -> Vec<u64> { text.split('.').map(|part| part.parse().unwrap_or(0)).collect() };
    let (app, wanted) = (parts(app), parts(wanted));
    (0..3).map(|i| (app.get(i).copied().unwrap_or(0), wanted.get(i).copied().unwrap_or(0))).find(|(a, w)| a != w).is_none_or(|(a, w)| a > w)
}

/// Makes the index for a repository folder: `repo.json` (`{"name": ...}`, and `key` when it will be signed),
/// every `themes/*.json` and every `sources/*.json`, each with its hash. What a repository's author runs
/// before publishing.
pub fn build_index(dir: &std::path::Path, key: Option<String>) -> anyhow::Result<Index> {
    let read = |path: &std::path::Path| -> anyhow::Result<(Vec<u8>, serde_json::Value)> {
        let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        let value = serde_json::from_slice(&bytes).with_context(|| format!("{} is not JSON", path.display()))?;
        Ok((bytes, value))
    };
    let (_, meta) = read(&dir.join("repo.json")).context("a repository needs a repo.json with its name")?;
    let name = meta["name"].as_str().context("repo.json needs a \"name\"")?.to_string();
    let files = |folder: &str| -> anyhow::Result<Vec<std::path::PathBuf>> {
        let mut found: Vec<_> = match std::fs::read_dir(dir.join(folder)) {
            Ok(entries) => entries.filter_map(|entry| Some(entry.ok()?.path())).filter(|path| path.extension().is_some_and(|ext| ext == "json")).collect(),
            Err(_) => Vec::new(),
        };
        found.sort();
        Ok(found)
    };
    let mut themes = Vec::new();
    for path in files("themes")? {
        let (bytes, value) = read(&path)?;
        let id = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().to_string();
        themes.push(IndexTheme { id: id.clone(), version: value["version"].as_u64().with_context(|| format!("theme {id} needs a \"version\""))?, path: format!("themes/{id}.json"), sha256: sha256_hex(&bytes) });
    }
    let mut sources = Vec::new();
    for path in files("sources")? {
        let (bytes, value) = read(&path)?;
        let id = value["id"].as_str().with_context(|| format!("{} has no id", path.display()))?.to_string();
        ensure!(path.file_stem().and_then(|stem| stem.to_str()) == Some(id.as_str()), "{} must be named {id}.json after its id", path.display());
        sources.push(IndexSource {
            name: value["name"].as_str().with_context(|| format!("source {id} has no name"))?.to_string(),
            version: value["version"].as_u64().with_context(|| format!("source {id} has no version"))?,
            path: format!("sources/{id}.json"),
            sha256: sha256_hex(&bytes),
            lang: value["lang"].as_array().map(|list| list.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            nsfw: value["nsfw"].as_bool().unwrap_or(false),
            min_app: value["min_app"].as_str().map(str::to_string),
            requires: value["extends"].as_str().map(|theme| vec![theme.to_string()]).unwrap_or_default(),
            id,
        });
    }
    let index = Index { schema: SCHEMA, name, key, sources, themes };
    index.validate()?;
    Ok(index)
}

pub struct Fetched {
    pub index: Index,
    /// The raw bytes, kept so a signature can be checked against exactly what was read.
    pub signed: bool,
}

async fn get(url: &str, max: usize) -> anyhow::Result<Option<Vec<u8>>> {
    let response = client().get(url).send().await.with_context(|| format!("could not reach {url}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    ensure!(response.status().is_success(), "{url} answered {}", response.status());
    Ok(Some(read_capped(response, max).await?))
}

/// Reads and checks a repository's index.
pub async fn fetch_index(base: &str) -> anyhow::Result<Fetched> {
    let url = format!("{base}index.json");
    let bytes = get(&url, MAX_INDEX).await?.with_context(|| format!("there is no index.json at {base}"))?;
    let index: Index = serde_json::from_slice(&bytes).context("index.json is not a valid repository index")?;
    index.validate()?;
    let signed = match &index.key {
        Some(key) => {
            let signature = get(&format!("{base}index.json.sig"), 4096).await?.context("the index names a key but there is no index.json.sig")?;
            verify(&bytes, &String::from_utf8_lossy(&signature), key)?;
            true
        }
        None => false,
    };
    Ok(Fetched { index, signed })
}

/// Reads one of a repository's files and checks it is the one the index named.
pub async fn fetch_file(base: &str, path: &str, sha256: &str) -> anyhow::Result<String> {
    ensure!(inside(path), "{path} is not inside the repository");
    let bytes = get(&format!("{base}{path}"), MAX_FILE).await?.with_context(|| format!("{path} is not in the repository"))?;
    ensure!(sha256_hex(&bytes).eq_ignore_ascii_case(sha256), "{path} is not the file the index describes (its hash differs)");
    String::from_utf8(bytes).with_context(|| format!("{path} is not text"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_addresses_become_raw_folders() {
        assert_eq!(base_of("https://github.com/owner/repo").unwrap(), "https://raw.githubusercontent.com/owner/repo/HEAD/");
        assert_eq!(base_of("github.com/owner/repo.git").unwrap(), "https://raw.githubusercontent.com/owner/repo/HEAD/");
        assert_eq!(base_of("https://github.com/owner/repo/tree/main/plugins/manga").unwrap(), "https://raw.githubusercontent.com/owner/repo/main/plugins/manga/");
        assert_eq!(base_of("https://raw.githubusercontent.com/o/r/dev/index.json").unwrap(), "https://raw.githubusercontent.com/o/r/dev/");
        assert_eq!(base_of("https://example.org/a/b").unwrap(), "https://example.org/a/b/");
    }

    #[test]
    fn plain_http_and_odd_addresses_are_refused() {
        assert!(base_of("http://example.org/repo").is_err());
        assert!(base_of("http://127.0.0.1:8770/repo/").is_ok());
        assert!(base_of("https://user:pass@example.org/").is_err());
        assert!(base_of("https://github.com/owner").is_err());
        assert!(base_of("https://github.com/owner/repo/issues/1").is_err());
        assert!(base_of("not a link").is_err());
    }

    fn index() -> Index {
        Index {
            schema: 1,
            name: "Test".into(),
            key: None,
            sources: vec![IndexSource { id: "a".into(), name: "A".into(), version: 1, path: "sources/a.json".into(), sha256: "0".repeat(64), lang: vec![], nsfw: false, min_app: None, requires: vec!["t".into()] }],
            themes: vec![IndexTheme { id: "t".into(), version: 1, path: "themes/t.json".into(), sha256: "1".repeat(64) }],
        }
    }

    #[test]
    fn an_index_cannot_point_outside_itself_or_at_a_theme_it_lacks() {
        assert!(index().validate().is_ok());
        for bad in ["../x.json", "/etc/passwd", "https://evil.test/a.json", "a//b.json", "a/./b.json"] {
            let mut i = index();
            i.sources[0].path = bad.into();
            assert!(i.validate().is_err(), "{bad}");
        }
        let mut i = index();
        i.sources[0].requires = vec!["missing".into()];
        assert!(i.validate().is_err());
        let mut i = index();
        i.sources[0].sha256 = "xyz".into();
        assert!(i.validate().is_err());
        let mut i = index();
        i.schema = 2;
        assert!(i.validate().is_err());
    }

    #[test]
    fn signatures_verify_only_for_the_signed_bytes_and_key() {
        let (secret, public) = generate_key().unwrap();
        assert_eq!(public_of(&secret).unwrap(), public);
        let signature = sign(b"index", &secret).unwrap();
        assert!(verify(b"index", &signature, &public).is_ok());
        assert!(verify(b"index!", &signature, &public).is_err());
        let (_, other) = generate_key().unwrap();
        assert!(verify(b"index", &signature, &other).is_err());
    }

    #[test]
    fn app_versions_compare_by_number_not_by_text() {
        assert!(app_at_least("0.10.0", "0.9.5"));
        assert!(app_at_least("1.0.0", "1.0.0"));
        assert!(!app_at_least("0.1.25", "0.2.0"));
        assert!(app_at_least("2", "1.9.9"));
    }
}
