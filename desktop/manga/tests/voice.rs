//! Voice models: what a definition may say, and installing one over HTTP (what is refused, and that a model that
//! fails leaves nothing behind). The one test that needs a real model is ignored: see its comment.

use akidev_manga::repo::{self, IndexVoice, sha256_hex};
use akidev_manga::voice::{self, VoiceDef};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A server's address, before it has anything to serve: a definition names the files by it.
async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    (listener, base)
}

/// Serves given bytes at given paths.
fn serve(listener: TcpListener, files: HashMap<String, Vec<u8>>) {
    let files = Arc::new(files);
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let files = files.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 4096];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).to_string();
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let (status, body) = match files.get(path) {
                    Some(body) => ("200 OK", body.clone()),
                    None => ("404 Not Found", b"no".to_vec()),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
}

fn definition(base: &str, files: &[(&str, &[u8])]) -> Value {
    json!({
        "id": "test-model", "name": "Test model", "version": 1, "family": "ptts", "lang": "en",
        "languages": ["en"],
        "licence": { "name": "Apache 2.0", "url": "https://example.org", "commercial": true },
        "files": files.iter().map(|(name, bytes)| json!({
            "path": name, "url": format!("{base}files/{name}"),
            "sha256": sha256_hex(bytes), "bytes": bytes.len(),
        })).collect::<Vec<_>>(),
    })
}

/// The definition with other voices listed, each served at `files/<id>.safetensors`.
fn with_voices(mut def: Value, base: &str, voices: &[(&str, &[u8])]) -> Value {
    def["voices"] = voices
        .iter()
        .map(|(id, bytes)| {
            json!({
                "id": id, "name": format!("Voice {id}"), "description": "A voice.",
                "url": format!("{base}files/{id}.safetensors"),
                "sha256": sha256_hex(bytes), "bytes": bytes.len(),
            })
        })
        .collect();
    def
}

fn offered(def: &[u8]) -> IndexVoice {
    let parsed = VoiceDef::parse(def).unwrap();
    IndexVoice {
        id: parsed.id,
        name: parsed.name,
        version: 1,
        path: "voices/test-model.json".into(),
        sha256: sha256_hex(def),
        family: "ptts".into(),
        languages: vec!["en".into()],
        bytes: 0,
        licence: "Apache 2.0".into(),
        commercial: true,
        min_app: None,
    }
}

/// A server with the definition, and the files it holds (which may differ from what the definition says).
async fn repository(def_files: &[(&str, &[u8])], held: &[(&str, &[u8])]) -> (String, IndexVoice) {
    let (listener, base) = bind().await;
    let def = definition(&base, def_files).to_string().into_bytes();
    let mut files = HashMap::new();
    files.insert("voices/test-model.json".to_string(), def.clone());
    for (name, bytes) in held {
        files.insert(format!("files/{name}"), bytes.to_vec());
    }
    serve(listener, files);
    (base, offered(&def))
}

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("aki-voice-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn a_definition_must_stay_plain_and_known() {
    let good = definition("https://example.org/", &[("config.json", b"{}")]);
    assert!(VoiceDef::parse(good.to_string().as_bytes()).is_ok());
    let broken = |change: &dyn Fn(&mut Value)| {
        let mut def = good.clone();
        change(&mut def);
        VoiceDef::parse(def.to_string().as_bytes()).is_err()
    };
    assert!(broken(&|d| d["family"] = "unknown".into()));
    assert!(broken(&|d| d["id"] = "Has Space".into()));
    assert!(broken(&|d| d["files"][0]["path"] = "../x".into()));
    assert!(broken(&|d| d["files"][0]["path"] = "a/b".into()));
    assert!(broken(
        &|d| d["files"][0]["url"] = "http://example.org/x".into()
    ));
    assert!(broken(
        &|d| d["files"][0]["url"] = "file:///etc/passwd".into()
    ));
    assert!(broken(
        &|d| d["files"][0]["url"] = "http://localhost.evil.com/x".into()
    ));
    assert!(broken(
        &|d| d["files"][0]["url"] = "http://127.0.0.1.evil.com/x".into()
    ));
    assert!(!broken(
        &|d| d["files"][0]["url"] = "http://127.0.0.1:8000/x".into()
    ));
    assert!(broken(&|d| d["files"][0]["bytes"] = u64::MAX.into()));
    assert!(broken(&|d| d["files"] = json!([
        { "path": "a", "url": "https://x.org/a", "sha256": "0".repeat(64), "bytes": u64::MAX / 2 },
        { "path": "b", "url": "https://x.org/b", "sha256": "0".repeat(64), "bytes": u64::MAX / 2 },
    ])));
    assert!(broken(&|d| d["files"][0]["sha256"] = "abc".into()));
    assert!(broken(&|d| d["files"] = json!([])));
    assert!(broken(&|d| d["languages"] = json!([])));
}

#[test]
fn a_models_other_voices_must_stay_plain() {
    let good = with_voices(
        definition("https://example.org/", &[("config.json", b"{}")]),
        "https://example.org/",
        &[("Ab_c-1", b"v")],
    );
    assert!(VoiceDef::parse(good.to_string().as_bytes()).is_ok());
    let broken = |change: &dyn Fn(&mut Value)| {
        let mut def = good.clone();
        change(&mut def);
        VoiceDef::parse(def.to_string().as_bytes()).is_err()
    };
    assert!(broken(&|d| d["voices"][0]["id"] = "a/b".into()));
    assert!(broken(&|d| d["voices"][0]["id"] = "..".into()));
    assert!(broken(
        &|d| d["voices"][0]["url"] = "http://example.org/v".into()
    ));
    assert!(broken(&|d| d["voices"][0]["sha256"] = "abc".into()));
    assert!(broken(&|d| d["voices"][0]["bytes"] = u64::MAX.into()));
    assert!(broken(
        &|d| d["voices"] = json!([d["voices"][0].clone(), d["voices"][0].clone()])
    ));
}

#[tokio::test]
async fn another_voice_is_fetched_checked_and_kept_and_a_bad_one_leaves_nothing() {
    let root = root("choice");
    let (listener, base) = bind().await;
    let good: &[u8] = b"voice bytes";
    let held = HashMap::from([
        ("files/good.safetensors".to_string(), good.to_vec()),
        (
            "files/bad.safetensors".to_string(),
            b"not what was promised".to_vec(),
        ),
    ]);
    serve(listener, held);
    let mut def = with_voices(
        definition(&base, &[("config.json", b"{}")]),
        &base,
        &[("good", good), ("bad", b"promised")],
    );
    def["voices"][1]["bytes"] = 21.into();
    let folder = root.join("test-model");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("definition.json"), def.to_string()).unwrap();

    assert!(!voice::has_choice(&folder, "good"));
    voice::get_choice(&root, "test-model", "good")
        .await
        .unwrap();
    assert!(voice::has_choice(&folder, "good"));
    assert_eq!(
        std::fs::read(folder.join("voices/good.safetensors")).unwrap(),
        good
    );

    let error = voice::get_choice(&root, "test-model", "bad")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hash differs"), "{error}");
    assert!(!voice::has_choice(&folder, "bad"));
    assert_eq!(std::fs::read_dir(folder.join("voices")).unwrap().count(), 1);
    assert!(
        voice::get_choice(&root, "test-model", "missing")
            .await
            .is_err()
    );
    assert!(
        voice::get_choice(&root, "other-model", "good")
            .await
            .is_err()
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_file_that_is_not_the_one_named_is_refused_and_leaves_nothing() {
    let root = root("hash");
    let (base, voice) = repository(
        &[("config.json", b"{\"x\":2}")],
        &[("config.json", b"{\"x\":1}")],
    )
    .await;
    let error = voice::install(&root, &base, &voice, "1.0.0", |_, _| {})
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hash differs"), "{error}");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn files_that_are_not_a_model_fail_the_test_and_leave_nothing() {
    let root = root("notamodel");
    let files: &[(&str, &[u8])] = &[("config.json", b"{\"x\":1}")];
    let (base, voice) = repository(files, files).await;
    let mut last = (0, 0);
    let error = voice::install(&root, &base, &voice, "1.0.0", |done, total| {
        last = (done, total)
    })
    .await
    .unwrap_err();
    assert_eq!(last, (7, 7), "all was fetched before the check failed");
    assert!(!error.to_string().is_empty());
    assert!(voice::installed(&root).is_empty());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_model_for_a_newer_app_and_a_missing_file_are_refused() {
    let root = root("refused");
    let files: &[(&str, &[u8])] = &[("config.json", b"{}")];
    let (base, mut voice) = repository(files, files).await;
    voice.min_app = Some("9.0.0".into());
    let error = voice::install(&root, &base, &voice, "1.0.0", |_, _| {})
        .await
        .unwrap_err();
    assert!(error.to_string().contains("newer"), "{error}");
    let (base, voice) = repository(files, &[]).await;
    assert!(
        voice::install(&root, &base, &voice, "1.0.0", |_, _| {})
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn installed_models_are_the_folders_that_hold_a_definition_and_unfinished_ones_are_cleared() {
    let root = root("list");
    let def = definition("https://example.org/", &[("config.json", b"{}")]).to_string();
    std::fs::create_dir_all(root.join("test-model")).unwrap();
    std::fs::write(root.join("test-model/definition.json"), &def).unwrap();
    std::fs::create_dir_all(root.join("half.part")).unwrap();
    // A download is still putting this one together: its definition is there, but it is not installed.
    std::fs::write(root.join("half.part/definition.json"), &def).unwrap();
    std::fs::create_dir_all(root.join("stray")).unwrap();
    let found = voice::installed(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].def.id, "test-model");
    voice::clear_unfinished(&root);
    assert!(!root.join("half.part").exists() && root.join("stray").exists());
    assert!(voice::remove(&root, "nothing").is_err());
    voice::remove(&root, "test-model").unwrap();
    assert!(voice::installed(&root).is_empty());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_index_lists_voices_with_their_hashes() {
    let dir = root("index");
    std::fs::write(dir.join("repo.json"), r#"{"name":"Voices"}"#).unwrap();
    std::fs::create_dir_all(dir.join("voices")).unwrap();
    let def = definition(
        "https://example.org/",
        &[("config.json", b"{}"), ("m.gguf", b"1234")],
    )
    .to_string();
    std::fs::write(dir.join("voices/test-model.json"), &def).unwrap();
    let index = repo::build_index(&dir, None).unwrap();
    assert_eq!(index.voices.len(), 1);
    assert_eq!(index.voices[0].bytes, 6);
    assert_eq!(index.voices[0].sha256, sha256_hex(def.as_bytes()));
    std::fs::rename(
        dir.join("voices/test-model.json"),
        dir.join("voices/other.json"),
    )
    .unwrap();
    assert!(
        repo::build_index(&dir, None).is_err(),
        "a file is named after its id"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Installs and speaks with a real model. Needs a folder with Phonon 31M's four files (config.json, tokenizer.json,
/// default-voice.safetensors, model.q8.gguf, from huggingface.co/gradium/phonon-31m-en) in `AKI_VOICE_MODEL`:
/// `AKI_VOICE_MODEL=/path cargo test -p akidev-manga --release --test voice -- --ignored`.
#[tokio::test]
#[ignore]
async fn a_real_model_installs_speaks_and_is_removed() {
    let source = PathBuf::from(std::env::var("AKI_VOICE_MODEL").expect("AKI_VOICE_MODEL"));
    let names = [
        "config.json",
        "tokenizer.json",
        "default-voice.safetensors",
        "model.q8.gguf",
    ];
    let bytes: Vec<Vec<u8>> = names
        .iter()
        .map(|name| std::fs::read(source.join(name)).unwrap())
        .collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .zip(&bytes)
        .map(|(name, bytes)| (*name, bytes.as_slice()))
        .collect();
    // A voice to choose, when the folder has a `voices` folder with one in it.
    let other = std::fs::read_dir(source.join("voices"))
        .ok()
        .and_then(|mut entries| entries.find_map(|entry| Some(entry.ok()?.path())));
    let other_bytes = other.as_ref().map(|path| std::fs::read(path).unwrap());
    let other_id = other
        .as_ref()
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned());
    let (listener, base) = bind().await;
    let mut def = definition(&base, &files);
    if let (Some(id), Some(bytes)) = (&other_id, &other_bytes) {
        def = with_voices(def, &base, &[(id.as_str(), bytes.as_slice())]);
    }
    let def = def.to_string().into_bytes();
    let mut held = HashMap::from([("voices/test-model.json".to_string(), def.clone())]);
    for (name, bytes) in &files {
        held.insert(format!("files/{name}"), bytes.to_vec());
    }
    if let (Some(id), Some(bytes)) = (&other_id, &other_bytes) {
        held.insert(format!("files/{id}.safetensors"), bytes.clone());
    }
    serve(listener, held);
    let voice = offered(&def);
    let root = root("real");
    let mut last = (0, 0);
    voice::install(&root, &base, &voice, "1.0.0", |done, total| {
        last = (done, total)
    })
    .await
    .unwrap();
    assert_eq!(last.0, last.1);
    let model = voice::installed(&root).remove(0);
    let speaker = voice::Speaker::open(&model.folder, &model.def.lang).unwrap();
    let pcm = speaker
        .speak("It is a truth universally acknowledged.", None)
        .unwrap();
    let seconds = pcm.len() as f32 / speaker.sample_rate() as f32;
    assert!((1.0..8.0).contains(&seconds), "{seconds} seconds");
    assert!(pcm.iter().any(|sample| sample.abs() > 0.01));
    if let Some(id) = &other_id {
        voice::get_choice(&root, &model.def.id, id).await.unwrap();
        let speaker = voice::Speaker::open(&model.folder, &model.def.lang).unwrap();
        let again = speaker
            .speak("It is a truth universally acknowledged.", Some(id))
            .unwrap();
        assert!(again.iter().any(|sample| sample.abs() > 0.01));
        assert_ne!(again, pcm, "another voice sounds different");
    }
    // An update fetches nothing that has not changed: this server holds the new definition and no files at all.
    let (listener, base) = bind().await;
    let mut def = definition(&base, &files);
    def["version"] = 2.into();
    if let (Some(id), Some(bytes)) = (&other_id, &other_bytes) {
        def = with_voices(def, &base, &[(id.as_str(), bytes.as_slice())]);
    }
    let def = def.to_string().into_bytes();
    serve(
        listener,
        HashMap::from([("voices/test-model.json".to_string(), def.clone())]),
    );
    voice::install(&root, &base, &offered(&def), "1.0.0", |_, _| {})
        .await
        .unwrap();
    let model = voice::installed(&root).remove(0);
    assert_eq!(model.def.version, 2);
    if let Some(id) = &other_id {
        assert!(
            voice::has_choice(&model.folder, id),
            "the fetched voice is kept"
        );
    }
    voice::remove(&root, &model.def.id).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}
