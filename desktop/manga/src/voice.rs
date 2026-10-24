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
const MAX_VOICE: u64 = 16 * 1024 * 1024;
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
    /// Other voices the model can speak in, each fetched on its own when chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voices: Vec<VoiceChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app: Option<String>,
}

/// A voice a model can speak in: a small file that goes in the model's `voices` folder, named by its id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceChoice {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,
    pub description: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
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
        ensure!(def.voices.len() <= 300, "the model lists too many voices");
        let mut seen = std::collections::HashSet::new();
        for voice in &def.voices {
            ensure!(
                !voice.id.is_empty()
                    && voice.id.len() <= 64
                    && voice
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
                "the voice id {:?} is not a plain name",
                voice.id
            );
            ensure!(seen.insert(&voice.id), "voice {} is listed twice", voice.id);
            ensure!(
                !voice.name.trim().is_empty()
                    && voice.name.len() <= 80
                    && voice.description.len() <= 600,
                "voice {} has no usable name or too long a description",
                voice.id
            );
            ensure!(
                voice.sha256.len() == 64 && voice.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "voice {}: sha256 must be 64 hex digits",
                voice.id
            );
            ensure!(
                Url::parse(&voice.url).is_ok_and(|url| repo::https_or_local(&url)),
                "voice {}: the address must be https",
                voice.id
            );
            ensure!(
                voice.bytes <= MAX_VOICE,
                "voice {} is larger than allowed",
                voice.id
            );
        }
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
        // An update keeps what has not changed: a file, or a voice, that is already here with the right hash is not fetched again.
        let old = root.join(&def.id);
        let mut done = 0;
        for file in &def.files {
            let before = done;
            let target = staging.join(&file.path);
            if !reuse(&old.join(&file.path), &target, &file.sha256) {
                fetch_file(&client, file, &target, |read| {
                    progress(before + read, total)
                })
                .await?;
            }
            done += file.bytes;
            progress(done, total);
        }
        std::fs::create_dir_all(staging.join("voices"))?;
        for voice in &def.voices {
            let name = format!("voices/{}.safetensors", voice.id);
            reuse(&old.join(&name), &staging.join(&name), &voice.sha256);
        }
        std::fs::write(staging.join("definition.json"), &text)?;
        let (folder, lang) = (staging.clone(), def.lang.clone());
        let heard = tokio::task::spawn_blocking(move || {
            Speaker::open(&folder, &lang)?.speak(TEST_SENTENCE, None)
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

/// Copies `from` to `to` when it is there and is the file with this hash.
fn reuse(from: &Path, to: &Path, sha256: &str) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(from) else {
        return false;
    };
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    while let Ok(read) = file.read(&mut buffer) {
        if read == 0 {
            let found: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
            return found.eq_ignore_ascii_case(sha256) && std::fs::copy(from, to).is_ok();
        }
        hash.update(&buffer[..read]);
    }
    false
}

/// Whether a model's voice has been fetched.
pub fn has_choice(folder: &Path, voice: &str) -> bool {
    folder
        .join("voices")
        .join(format!("{voice}.safetensors"))
        .is_file()
}

/// Fetches one of an installed model's other voices, checks it and puts it in the model's `voices` folder.
pub async fn get_choice(root: &Path, model: &str, voice: &str) -> anyhow::Result<()> {
    let model = installed(root)
        .into_iter()
        .find(|found| found.def.id == model)
        .with_context(|| format!("{model} is not on this device"))?;
    let choice = model
        .def
        .voices
        .iter()
        .find(|found| found.id == voice)
        .with_context(|| format!("{} has no voice {voice}", model.def.name))?;
    let folder = model.folder.join("voices");
    std::fs::create_dir_all(&folder)?;
    let part = folder.join(format!("{voice}.part"));
    let file = VoiceFile {
        path: voice.to_string(),
        url: choice.url.clone(),
        sha256: choice.sha256.clone(),
        bytes: choice.bytes,
    };
    let result = fetch_file(&download_client()?, &file, &part, |_| {}).await;
    match result {
        Ok(()) => std::fs::rename(&part, folder.join(format!("{voice}.safetensors")))?,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            return Err(error);
        }
    }
    Ok(())
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

    /// The speech for `text` in one of the model's voices (its own when none is named), mono samples at `sample_rate()`.
    pub fn speak(&self, text: &str, voice: Option<&str>) -> anyhow::Result<Vec<f32>> {
        use ptts::synth::SpeechOptions;
        let options = match voice {
            Some(voice) => SpeechOptions::default().voice(voice),
            None => SpeechOptions::default(),
        };
        Ok(self.synth.say_with(text, &options)?)
    }

    pub fn sample_rate(&self) -> u32 {
        self.synth.sample_rate()
    }
}
