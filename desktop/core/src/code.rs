//! Running a plugin's code: a WebAssembly module that makes requests and answers with JSON.
//!
//! The module replaces only the step JSON cannot say (signing, a login, scraping a page). What it
//! answers is read by the plugin's own declarative definition, so every check after it is unchanged.
//! See `pipeline/WASM_PLUGINS.md` for the ABI and the rules.
//!
//! The module is untrusted. It gets no files and no clock, and no network except through `http`, which
//! is held to the rules of a JSON plugin and to the hosts the plugin declared. Its CPU is bounded by
//! fuel, its memory by a limit and its size of answer by a cap.

use anyhow::{Context, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
};

/// What a call may use. The defaults are for a plugin; a test may lower them.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Instructions the call may run. An endless loop stops here.
    pub fuel: u64,
    /// The most memory the module may hold.
    pub memory_bytes: usize,
    /// The longest the whole call may take, requests included.
    pub wall: Duration,
    /// How many requests one call may make.
    pub requests: u32,
    /// The largest answer to one request, and the largest input and output of a call.
    pub bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 300_000_000,
            memory_bytes: 64 << 20,
            wall: Duration::from_secs(20),
            requests: 8,
            bytes: 2 << 20,
        }
    }
}

use crate::plugin::MAX_MODULE_BYTES;

/// One request a module asks the host to make.
#[derive(Deserialize)]
struct Ask {
    #[serde(default = "get")]
    method: String,
    url: String,
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

fn get() -> String {
    "GET".into()
}

/// What the host answers: the status and the text, or why there is none.
#[derive(Serialize)]
struct Answer {
    status: u16,
    body: String,
}

/// The state of one call.
struct Host {
    limits: StoreLimits,
    handle: tokio::runtime::Handle,
    /// The host names the plugin declared. A request to any other is refused.
    hosts: Vec<String>,
    requests_left: u32,
    max_bytes: usize,
    deadline: Instant,
    /// Whether an address is allowed at all: the public https rules of a plugin.
    allowed: fn(&str) -> anyhow::Result<()>,
}

/// Runs `export` of the module with `input`, and returns what it answers.
pub async fn run(
    module: Vec<u8>,
    export: &str,
    input: Value,
    hosts: Vec<String>,
    limits: Limits,
) -> anyhow::Result<Value> {
    run_with(
        module,
        export,
        input,
        hosts,
        limits,
        crate::plugin::check_public_url,
    )
    .await
}

pub(crate) async fn run_with(
    module: Vec<u8>,
    export: &str,
    input: Value,
    hosts: Vec<String>,
    limits: Limits,
    allowed: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    ensure!(module.len() <= MAX_MODULE_BYTES, "the module is too large");
    let handle = tokio::runtime::Handle::current();
    let export = export.to_string();
    let wall = limits.wall;
    // The module runs on a thread of its own, because it is not async and its requests wait on the network.
    // The wall clock below gives the caller its answer on time; fuel ends the thread soon after.
    let job = tokio::task::spawn_blocking(move || {
        call(&module, &export, &input, hosts, &limits, handle, allowed)
    });
    match tokio::time::timeout(wall, job).await {
        Ok(joined) => joined.context("the module's thread failed")?,
        Err(_) => bail!("the module took longer than {} seconds", wall.as_secs()),
    }
}

fn call(
    module: &[u8],
    export: &str,
    input: &Value,
    hosts: Vec<String>,
    limits: &Limits,
    handle: tokio::runtime::Handle,
    allowed: fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let input = serde_json::to_vec(input)?;
    ensure!(input.len() <= limits.bytes, "the input is too large");

    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, module).map_err(|error| anyhow!("not a module: {error}"))?;
    let mut store = Store::new(
        &engine,
        Host {
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.memory_bytes)
                .memories(1)
                .tables(4)
                .instances(1)
                .trap_on_grow_failure(true)
                .build(),
            handle,
            hosts,
            requests_left: limits.requests,
            max_bytes: limits.bytes,
            deadline: Instant::now() + limits.wall,
            allowed,
        },
    );
    store.limiter(|host| &mut host.limits);
    store
        .set_fuel(limits.fuel)
        .map_err(|error| anyhow!("fuel: {error}"))?;

    let mut linker = <Linker<Host>>::new(&engine);
    linker
        .func_wrap("akidev", "http", http)
        .map_err(|error| anyhow!("{error}"))?;
    linker
        .func_wrap("akidev", "log", log)
        .map_err(|error| anyhow!("{error}"))?;
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| anyhow!("the module cannot start: {error}"))?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or_else(|| anyhow!("the module exports no memory"))?;
    let alloc = instance
        .get_typed_func::<i32, i32>(&store, "alloc")
        .map_err(|error| anyhow!("the module exports no alloc: {error}"))?;
    let entry = instance
        .get_typed_func::<(i32, i32), i64>(&store, export)
        .map_err(|error| anyhow!("the module exports no {export}: {error}"))?;

    let at = alloc
        .call(&mut store, input.len() as i32)
        .map_err(|error| anyhow!("alloc: {error}"))?;
    memory
        .write(&mut store, at as usize, &input)
        .map_err(|error| anyhow!("the input does not fit: {error}"))?;
    let packed = entry
        .call(&mut store, (at, input.len() as i32))
        .map_err(|error| anyhow!("{export} failed: {error}"))?;

    let (pointer, length) = unpack(packed);
    ensure!(length <= limits.bytes, "the answer is too large");
    let mut out = vec![0; length];
    memory
        .read(&store, pointer, &mut out)
        .map_err(|_| anyhow!("the answer is not in the module's memory"))?;
    serde_json::from_slice(&out).context("the answer is not JSON")
}

/// `(pointer << 32) | length`, as the module returns it.
fn unpack(packed: i64) -> (usize, usize) {
    let packed = packed as u64;
    ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize)
}

fn pack(pointer: i32, length: usize) -> i64 {
    (((pointer as u32 as u64) << 32) | length as u64) as i64
}

/// A line from the module for the log.
fn log(mut caller: Caller<'_, Host>, pointer: i32, length: i32) {
    if let Some(text) = read_text(&mut caller, pointer, length, 2000) {
        tracing::debug!(target: "plugin", "{text}");
    }
}

fn read_text(
    caller: &mut Caller<'_, Host>,
    pointer: i32,
    length: i32,
    most: usize,
) -> Option<String> {
    let length = usize::try_from(length)
        .ok()
        .filter(|length| *length <= most)?;
    let memory = caller.get_export("memory").and_then(Extern::into_memory)?;
    let mut bytes = vec![0; length];
    memory
        .read(&*caller, usize::try_from(pointer).ok()?, &mut bytes)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// One request, made for the module. It answers `{status, body}`, or `{error}` when the request is
/// refused or fails, so the module can decide what to do.
fn http(mut caller: Caller<'_, Host>, pointer: i32, length: i32) -> Result<i64, wasmi::Error> {
    let max = caller.data().max_bytes;
    let answer = match read_text(&mut caller, pointer, length, max) {
        None => serde_json::json!({ "error": "the request is not text in the module's memory" }),
        Some(text) => match ask(caller.data_mut(), &text) {
            Ok(answer) => serde_json::to_value(answer).unwrap_or_default(),
            Err(error) => serde_json::json!({ "error": format!("{error:#}") }),
        },
    };
    let bytes =
        serde_json::to_vec(&answer).map_err(|error| wasmi::Error::new(error.to_string()))?;
    let alloc = caller
        .get_export("alloc")
        .and_then(Extern::into_func)
        .ok_or_else(|| wasmi::Error::new("the module exports no alloc"))?
        .typed::<i32, i32>(&caller)
        .map_err(|error| wasmi::Error::new(error.to_string()))?;
    let at = alloc.call(&mut caller, bytes.len() as i32)?;
    let memory = caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmi::Error::new("the module exports no memory"))?;
    memory
        .write(&mut caller, at as usize, &bytes)
        .map_err(|error| wasmi::Error::new(error.to_string()))?;
    Ok(pack(at, bytes.len()))
}

/// Checks a request against the rules, makes it, and reads the answer within the cap.
fn ask(host: &mut Host, text: &str) -> anyhow::Result<Answer> {
    let ask: Ask =
        serde_json::from_str(text).context("the request is not JSON of the right shape")?;
    ensure!(
        host.requests_left > 0,
        "the module has made all the requests it may"
    );
    host.requests_left -= 1;
    ensure!(Instant::now() < host.deadline, "the module is out of time");
    (host.allowed)(&ask.url)?;
    let url = reqwest::Url::parse(&ask.url).context("the address is not one")?;
    let name = url.host_str().unwrap_or_default().to_ascii_lowercase();
    ensure!(
        host.hosts
            .iter()
            .any(|declared| declared.eq_ignore_ascii_case(&name)),
        "{name} is not a host the plugin declared"
    );
    let method = match ask.method.to_ascii_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        other => bail!("{other} is not a method a plugin may use"),
    };
    let mut request = crate::http::client()
        .request(method, url)
        .timeout(Duration::from_secs(10));
    for (key, value) in &ask.headers {
        ensure!(
            !crate::plugin::FORBIDDEN_HEADERS.contains(&key.to_ascii_lowercase().as_str()),
            "the header {key} may not be set"
        );
        request = request.header(key, value);
    }
    if let Some(body) = ask.body {
        ensure!(body.len() <= host.max_bytes, "the body is too large");
        request = request.body(body);
    }
    let max = host.max_bytes;
    host.handle.block_on(async move {
        let response = request.send().await.context("the request failed")?;
        let status = response.status().as_u16();
        let bytes = crate::http::read_capped(response, max).await?;
        Ok(Answer {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A module with the exports the host needs, and the entries each test asks for.
    fn guest(imports: &str, data: &str, entries: &str) -> Vec<u8> {
        format!(
            r#"(module
                {imports}
                (memory (export "memory") 17)
                {data}
                (global $heap (mut i32) (i32.const 8192))
                (func (export "alloc") (param $n i32) (result i32)
                    (local $at i32)
                    (local.set $at (global.get $heap))
                    (global.set $heap (i32.add (global.get $heap) (local.get $n)))
                    (local.get $at))
                (func (export "echo") (param $p i32) (param $n i32) (result i64)
                    (i64.or (i64.shl (i64.extend_i32_u (local.get $p)) (i64.const 32))
                            (i64.extend_i32_u (local.get $n))))
                {entries}
            )"#
        )
        .into_bytes()
    }

    fn plain(entries: &str) -> Vec<u8> {
        guest("", "", entries)
    }

    const ANYWHERE: fn(&str) -> anyhow::Result<()> = |_| Ok(());

    async fn run_test(
        module: Vec<u8>,
        export: &str,
        hosts: &[&str],
        limits: Limits,
    ) -> anyhow::Result<Value> {
        run_with(
            module,
            export,
            json!({ "term": "show" }),
            hosts.iter().map(|host| host.to_string()).collect(),
            limits,
            ANYWHERE,
        )
        .await
    }

    #[tokio::test]
    async fn a_module_is_given_json_and_answers_json() {
        let answer = run_test(plain(""), "echo", &[], Limits::default())
            .await
            .unwrap();
        assert_eq!(answer, json!({ "term": "show" }));
    }

    #[tokio::test]
    async fn something_that_is_not_a_module_or_lacks_an_export_is_refused() {
        let refused = |result: anyhow::Result<Value>| result.unwrap_err().to_string();
        assert!(
            refused(run_test(b"not wasm".to_vec(), "echo", &[], Limits::default()).await)
                .contains("not a module")
        );
        assert!(
            refused(run_test(plain(""), "missing", &[], Limits::default()).await)
                .contains("exports no missing")
        );
        let too_big = vec![0; MAX_MODULE_BYTES + 1];
        assert!(
            refused(run_test(too_big, "echo", &[], Limits::default()).await).contains("too large")
        );
    }

    #[tokio::test]
    async fn an_endless_loop_runs_out_of_fuel() {
        let module = plain(
            r#"(func (export "spin") (param i32 i32) (result i64)
                (loop $again (br $again)) (i64.const 0))"#,
        );
        let limits = Limits {
            fuel: 1_000_000,
            ..Limits::default()
        };
        let error = run_test(module, "spin", &[], limits).await.unwrap_err();
        assert!(error.to_string().contains("failed"), "{error}");
    }

    #[tokio::test]
    async fn a_module_that_grows_its_memory_without_end_is_stopped() {
        let module = plain(
            r#"(func (export "grow") (param i32 i32) (result i64)
                (loop $again (drop (memory.grow (i32.const 1000))) (br $again)) (i64.const 0))"#,
        );
        let limits = Limits {
            memory_bytes: 8 << 20,
            ..Limits::default()
        };
        let error = run_test(module, "grow", &[], limits).await.unwrap_err();
        assert!(error.to_string().contains("failed"), "{error}");
    }

    #[tokio::test]
    async fn a_call_that_takes_too_long_is_given_up_on() {
        let module = plain(
            r#"(func (export "spin") (param i32 i32) (result i64)
                (loop $again (br $again)) (i64.const 0))"#,
        );
        let limits = Limits {
            wall: Duration::from_millis(100),
            // Enough fuel that only the clock can stop it in time; it ends on its own soon after.
            fuel: 50_000_000,
            ..Limits::default()
        };
        let error = run_test(module, "spin", &[], limits).await.unwrap_err();
        assert!(error.to_string().contains("took longer"), "{error}");
    }

    #[tokio::test]
    async fn an_answer_that_is_too_large_or_not_json_is_refused() {
        let module = plain(
            r#"(func (export "big") (param i32 i32) (result i64)
                    (i64.or (i64.shl (i64.const 100) (i64.const 32)) (i64.const 100000)))
               (func (export "text") (param i32 i32) (result i64)
                    (i64.or (i64.shl (i64.const 100) (i64.const 32)) (i64.const 4)))"#,
        );
        let limits = Limits {
            bytes: 1000,
            ..Limits::default()
        };
        let big = run_test(module.clone(), "big", &[], limits.clone()).await;
        assert!(big.unwrap_err().to_string().contains("too large"));
        // Four bytes of zeros are not JSON.
        let text = run_test(module, "text", &[], limits).await;
        assert!(text.unwrap_err().to_string().contains("not JSON"));
    }

    /// A module that makes `times` requests, and answers with the last answer it got.
    fn fetcher(request: &str, times: u32) -> Vec<u8> {
        let escaped = request.replace('\\', "\\\\").replace('"', "\\\"");
        let calls =
            "(local.set $last (call $http (i32.const 0) (i32.const LEN)))\n".repeat(times as usize);
        guest(
            r#"(import "akidev" "http" (func $http (param i32 i32) (result i64)))"#,
            &format!(r#"(data (i32.const 0) "{escaped}")"#),
            &format!(
                r#"(func (export "fetch") (param i32 i32) (result i64) (local $last i64)
                    {calls}
                    (local.get $last))"#
            )
            .replace("LEN", &request.len().to_string()),
        )
    }

    /// A server on this machine that answers `body` to every request, and the address of it.
    async fn server(body: &'static str) -> String {
        let app = axum::Router::new().fallback(axum::routing::get(move || async move { body }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}/page")
    }

    #[tokio::test]
    async fn a_module_can_make_a_request_to_a_host_it_declared_and_read_the_answer() {
        let url = server("hello").await;
        let request = json!({ "url": url }).to_string();
        let answer = run_test(
            fetcher(&request, 1),
            "fetch",
            &["127.0.0.1"],
            Limits::default(),
        )
        .await
        .unwrap();
        assert_eq!(answer, json!({ "status": 200, "body": "hello" }));
    }

    #[tokio::test]
    async fn a_request_to_a_host_it_did_not_declare_is_refused_and_the_module_is_told_why() {
        let url = server("hello").await;
        let request = json!({ "url": url }).to_string();
        let answer = run_test(
            fetcher(&request, 1),
            "fetch",
            &["example.com"],
            Limits::default(),
        )
        .await
        .unwrap();
        assert!(
            answer["error"]
                .as_str()
                .unwrap()
                .contains("not a host the plugin declared")
        );
    }

    #[tokio::test]
    async fn the_public_https_rules_of_a_plugin_apply_to_what_a_module_asks() {
        // The real check, not the lenient one the other tests use: this machine is not a public https host.
        let url = server("hello").await;
        let request = json!({ "url": url }).to_string();
        let answer = run(
            fetcher(&request, 1),
            "fetch",
            json!({}),
            vec!["127.0.0.1".into()],
            Limits::default(),
        )
        .await
        .unwrap();
        assert!(
            answer["error"].as_str().unwrap().contains("https"),
            "{answer}"
        );
    }

    #[tokio::test]
    async fn the_methods_and_headers_a_module_may_use_are_limited() {
        let url = server("hello").await;
        let method = json!({ "url": url, "method": "DELETE" }).to_string();
        let answer = run_test(
            fetcher(&method, 1),
            "fetch",
            &["127.0.0.1"],
            Limits::default(),
        )
        .await
        .unwrap();
        assert!(answer["error"].as_str().unwrap().contains("method"));
        let header = json!({ "url": url, "headers": { "Host": "evil.example" } }).to_string();
        let answer = run_test(
            fetcher(&header, 1),
            "fetch",
            &["127.0.0.1"],
            Limits::default(),
        )
        .await
        .unwrap();
        assert!(answer["error"].as_str().unwrap().contains("may not be set"));
    }

    #[tokio::test]
    async fn a_module_may_only_make_so_many_requests() {
        let url = server("hello").await;
        let request = json!({ "url": url }).to_string();
        let limits = Limits {
            requests: 2,
            ..Limits::default()
        };
        // The third answer is the refusal.
        let answer = run_test(fetcher(&request, 3), "fetch", &["127.0.0.1"], limits)
            .await
            .unwrap();
        assert!(
            answer["error"]
                .as_str()
                .unwrap()
                .contains("all the requests")
        );
    }

    #[tokio::test]
    async fn an_answer_larger_than_the_cap_is_not_handed_over() {
        let url = server(
            "a response that is far longer than the cap, a response that is far longer than the cap, \
             a response that is far longer than the cap, a response that is far longer than the cap",
        )
        .await;
        let request = json!({ "url": url }).to_string();
        let limits = Limits {
            bytes: 120,
            ..Limits::default()
        };
        let answer = run_test(fetcher(&request, 1), "fetch", &["127.0.0.1"], limits)
            .await
            .unwrap();
        assert!(answer.get("error").is_some(), "{answer}");
    }
}
