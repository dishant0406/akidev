//! TorBox as a stream service: a debrid service that already holds a release and serves the file from
//! its own servers, so playback starts at once and nothing joins a torrent swarm.
//!
//! The app calls `resolve` with `{base, key, want, release, media, file, state}` and reads the answer:
//!
//! - `want: "files"`: the files of the release if TorBox holds it (adding it first when it is cached
//!   there), as `{status: "files", files, state}`. Not cached: `{status: "unavailable"}`.
//! - `want: "link"`: a link to the file the app picked, as `{status: "ready", url}`.
//! - `{status: "retry", after_ms, state}` asks the app to call again after a pause.
//!
//! The app chooses which file is the episode. This module only talks to TorBox.

use serde_json::{json, Value};

#[link(wasm_import_module = "akidev")]
extern "C" {
    fn http(pointer: i32, length: i32) -> i64;
}

#[no_mangle]
pub extern "C" fn alloc(length: i32) -> i32 {
    let mut space = Vec::<u8>::with_capacity(length as usize);
    let pointer = space.as_mut_ptr();
    std::mem::forget(space);
    pointer as i32
}

fn give(value: &Value) -> i64 {
    let bytes = serde_json::to_vec(value).unwrap();
    let length = bytes.len();
    let pointer = bytes.leak().as_ptr();
    (((pointer as u32 as u64) << 32) | length as u64) as i64
}

fn take(pointer: i32, length: i32) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(pointer as *const u8, length as usize).to_vec() }
}

/// One request through the host. The answer is `{status, body}` or `{error}`.
fn fetch(request: Value) -> Value {
    let bytes = serde_json::to_vec(&request).unwrap();
    let packed = unsafe { http(bytes.as_ptr() as i32, bytes.len() as i32) } as u64;
    let answer = take((packed >> 32) as i32, (packed & 0xffff_ffff) as i32);
    serde_json::from_slice(&answer).unwrap_or(Value::Null)
}

fn unavailable(reason: &str) -> Value {
    json!({ "status": "unavailable", "reason": reason })
}

/// Text safe to put in an address.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// TorBox wraps every answer as `{success, data}`. The data, or why there is none.
fn api(answer: &Value) -> Result<Value, String> {
    if let Some(error) = answer["error"].as_str() {
        return Err(error.to_string());
    }
    let body: Value = serde_json::from_str(answer["body"].as_str().unwrap_or("null"))
        .map_err(|_| "TorBox did not answer JSON".to_string())?;
    if body["success"].as_bool() != Some(true) {
        let reason = body["detail"].as_str().or(body["error"].as_str());
        return Err(reason.unwrap_or("TorBox refused the request").to_string());
    }
    Ok(body["data"].clone())
}

fn get(base: &str, route: &str, query: &str, key: &str) -> Result<Value, String> {
    api(&fetch(json!({
        "url": format!("{base}/{route}?{query}"),
        "headers": { "authorization": format!("Bearer {key}") },
    })))
}

/// The files of a torrent in the account, if it has any yet.
fn files_of(entry: &Value) -> Option<Vec<Value>> {
    let files = entry["files"].as_array().filter(|files| !files.is_empty())?;
    Some(
        files
            .iter()
            .map(|file| {
                let name = file["name"].as_str().filter(|name| !name.is_empty());
                json!({
                    "id": file["id"],
                    "name": name.or(file["short_name"].as_str()).unwrap_or_default(),
                    "size": file["size"],
                })
            })
            .collect(),
    )
}

fn listed(entry: &Value, torrent_id: &Value) -> Value {
    match files_of(entry) {
        Some(files) => json!({ "status": "files", "files": files, "state": { "torrent_id": torrent_id } }),
        None => unavailable("TorBox has no files for it"),
    }
}

fn files(input: &Value) -> Value {
    let base = input["base"].as_str().unwrap_or_default();
    let key = input["key"].as_str().unwrap_or_default();
    let hash = input["release"]["hash"].as_str().unwrap_or_default().to_ascii_lowercase();
    let magnet = input["release"]["magnet"].as_str().unwrap_or_default();
    let state = &input["state"];

    // A torrent added a moment ago: read it by its id until its files show.
    if let Some(id) = state["torrent_id"].as_i64() {
        return match get(base, "mylist", &format!("id={id}&bypass_cache=true"), key) {
            Ok(data) => {
                let entry = match &data {
                    Value::Array(all) => all.first().cloned().unwrap_or(Value::Null),
                    other => other.clone(),
                };
                if files_of(&entry).is_some() {
                    listed(&entry, &json!(id))
                } else if state["tries"].as_u64().unwrap_or(0) < 6 {
                    json!({ "status": "retry", "after_ms": 700,
                            "state": { "torrent_id": id, "tries": state["tries"].as_u64().unwrap_or(0) + 1 } })
                } else {
                    unavailable("TorBox did not list its files in time")
                }
            }
            Err(reason) => unavailable(&reason),
        };
    }

    // Already in the account?
    if let Ok(Value::Array(all)) = get(base, "mylist", "limit=5000", key) {
        if let Some(entry) = all
            .iter()
            .find(|entry| entry["hash"].as_str().is_some_and(|each| each.eq_ignore_ascii_case(&hash)))
        {
            if files_of(entry).is_some() {
                return listed(entry, &entry["id"]);
            }
        }
    }

    // Not there: add it, but only if TorBox already has it, because one it would have to fetch cannot be
    // watched now. A refusal is the normal answer for most releases.
    let boundary = "----akidev-torbox";
    let form = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"magnet\"\r\n\r\n{magnet}\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"add_only_if_cached\"\r\n\r\ntrue\r\n\
         --{boundary}--\r\n"
    );
    let added = api(&fetch(json!({
        "method": "POST",
        "url": format!("{base}/createtorrent"),
        "headers": {
            "authorization": format!("Bearer {key}"),
            "content-type": format!("multipart/form-data; boundary={boundary}"),
        },
        "body": form,
    })));
    match added {
        Ok(data) => match data["torrent_id"].as_i64() {
            Some(id) => json!({ "status": "retry", "after_ms": 300,
                                "state": { "torrent_id": id, "tries": 0 } }),
            None => unavailable("TorBox added it without an id"),
        },
        Err(_) => unavailable("TorBox does not have it cached"),
    }
}

fn link(input: &Value) -> Value {
    let base = input["base"].as_str().unwrap_or_default();
    let key = input["key"].as_str().unwrap_or_default();
    let torrent = input["state"]["torrent_id"].as_i64();
    let file = input["file"]["id"].as_str().and_then(|id| id.parse::<i64>().ok());
    let (Some(torrent), Some(file)) = (torrent, file) else {
        return unavailable("no torrent or file to link");
    };
    // This route wants the key in the address as well as in the header.
    let query = format!("torrent_id={torrent}&file_id={file}&token={}", encode(key));
    match get(base, "requestdl", &query, key) {
        Ok(data) => match data.as_str() {
            Some(url) => json!({ "status": "ready", "url": url }),
            None => unavailable("TorBox gave no link"),
        },
        Err(reason) => unavailable(&reason),
    }
}

#[no_mangle]
pub extern "C" fn resolve(pointer: i32, length: i32) -> i64 {
    let input: Value = serde_json::from_slice(&take(pointer, length)).unwrap_or(Value::Null);
    let answer = match input["want"].as_str() {
        Some("files") => files(&input),
        Some("link") => link(&input),
        _ => unavailable("not asked for files or a link"),
    };
    give(&answer)
}
