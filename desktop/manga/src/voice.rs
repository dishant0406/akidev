//! Voice models: speech made on the device.
//!
//! A model is a folder of files and a definition that names them (`voices/<id>.json` in a voices repository, see
//! `repo.rs`). The definition is data: which files to download from where, and `family`, the kind of model, which says
//! which runtime of the app runs it. A model of a family the app has needs no new app version; today that is `ptts`
//! (Phonon, from Gradium). Nothing downloaded is ever run as code.
//!
//! On the device a model lives in `<root>/<id>/` with its files and a copy of its definition. It is put there whole or
//! not at all: files are fetched and checked into `<id>.part`, the model speaks one sentence as a test, and only then
//! does the folder take its name.

use crate::http::client_builder;
use crate::repo::{self, IndexVoice};
use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

const MAX_MODEL: u64 = 1024 * 1024 * 1024;
const FAMILIES: [&str; 1] = ["ptts"];
const TEST_SENTENCE: &str = "Hello, this is a test.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceDef {
    pub id: String,
    pub name: String,
    pub version: u64,
    /// The kind of model; one of `FAMILIES`.
    pub family: String,
    /// The language the text is read in (how numbers and abbreviations are spoken), as `en`.
    pub lang: String,
    /// The languages it can speak.
    pub languages: Vec<String>,
    pub licence: Licence,
    pub files: Vec<VoiceFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Licence {
    pub name: String,
    pub url: String,
    /// It may be used in a product that earns money.
    pub commercial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceFile {
    /// The file's name inside the model's folder: no folders.
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}

impl VoiceDef {
    pub fn parse(bytes: &[u8]) -> anyhow::Result<VoiceDef> {
        let def: VoiceDef = serde_json::from_slice(bytes).context("not a valid voice model")?;
        ensure!(
            !def.id.is_empty()
                && def.id.len() <= 64
                && def
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "the id {:?} is not lowercase letters, digits and dashes",
            def.id
        );
        ensure!(
            !def.name.trim().is_empty() && def.name.len() <= 80,
            "the model has no usable name"
        );
        ensure!(
            FAMILIES.contains(&def.family.as_str()),
            "{} is a kind of model this app does not know",
            def.family
        );
        ensure!(!def.languages.is_empty(), "the model speaks no language");
        ensure!(
            !def.files.is_empty() && def.files.len() <= 32,
            "the model lists no files, or too many"
        );
        let mut total: u64 = 0;
        for file in &def.files {
            ensure!(
                !file.path.is_empty()
                    && !file.path.starts_with('.')
                    && file
                        .path
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
                "the file name {:?} is not a plain name",
                file.path
            );
            ensure!(
                file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}: sha256 must be 64 hex digits",
                file.path
            );
            ensure!(
                Url::parse(&file.url).is_ok_and(|url| repo::https_or_local(&url)),
                "{}: the address must be https",
                file.path
            );
            ensure!(
                file.bytes <= MAX_MODEL,
                "{} is larger than allowed",
                file.path
            );
            total += file.bytes;
        }
        ensure!(total <= MAX_MODEL, "the model is larger than allowed");
        Ok(def)
    }
}

/// Whether this app can run a model a repository lists: it knows the kind, and is new enough.
pub fn runs(offered: &IndexVoice, app_version: &str) -> bool {
    FAMILIES.contains(&offered.family.as_str())
        && offered
            .min_app
            .as_deref()
            .is_none_or(|wanted| repo::app_at_least(app_version, wanted))
}

/// A model on the device.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub def: VoiceDef,
    pub folder: PathBuf,
}

/// The models on the device, by name. A folder counts only if it is named after the model it holds: the one a download is
/// still putting together (`<id>.part`) is not there yet.
pub fn installed(root: &Path) -> Vec<Installed> {
    let mut found: Vec<Installed> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let folder = entry.path();
            let bytes = std::fs::read(folder.join("definition.json")).ok()?;
            let def = VoiceDef::parse(&bytes).ok()?;
            (folder.file_name()?.to_str()? == def.id).then_some(Installed { def, folder })
        })
        .collect();
    found.sort_by(|a, b| a.def.name.cmp(&b.def.name));
    found
}

/// Removes models downloads left half-written.
pub fn clear_unfinished(root: &Path) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.path().extension().is_some_and(|ext| ext == "part") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub fn remove(root: &Path, id: &str) -> anyhow::Result<()> {
    ensure!(
        installed(root).iter().any(|model| model.def.id == id),
        "{id} is not on this device"
    );
    std::fs::remove_dir_all(root.join(id))?;
    Ok(())
}

/// A client for big files: no limit on the whole download, only on silence, and it follows redirects (Hugging Face
/// hands files to a CDN) only to https.
fn download_client() -> anyhow::Result<reqwest::Client> {
    Ok(client_builder()
        .user_agent("AkiManga/1.0")
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && attempt.url().scheme() == "https" {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()?)
}

/// Downloads a model a repository offers, checks it and puts it in `root`. `progress(done, total)` is in bytes.
pub async fn install(
    root: &Path,
    base: &str,
    offered: &IndexVoice,
    app_version: &str,
    mut progress: impl FnMut(u64, u64),
) -> anyhow::Result<()> {
    ensure!(
        runs(offered, app_version),
        "{} needs a newer Akidev",
        offered.name
    );
    let text = repo::fetch_file(base, &offered.path, &offered.sha256).await?;
    let def = VoiceDef::parse(text.as_bytes())?;
    ensure!(
        def.id == offered.id,
        "the repository's {} is a different model",
        offered.id
    );
    let staging = root.join(format!("{}.part", def.id));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let result = async {
        let total: u64 = def.files.iter().map(|file| file.bytes).sum();
        let client = download_client()?;
        let mut done = 0;
        for file in &def.files {
            let before = done;
            fetch_file(&client, file, &staging.join(&file.path), |read| {
                progress(before + read, total)
            })
            .await?;
            done += file.bytes;
        }
        std::fs::write(staging.join("definition.json"), &text)?;
        let (folder, lang) = (staging.clone(), def.lang.clone());
        let heard = tokio::task::spawn_blocking(move || {
            Speaker::open(&folder, &lang)?.speak(TEST_SENTENCE)
        })
        .await??;
        ensure!(!heard.is_empty(), "the model made no sound");
        let _ = std::fs::remove_dir_all(root.join(&def.id));
        std::fs::rename(&staging, root.join(&def.id))?;
        anyhow::Ok(())
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

/// Fetches one file to `target`, checking its size and hash as it arrives.
async fn fetch_file(
    client: &reqwest::Client,
    file: &VoiceFile,
    target: &Path,
    mut progress: impl FnMut(u64),
) -> anyhow::Result<()> {
    use std::io::Write;
    let mut response = client
        .get(&file.url)
        .send()
        .await
        .with_context(|| format!("cannot reach {}", file.url))?
        .error_for_status()
        .with_context(|| format!("{} did not answer", file.url))?;
    let mut out = std::fs::File::create(target)?;
    let mut hash = Sha256::new();
    let mut read = 0u64;
    while let Some(chunk) = response.chunk().await? {
        read += chunk.len() as u64;
        ensure!(
            read <= file.bytes,
            "{} is larger than it should be",
            file.path
        );
        hash.update(&chunk);
        out.write_all(&chunk)?;
        progress(read);
    }
    ensure!(
        read == file.bytes,
        "{} ended early ({read} of {} bytes)",
        file.path,
        file.bytes
    );
    let found: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    ensure!(
        found.eq_ignore_ascii_case(&file.sha256),
        "{} is not the file the model describes (its hash differs)",
        file.path
    );
    Ok(())
}

/// A model loaded and ready to speak.
pub struct Speaker {
    synth: ptts::synth::Synth,
}

impl Speaker {
    /// Loads the model in `folder`, whose text is read as `lang` (how numbers are spoken).
    pub fn open(folder: &Path, lang: &str) -> anyhow::Result<Speaker> {
        use ptts::checkpoint::{Checkpoint, ResolveOptions};
        use ptts::synth::{DeviceKind, Quant};
        let Ok(lang) = lang.parse::<ptts::preprocess::Lang>() else {
            bail!("{lang} is not a language this model reads");
        };
        let checkpoint = Checkpoint::resolve(
            folder.to_string_lossy().as_ref(),
            ResolveOptions {
                quant: Quant::Q80,
                weights: None,
            },
        )?;
        let mut synth = checkpoint.builder(lang).device(DeviceKind::Cpu).build()?;
        checkpoint.register_voices(&mut synth);
        Ok(Speaker { synth })
    }

    /// The speech for `text`, mono samples at `sample_rate()`.
    pub fn speak(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(self.synth.say(text)?)
    }

    pub fn sample_rate(&self) -> u32 {
        self.synth.sample_rate()
    }
}
