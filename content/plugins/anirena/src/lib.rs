//! AniRena indexer module: the search answer is an HTML page and the info hash is only on each
//! torrent's detail page, so this module fetches the search page and then the detail pages.
//!
//! The app calls `search` with {host, kind, path, query, json}. It answers {"items":
//! [{title, hash, size, seeders}]} and the plugin's own `response` reads that JSON, so the app's
//! normal checks (a valid hash, the right episode, ranking) still apply to every row.

use serde_json::{json, Value};

#[link(wasm_import_module = "akidev")]
extern "C" {
    fn http(pointer: i32, length: i32) -> i64;
}

// ---- plumbing -----------------------------------------------------------------------------------

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

fn fetch(request: Value) -> Value {
    let bytes = serde_json::to_vec(&request).unwrap();
    let packed = unsafe { http(bytes.as_ptr() as i32, bytes.len() as i32) } as u64;
    let answer = take((packed >> 32) as i32, (packed & 0xffff_ffff) as i32);
    serde_json::from_slice(&answer).unwrap_or(Value::Null)
}

// ---- small helpers for reading HTML -------------------------------------------------------------

/// The text between `start` and `end`, searching from the beginning of `text`.
/// Returns it and the position just after `end`.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<(&'a str, usize)> {
    let from = text.find(start)? + start.len();
    let length = text[from..].find(end)?;
    Some((&text[from..from + length], from + length + end.len()))
}

fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for character in html.chars() {
        match character {
            '<' => inside = true,
            '>' => inside = false,
            other if !inside => out.push(other),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

/// The leading number of a cell's text (seeders are shown with a warning sign after the count).
fn leading_number(text: &str) -> Option<u64> {
    let digits: String = text
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// A 40-hex-digit info hash, in lower case.
fn is_info_hash(hash: &str) -> bool {
    hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---- the search ---------------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn search(pointer: i32, length: i32) -> i64 {
    let input: Value = serde_json::from_slice(&take(pointer, length)).unwrap_or(Value::Null);
    let host = input["host"].as_str().unwrap_or_default();
    let term = input["query"]
        .as_array()
        .and_then(|pairs| pairs.iter().find(|pair| pair[0] == "q"))
        .and_then(|pair| pair[1].as_str())
        .unwrap_or_default();

    // The search page: https://www.anirena.com/?q=<term>  (request 1 of the 8 the sandbox allows)
    let answer = fetch(json!({
        "url": format!("{host}/?q={}", encode(term)),
        "headers": {},
    }));
    let page = answer["body"].as_str().unwrap_or_default();

    // Each result row starts at its name cell (<td class="col-name">, which holds the detail link
    // /torrent/<uuid> and the title) and ends at the row's </tr>. The size and seeders cells are
    // between them.
    let mut items = Vec::new();
    let mut details = 0usize;
    let mut rest = page;
    while let Some(at_name) = rest.find("<td class=\"col-name\">") {
        let after_name = at_name + "<td class=\"col-name\">".len();
        let Some(name_end) = rest[after_name..].find("</td>").map(|p| after_name + p) else { break };
        let Some(row_end) = rest[name_end..].find("</tr>").map(|p| name_end + p) else { break };
        let row = &rest[at_name..row_end];
        rest = &rest[row_end..];

        let Some((uuid, uuid_at)) = between(row, "href=\"/torrent/", "\"") else { continue };
        // The row's title anchor is the one on /torrent/<uuid>; an earlier anchor can be the group's.
        let after_uuid = uuid_at - uuid.len();
        let Some((title, _)) = between(&row[after_uuid..], ">", "</a>") else { continue };
        let title = plain(title);
        if title.is_empty() {
            continue;
        }
        let size = between(row, "col-size", ">").and_then(|(_, at)| {
            let cell = &row[at..];
            cell.find("</td>").map(|end| plain(&cell[..end]))
        });
        let seeders = between(row, "col-se\"", "</td>").and_then(|(cell, _)| leading_number(&plain(cell)));

        // The info hash is only on the detail page. One request per row (requests 2..8 of 8), so
        // rows beyond the seventh carry no hash and are dropped by the filter below.
        let hash = if details < 7 {
            details += 1;
            let detail = fetch(json!({
                "url": format!("{host}/torrent/{uuid}"),
                "headers": {},
            }));
            detail["body"]
                .as_str()
                .and_then(|page| between(page, "data-label=\"Info Hash (SHA-1)\">", "<"))
                .map(|(hash, _)| hash.trim().to_ascii_lowercase())
                .filter(|hash| is_info_hash(hash))
        } else {
            None
        };

        if let Some(hash) = hash {
            items.push(json!({ "title": title, "hash": hash, "size": size, "seeders": seeders }));
        }
    }
    give(&json!({ "items": items }))
}
