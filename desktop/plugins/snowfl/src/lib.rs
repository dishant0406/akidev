//! Snowfl meta-search for the desktop app.
//!
//! Snowfl gives a public client key through its homepage and JavaScript bundle.
//! The module reads that key at runtime, searches, and returns torrent rows.

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

fn fetch(request: &Value) -> Value {
    let bytes = serde_json::to_vec(request).unwrap();
    let packed = unsafe { http(bytes.as_ptr() as i32, bytes.len() as i32) } as u64;
    let answer = take((packed >> 32) as i32, (packed & 0xffff_ffff) as i32);
    serde_json::from_slice(&answer).unwrap_or(Value::Null)
}

fn error(message: &str) -> Value {
    json!({ "error": message })
}

fn body(answer: &Value) -> Result<&str, String> {
    if let Some(message) = answer["error"].as_str() {
        return Err(message.to_string());
    }
    let status = answer["status"].as_u64().unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("Snowfl returned HTTP {status}"));
    }
    answer["body"]
        .as_str()
        .filter(|body| !body.is_empty())
        .ok_or_else(|| "Snowfl returned no body".to_string())
}

fn get(url: &str) -> Result<String, String> {
    let answer = fetch(&json!({
        "url": url,
        "headers": { "user-agent": "Mozilla/5.0" },
    }));
    body(&answer).map(str::to_string)
}

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

fn base64(text: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(c & 63) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn script_path(home: &str) -> Option<&str> {
    let at = home.find("b.min.js")?;
    let rest = &home[at..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn client_key(script: &str) -> Option<&str> {
    let at = script.find("findNextItem")?;
    let rest = &script[at..];
    let quote = rest.find('"')?;
    let after = &rest[quote + 1..];
    let end = after.find('"')?;
    let key = &after[..end];
    (!key.is_empty()).then_some(key)
}

fn info_hash(magnet: &str) -> Option<String> {
    let query = magnet.split_once('?')?.1;
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if !name.eq_ignore_ascii_case("xt") {
            continue;
        }
        let lower = value.to_ascii_lowercase();
        let Some(hash) = lower.strip_prefix("urn:btih:") else {
            continue;
        };
        if !hash.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            continue;
        }
        let hex = hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
        let base32 = hash.len() == 32
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || (b'2'..=b'7').contains(&byte));
        if hex || base32 {
            return Some(hash.to_string());
        }
    }
    None
}

fn media_type_allowed(row: &Value) -> bool {
    if row["nsfw"].as_bool() == Some(true) {
        return false;
    }
    let kind = row["type"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    kind.is_empty()
        || ![
            "music", "audio", "game", "software", "ebook", "document", "image", "xxx",
        ]
        .iter()
        .any(|word| kind.contains(word))
}

fn rows(body: &str) -> Result<Vec<Value>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "Snowfl did not return JSON".to_string())?;
    match value {
        Value::Array(rows) => Ok(rows),
        Value::Object(value) => value
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "Snowfl returned no result list".to_string()),
        _ => Err("Snowfl returned no result list".to_string()),
    }
}

fn magnet_url(host: &str, key: &str, row: &Value) -> Option<String> {
    let site = row["site"].as_str()?;
    let url = row["url"].as_str()?;
    Some(format!(
        "{host}/{key}/{}/{}",
        encode(site),
        base64(&encode(url))
    ))
}

fn resolved_magnet(host: &str, key: &str, row: &Value) -> Option<String> {
    let url = magnet_url(host, key, row)?;
    let answer = get(&url).ok()?;
    let value: Value = serde_json::from_str(&answer).ok()?;
    value["url"]
        .as_str()
        .map(str::to_string)
        .or_else(|| answer.starts_with("magnet:").then_some(answer))
}

fn title(row: &Value) -> String {
    let name = row["name"].as_str().unwrap_or_default().trim();
    let site = row["site"].as_str().unwrap_or_default().trim();
    match (name.is_empty(), site.is_empty()) {
        (true, _) => String::new(),
        (false, true) => name.to_string(),
        (false, false) => format!("{name} [{site}]"),
    }
}

fn seeders(row: &Value) -> Value {
    row["seeder"]
        .as_u64()
        .map(Value::from)
        .or_else(|| row["seeder"].as_str()?.parse::<u64>().ok().map(Value::from))
        .unwrap_or(Value::Null)
}

#[no_mangle]
pub extern "C" fn search(pointer: i32, length: i32) -> i64 {
    let input: Value = serde_json::from_slice(&take(pointer, length)).unwrap_or(Value::Null);
    let host = input["host"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches('/');
    let term = input["query"]
        .as_array()
        .and_then(|pairs| pairs.iter().find(|pair| pair[0] == "q"))
        .and_then(|pair| pair[1].as_str())
        .unwrap_or_default();
    if term.chars().count() < 3 || host.is_empty() {
        return give(&error(
            "Snowfl needs a host and a query of at least three characters",
        ));
    }

    let home = match get(&format!("{host}/")) {
        Ok(home) => home,
        Err(message) => return give(&error(&message)),
    };
    let Some(path) = script_path(&home) else {
        return give(&error("Snowfl homepage has no client script"));
    };
    let script_url = if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{host}/{path}")
    };
    let script = match get(&script_url) {
        Ok(script) => script,
        Err(message) => return give(&error(&message)),
    };
    let Some(key) = client_key(&script) else {
        return give(&error("Snowfl client script has no key"));
    };
    let url = format!("{host}/{key}/{}/DH5kKsJw/0/SEED/NONE/0", encode(term));
    let body = match get(&url) {
        Ok(body) => body,
        Err(message) => return give(&error(&message)),
    };
    let found = match rows(&body) {
        Ok(found) => found,
        Err(message) => return give(&error(&message)),
    };

    let mut requests = 3;
    let mut items = Vec::new();
    for row in found {
        if !media_type_allowed(&row) {
            continue;
        }
        let mut magnet = row["magnet"].as_str().unwrap_or_default().to_string();
        if magnet.is_empty() && requests < 8 {
            requests += 1;
            magnet = resolved_magnet(host, key, &row).unwrap_or_default();
        }
        let Some(hash) = info_hash(&magnet) else {
            continue;
        };
        let name = title(&row);
        if name.is_empty() {
            continue;
        }
        items.push(json!({
            "title": name,
            "hash": hash,
            "size": row["size"],
            "seeders": seeders(&row),
        }));
    }
    give(&json!({ "items": items }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_public_script_and_client_key() {
        assert_eq!(
            script_path("<script src=\"/b.min.js?v=42\"></script>"),
            Some("b.min.js?v=42")
        );
        assert_eq!(
            client_key("const x = findNextItem(items, \"public-key\", 0);"),
            Some("public-key")
        );
    }

    #[test]
    fn reads_hex_and_base32_hashes_only() {
        assert_eq!(
            info_hash("magnet:?xt=urn:btih:ABCDEF0123456789ABCDEF0123456789ABCDEF01"),
            Some("abcdef0123456789abcdef0123456789abcdef01".to_string())
        );
        assert_eq!(
            info_hash("magnet:?xt=urn:btih:QCQTJORS4NU3BKJHPAYLRRPR2CJ3GRR4"),
            Some("qcqtjors4nu3bkjhpaylrrpr2cj3grr4".to_string())
        );
        assert_eq!(
            info_hash("magnet:?xt=urn:btih:qcqtjors4nu3bkjhpaylrrpr2cj3grr4"),
            Some("qcqtjors4nu3bkjhpaylrrpr2cj3grr4".to_string())
        );
        assert_eq!(
            info_hash("magnet:?dn=btih:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB&xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        );
        assert_eq!(
            info_hash("magnet:?dn=xt=urn:btih:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB&xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        );
        assert_eq!(info_hash("magnet:?dn=btih:short"), None);
        assert_eq!(info_hash("magnet:?xt=urn:btih:short"), None);
    }

    #[test]
    fn accepts_an_array_or_the_wrapper_data_array() {
        assert_eq!(rows("[{\"name\":\"Show\"}]").unwrap().len(), 1);
        assert_eq!(rows("{\"data\":[{\"name\":\"Show\"}]}").unwrap().len(), 1);
        assert!(rows("{\"status\":403}").is_err());
    }

    #[test]
    fn builds_the_current_magnet_resolution_route() {
        let row = json!({
            "site": "Example Site",
            "url": "https://example.com/a b",
        });
        assert_eq!(
            magnet_url("https://snowfl.com", "key", &row).as_deref(),
            Some("https://snowfl.com/key/Example%20Site/aHR0cHMlM0ElMkYlMkZleGFtcGxlLmNvbSUyRmElMjBi")
        );
    }

    #[test]
    fn keeps_video_rows_and_removes_music_and_nsfw_rows() {
        let video = json!({ "type": "Video", "nsfw": false });
        let music = json!({ "type": "Music", "nsfw": false });
        let nsfw = json!({ "type": "Video", "nsfw": true });
        assert!(media_type_allowed(&video));
        assert!(!media_type_allowed(&music));
        assert!(!media_type_allowed(&nsfw) && nsfw["nsfw"].as_bool() == Some(true));
    }
}
