//! An indexer whose request is signed and whose answer is a web page, not JSON.
//!
//! The host calls `search` with `{host, kind, path, query, json}`. This module asks the host to make the
//! request (`http`), reads the HTML that comes back, and answers with the JSON rows that the plugin's
//! `response` definition then reads like any other indexer's answer.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[link(wasm_import_module = "akidev")]
extern "C" {
    fn http(pointer: i32, length: i32) -> i64;
}

/// The host calls this to put the input into this module's memory.
#[no_mangle]
pub extern "C" fn alloc(length: i32) -> i32 {
    let mut space = Vec::<u8>::with_capacity(length as usize);
    let pointer = space.as_mut_ptr();
    std::mem::forget(space);
    pointer as i32
}

/// Hands bytes back: where they are, and how many.
fn give(bytes: Vec<u8>) -> i64 {
    let length = bytes.len();
    let pointer = bytes.leak().as_ptr();
    (((pointer as u32 as u64) << 32) | length as u64) as i64
}

fn take(pointer: i32, length: i32) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(pointer as *const u8, length as usize).to_vec() }
}

/// Asks the host for one request and returns what it answered.
fn fetch(request: &Value) -> Value {
    let bytes = serde_json::to_vec(request).unwrap();
    let packed = unsafe { http(bytes.as_ptr() as i32, bytes.len() as i32) } as u64;
    let answer = take((packed >> 32) as i32, (packed & 0xffff_ffff) as i32);
    serde_json::from_slice(&answer).unwrap_or(Value::Null)
}

/// The text between `start` and `end`, after `from`.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<(&'a str, usize)> {
    let begin = text.find(start)? + start.len();
    let length = text[begin..].find(end)?;
    Some((&text[begin..begin + length], begin + length + end.len()))
}

#[no_mangle]
pub extern "C" fn search(pointer: i32, length: i32) -> i64 {
    let input: Value = serde_json::from_slice(&take(pointer, length)).unwrap_or(Value::Null);
    let host = input["host"].as_str().unwrap_or_default();
    let term = input["query"]
        .as_array()
        .and_then(|pairs| pairs.iter().find(|pair| pair[0] == "q"))
        .and_then(|pair| pair[1].as_str())
        .unwrap_or_default();

    // The service wants each request signed: a hash of a shared secret and the search.
    let signature: String = Sha256::digest(format!("secret:{term}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let answer = fetch(&json!({
        "url": format!("{host}/list?q={term}"),
        "headers": { "x-sign": signature },
    }));

    // The answer is a page: <li data-hash="..." data-seeders="...">Title</li>. Read it into rows.
    let page = answer["body"].as_str().unwrap_or_default();
    let mut rows = Vec::new();
    let mut rest = page;
    while let Some((hash, after)) = between(rest, "data-hash=\"", "\"") {
        let Some((seeders, after)) = between(&rest[after..], "data-seeders=\"", "\"")
            .map(|(seeders, used)| (seeders, after + used))
        else {
            break;
        };
        let Some((title, after)) = between(&rest[after..], ">", "</li>")
            .map(|(title, used)| (title, after + used))
        else {
            break;
        };
        rows.push(json!({ "title": title, "hash": hash, "seeders": seeders.parse::<u64>().unwrap_or(0) }));
        rest = &rest[after..];
    }
    give(serde_json::to_vec(&json!({ "items": rows })).unwrap())
}

/// A catalogue or a mapper asks through this one: the host gives the filled `url` and `query`, this signs
/// the address, has the host fetch it, and answers with the JSON the service returned.
#[no_mangle]
pub extern "C" fn fetch_signed(pointer: i32, length: i32) -> i64 {
    let input: Value = serde_json::from_slice(&take(pointer, length)).unwrap_or(Value::Null);
    let mut url = input["url"].as_str().unwrap_or_default().to_string();
    let pairs: Vec<String> = input["query"]
        .as_array()
        .map(|pairs| {
            pairs
                .iter()
                .map(|pair| format!("{}={}", pair[0].as_str().unwrap_or_default(), pair[1].as_str().unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default();
    if !pairs.is_empty() {
        url = format!("{url}?{}", pairs.join("&"));
    }
    let signature: String = Sha256::digest(format!("secret:{url}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let answer = fetch(&json!({ "url": url, "headers": { "x-sign": signature } }));
    let body = answer["body"].as_str().unwrap_or("null");
    give(body.as_bytes().to_vec())
}
