//! Stream services: a plugin turns a release into a playable https link, the way a debrid service does.
//!
//! The plugin's WebAssembly module only wraps the service's API (see `code.rs`). The app keeps the
//! judgement: it picks which file of the release is the episode, it holds the deadline and does the
//! waiting (a module has no clock), and it decides whether the link it gets back may be played.
//! The protocol is in `pipeline/WASM_PLUGINS.md`.

use crate::definition::Module;
use crate::model::Release;
use crate::releases::{VideoFile, pick_for_identity};
use anyhow::{Context, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// A service a plugin brings.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceDef {
    /// 1 to 24 lower case letters, digits or dashes. The settings keep its key under this name.
    pub id: String,
    pub name: String,
    /// The export of the plugin's module that does the work.
    #[serde(default = "resolve")]
    pub export: String,
    /// The address of the service's API, given to the module so it is data and not code.
    pub base: String,
    /// The hosts the module may call.
    pub hosts: Vec<String>,
    /// The hosts the link it returns may be on (each also matches its subdomains).
    pub media_hosts: Vec<String>,
    /// What the key is called in Settings. Absent: the service needs no key.
    #[serde(default)]
    pub key_label: Option<String>,
    /// The plugin's module, put here when the plugin is put to use. Not in the JSON.
    #[serde(skip)]
    pub module: Option<Module>,
}

fn resolve() -> String {
    "resolve".into()
}

impl ServiceDef {
    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=24).contains(&self.id.len())
                && !self.id.starts_with('-')
                && self
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
            "the service id {:?} must be 1 to 24 lower case letters, digits or dashes",
            self.id
        );
        ensure!(
            (1..=40).contains(&self.name.chars().count()),
            "the service name must be 1 to 40 characters"
        );
        ensure!(
            self.key_label
                .as_ref()
                .is_none_or(|label| (1..=40).contains(&label.chars().count())),
            "the key label must be 1 to 40 characters"
        );
        ensure!(
            (1..=32).contains(&self.export.len())
                && self
                    .export
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
            "the export {:?} must be 1 to 32 lower case letters, digits or _",
            self.export
        );
        crate::plugin::check_public_url(&self.base).context("the service's base address")?;
        ensure!(
            reqwest::Url::parse(&self.base)
                .ok()
                .and_then(|base| base.host_str().map(str::to_ascii_lowercase))
                .is_some_and(|host| self
                    .hosts
                    .iter()
                    .any(|each| each.eq_ignore_ascii_case(&host))),
            "the service's base address is not on one of its hosts"
        );
        for (what, hosts) in [("hosts", &self.hosts), ("media_hosts", &self.media_hosts)] {
            ensure!(
                (1..=4).contains(&hosts.len()),
                "a service needs 1 to 4 {what}"
            );
            for host in hosts {
                ensure!(
                    crate::plugin::is_public_host(host),
                    "{host:?} in {what} is not a public host name"
                );
            }
        }
        Ok(())
    }
}

/// What is being watched, so the right file of a release can be chosen.
#[derive(Debug, Clone)]
pub struct Wanted {
    pub title: String,
    pub titles: Vec<String>,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// How long the app waits between calls at most, whatever a module asks for.
const LONGEST_WAIT: Duration = Duration::from_secs(5);
/// How many calls one resolve may make, so a module that always asks to retry cannot go on.
const MOST_CALLS: usize = 40;

/// A playable link for `release` from `service`, `Ok(None)` when the service does not have it, or why it
/// could not be asked. `key` is given to the module and to nothing else.
pub async fn resolve_release(
    service: &ServiceDef,
    key: &str,
    release: &Release,
    wanted: &Wanted,
    deadline: Instant,
) -> anyhow::Result<Option<String>> {
    resolve_with_public(
        service,
        key,
        release,
        wanted,
        deadline,
        crate::plugin::check_public_url,
    )
    .await
}

/// As `resolve_release`, with the check an address must pass given, so a test can use a server on this machine.
pub(crate) async fn resolve_with_public(
    service: &ServiceDef,
    key: &str,
    release: &Release,
    wanted: &Wanted,
    deadline: Instant,
    public: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Option<String>> {
    resolve_with(service, key, release, wanted, deadline, public).await
}

pub(crate) async fn resolve_with(
    service: &ServiceDef,
    key: &str,
    release: &Release,
    wanted: &Wanted,
    deadline: Instant,
    public: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Option<String>> {
    let base = json!({
        "base": service.base,
        "key": key,
        "release": {
            "hash": release.info_hash,
            "magnet": release.magnet,
            "title": release.title,
            "file_idx": release.file_idx,
        },
        "media": {
            "title": wanted.title,
            "year": wanted.year,
            "season": wanted.season,
            "episode": wanted.episode,
        },
    });

    // First the files of the release, which the service lists when it has the release.
    let (files, state) =
        match ask(service, &base, "files", Value::Null, None, deadline, public).await? {
            Answer::Files(files, state) => (files, state),
            Answer::Unavailable(_) => return Ok(None),
            Answer::Link(_) => bail!("the module answered a link when it was asked for the files"),
        };
    // The app, and not the module, decides which of them is the episode.
    let listing: Vec<VideoFile> = files
        .iter()
        .enumerate()
        .map(|(index, file)| VideoFile {
            index,
            name: file.name.clone(),
            length: file.size,
        })
        .collect();
    let Some(picked) = pick_for_identity(
        &listing,
        release,
        wanted.episode,
        wanted.season,
        &wanted.title,
        &wanted.titles,
        wanted.year,
    ) else {
        return Ok(None);
    };
    let file = &files[picked.index];

    // Then the link to that file.
    match ask(
        service,
        &base,
        "link",
        state,
        Some(&file.id),
        deadline,
        public,
    )
    .await?
    {
        Answer::Link(url) => Ok(Some(url)),
        Answer::Unavailable(_) => Ok(None),
        Answer::Files(..) => bail!("the module answered files when it was asked for a link"),
    }
}

struct ListedFile {
    id: String,
    name: String,
    size: u64,
}

enum Answer {
    Files(Vec<ListedFile>, Value),
    Link(String),
    Unavailable(#[allow(dead_code)] String),
}

/// Calls the module for `want` until it gives an answer, waiting when it asks to retry.
async fn ask(
    service: &ServiceDef,
    base: &Value,
    want: &str,
    mut state: Value,
    file: Option<&str>,
    deadline: Instant,
    public: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Answer> {
    for _ in 0..MOST_CALLS {
        ensure!(Instant::now() < deadline, "the service ran out of time");
        let mut input = base.clone();
        input["want"] = json!(want);
        input["state"] = state.clone();
        if let Some(file) = file {
            input["file"] = json!({ "id": file });
        }
        let answer = call(service, input, public).await?;
        match answer["status"].as_str().unwrap_or_default() {
            "retry" => {
                state = answer["state"].clone();
                let wait = Duration::from_millis(answer["after_ms"].as_u64().unwrap_or(1000))
                    .clamp(Duration::from_millis(100), LONGEST_WAIT);
                ensure!(
                    Instant::now() + wait < deadline,
                    "the service ran out of time"
                );
                tokio::time::sleep(wait).await;
            }
            "unavailable" => {
                let reason = answer["reason"].as_str().unwrap_or("not available");
                return Ok(Answer::Unavailable(reason.chars().take(200).collect()));
            }
            "files" => {
                ensure!(want == "files", "the module listed files unasked");
                let listed = answer["files"].as_array().context("no list of files")?;
                ensure!(listed.len() <= 2000, "too many files");
                let files = listed
                    .iter()
                    .map(|file| {
                        Ok(ListedFile {
                            id: match &file["id"] {
                                Value::String(id) => id.clone(),
                                Value::Number(id) => id.to_string(),
                                _ => bail!("a file has no id"),
                            },
                            name: file["name"].as_str().unwrap_or_default().to_string(),
                            size: file["size"].as_u64().unwrap_or(0),
                        })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                return Ok(Answer::Files(files, answer["state"].clone()));
            }
            "ready" => {
                ensure!(want == "link", "the module gave a link unasked");
                let url = answer["url"].as_str().context("no link")?;
                check_link(url, &service.media_hosts, public)?;
                return Ok(Answer::Link(url.to_string()));
            }
            other => bail!("the module answered {other:?}"),
        }
    }
    bail!("the service kept asking to retry")
}

/// A link the player may open: https, on a public host, on one of the hosts the plugin named.
fn check_link(
    url: &str,
    media_hosts: &[String],
    public: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    public(url).context("the link is not allowed")?;
    let parsed = reqwest::Url::parse(url).context("the link is not an address")?;
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    ensure!(
        media_hosts.iter().any(|allowed| {
            let allowed = allowed.to_ascii_lowercase();
            host == allowed || host.ends_with(&format!(".{allowed}"))
        }),
        "the link is on {host}, which the plugin did not name"
    );
    Ok(())
}

#[cfg(feature = "code-plugins")]
async fn call(
    service: &ServiceDef,
    input: Value,
    #[allow(unused_variables)] public: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let module = service
        .module
        .as_ref()
        .context("the service's module is not loaded")?;
    // The tests run a server on this machine, which the public address rules rightly refuse.
    #[cfg(test)]
    let allowed: fn(&str) -> anyhow::Result<()> = |_| Ok(());
    #[cfg(not(test))]
    let allowed: fn(&str) -> anyhow::Result<()> = crate::plugin::check_public_url;
    crate::code::run_with(
        module.0.to_vec(),
        &service.export,
        input,
        service.hosts.clone(),
        crate::code::Limits::default(),
        allowed,
    )
    .await
}

#[cfg(not(feature = "code-plugins"))]
async fn call(
    _: &ServiceDef,
    _: Value,
    _: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    bail!("this build cannot run a plugin's code")
}

#[cfg(all(test, feature = "code-plugins"))]
pub(crate) mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::{Query, State},
        http::HeaderMap,
        routing::{get, post},
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    pub(crate) const KEY: &str = "secret-key";
    pub(crate) const HASH: &str = "0123456789abcdef0123456789abcdef01234567";

    /// What the fake TorBox has seen, and how it behaves.
    #[derive(Default)]
    pub(crate) struct Fake {
        pub(crate) cached: bool,
        /// How many reads of the new entry come back before its files are listed.
        pub(crate) slow_to_list: usize,
        pub(crate) added: bool,
        pub(crate) reads: usize,
        pub(crate) authorised: usize,
        pub(crate) cdn: String,
    }

    pub(crate) type Shared = Arc<Mutex<Fake>>;

    fn envelope(data: Value) -> Json<Value> {
        Json(json!({ "success": true, "data": data }))
    }

    fn refused(detail: &str) -> Json<Value> {
        Json(json!({ "success": false, "detail": detail }))
    }

    fn entry() -> Value {
        json!({ "id": 7, "hash": HASH, "files": [
            { "id": 1, "name": "Show.S01E05.1080p.WEB.mkv", "size": 900_000_000u64 },
            { "id": 2, "name": "Show.S01E04.1080p.WEB.mkv", "size": 900_000_000u64 },
            { "id": 3, "name": "Show.S01E06.1080p.WEB.mkv", "size": 900_000_000u64 },
            { "id": 4, "name": "Show.S01E01.1080p.WEB.mkv", "size": 900_000_000u64 },
        ]})
    }

    pub(crate) async fn torbox(fake: Shared) -> String {
        let app =
            Router::new()
                .route(
                    "/v1/api/torrents/mylist",
                    get(
                        |State(fake): State<Shared>,
                         Query(query): Query<HashMap<String, String>>,
                         headers: HeaderMap| async move {
                            let mut fake = fake.lock().unwrap();
                            if headers
                                .get("authorization")
                                .and_then(|value| value.to_str().ok())
                                == Some(&format!("Bearer {KEY}"))
                            {
                                fake.authorised += 1;
                            }
                            if query.contains_key("id") {
                                fake.reads += 1;
                                if fake.reads <= fake.slow_to_list {
                                    return envelope(json!({ "id": 7, "hash": HASH, "files": [] }));
                                }
                                return envelope(entry());
                            }
                            envelope(json!([]))
                        },
                    ),
                )
                .route(
                    "/v1/api/torrents/createtorrent",
                    post(|State(fake): State<Shared>, body: String| async move {
                        let mut fake = fake.lock().unwrap();
                        // Only added when asked to add it only if cached, and with a magnet.
                        let asked_right = body.contains("add_only_if_cached")
                            && body.contains("true")
                            && body.contains("magnet:?xt=urn:btih:");
                        if fake.cached && asked_right {
                            fake.added = true;
                            envelope(json!({ "torrent_id": 7 }))
                        } else {
                            refused("not cached")
                        }
                    }),
                )
                .route(
                    "/v1/api/torrents/requestdl",
                    get(
                        |State(fake): State<Shared>,
                         Query(query): Query<HashMap<String, String>>| async move {
                            let fake = fake.lock().unwrap();
                            let right = query.get("token").map(String::as_str) == Some(KEY)
                                && query.get("torrent_id").map(String::as_str) == Some("7");
                            match (right, query.get("file_id")) {
                                (true, Some(file)) => {
                                    envelope(json!(format!("{}/{file}.mkv", fake.cdn)))
                                }
                                _ => refused("bad key"),
                            }
                        },
                    ),
                )
                .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        fake.lock().unwrap().cdn = format!("http://{address}/cdn");
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}/v1/api/torrents")
    }

    pub(crate) fn service(base: &str, media_hosts: &[&str]) -> ServiceDef {
        let mut service: ServiceDef = serde_json::from_value(json!({
            "id": "torbox", "name": "TorBox", "base": base,
            "hosts": ["127.0.0.1"], "media_hosts": media_hosts, "key_label": "TorBox API key",
        }))
        .unwrap();
        service.module = Some(Module(Arc::new(
            include_bytes!("../tests/fixtures/torbox.wasm").to_vec(),
        )));
        service
    }

    pub(crate) fn release() -> Release {
        serde_json::from_value(json!({
            "title": "Show S01 1080p WEB",
            "info_hash": HASH,
            "magnet": format!("magnet:?xt=urn:btih:{HASH}"),
            "size_bytes": 4_000_000_000u64,
            "source": "test",
        }))
        .unwrap()
    }

    pub(crate) fn wanted() -> Wanted {
        Wanted {
            title: "Show".into(),
            titles: vec![],
            year: None,
            season: Some(1),
            episode: Some(5),
        }
    }

    pub(crate) const LENIENT: fn(&str) -> anyhow::Result<()> = |_| Ok(());

    async fn resolve_for(
        service: &ServiceDef,
        key: &str,
        within: Duration,
    ) -> anyhow::Result<Option<String>> {
        resolve_with(
            service,
            key,
            &release(),
            &wanted(),
            Instant::now() + within,
            LENIENT,
        )
        .await
    }

    #[tokio::test]
    async fn a_cached_release_is_added_its_episode_is_picked_by_the_app_and_a_link_comes_back() {
        let fake: Shared = Arc::new(Mutex::new(Fake {
            cached: true,
            slow_to_list: 1,
            ..Fake::default()
        }));
        let base = torbox(fake.clone()).await;
        let service = service(&base, &["127.0.0.1"]);
        let url = resolve_for(&service, KEY, Duration::from_secs(20))
            .await
            .unwrap()
            .expect("a link");
        // Episode 5 is file 1: the app chose it from the list the module gave, not the module.
        assert!(url.ends_with("/cdn/1.mkv"), "{url}");
        let seen = fake.lock().unwrap();
        assert!(seen.added);
        assert!(seen.reads >= 2, "it waited for the files to be listed");
        assert!(seen.authorised > 0, "the key reached the service");
    }

    #[tokio::test]
    async fn a_release_the_service_does_not_have_is_not_available_and_is_not_an_error() {
        let fake: Shared = Arc::new(Mutex::new(Fake::default()));
        let base = torbox(fake.clone()).await;
        let service = service(&base, &["127.0.0.1"]);
        let answer = resolve_for(&service, KEY, Duration::from_secs(10))
            .await
            .unwrap();
        assert!(answer.is_none());
        assert!(!fake.lock().unwrap().added);
    }

    #[tokio::test]
    async fn a_wrong_key_gives_no_link() {
        let fake: Shared = Arc::new(Mutex::new(Fake {
            cached: true,
            ..Fake::default()
        }));
        let base = torbox(fake).await;
        let service = service(&base, &["127.0.0.1"]);
        let answer = resolve_for(&service, "not-the-key", Duration::from_secs(10))
            .await
            .unwrap();
        assert!(answer.is_none());
    }

    #[tokio::test]
    async fn a_link_on_a_host_the_plugin_did_not_name_is_refused() {
        let fake: Shared = Arc::new(Mutex::new(Fake {
            cached: true,
            ..Fake::default()
        }));
        let base = torbox(fake).await;
        let service = service(&base, &["cdn.example.com"]);
        let error = resolve_for(&service, KEY, Duration::from_secs(10))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("did not name"), "{error:#}");
    }

    #[tokio::test]
    async fn a_service_that_is_slow_to_list_its_files_runs_out_of_time() {
        let fake: Shared = Arc::new(Mutex::new(Fake {
            cached: true,
            slow_to_list: 1000,
            ..Fake::default()
        }));
        let base = torbox(fake).await;
        let service = service(&base, &["127.0.0.1"]);
        // The module gives up after a few tries; a short deadline stops the waiting sooner.
        let short = resolve_for(&service, KEY, Duration::from_millis(900)).await;
        assert!(short.is_err() || short.unwrap().is_none());
    }

    #[test]
    fn a_service_definition_is_held_to_the_rules() {
        let good = json!({
            "id": "torbox", "name": "TorBox", "base": "https://api.torbox.app/v1/api/torrents",
            "hosts": ["api.torbox.app"], "media_hosts": ["tb-cdn.io"],
        });
        let parse = |value: &Value| serde_json::from_value::<ServiceDef>(value.clone()).unwrap();
        assert!(parse(&good).check().is_ok());
        let refused = |change: fn(&mut Value)| {
            let mut changed = good.clone();
            change(&mut changed);
            assert!(parse(&changed).check().is_err(), "{changed}");
        };
        refused(|s| s["id"] = json!("Bad Id"));
        refused(|s| s["base"] = json!("http://api.torbox.app/v1"));
        refused(|s| s["base"] = json!("https://elsewhere.example.com/v1"));
        refused(|s| s["hosts"] = json!(["127.0.0.1"]));
        refused(|s| s["media_hosts"] = json!([]));
        refused(|s| s["export"] = json!("Bad Export"));
    }
}
