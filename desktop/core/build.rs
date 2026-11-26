// The sources document is built into the app (`include_str!` in src/sources.rs), but it lives in its own
// repo. When `content/sources/sources.json` is missing, download it from there. To use your own copy, put it at
// that path (or set AKIDEV_SOURCES_URL); a file that is already there is never replaced.

use std::{fs, path::Path, process::Command};

const DEFAULT_URL: &str =
    "https://raw.githubusercontent.com/dishant0406/akidev-sources/main/sources.json";

fn main() {
    println!("cargo:rerun-if-env-changed=AKIDEV_SOURCES_URL");
    let path = Path::new("../../content/sources/sources.json");
    println!("cargo:rerun-if-changed={}", path.display());
    if path.exists() {
        return;
    }
    let url = std::env::var("AKIDEV_SOURCES_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    fs::create_dir_all("../../content/sources").expect("cannot create content/sources");
    // Download to a side file first, so a failed download never leaves a half file that looks real.
    let part = path.with_extension("json.part");
    let done = Command::new("curl")
        .args(["-fsS", "--retry", "3", "-o"])
        .arg(&part)
        .arg(&url)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    let text = fs::read_to_string(&part).unwrap_or_default();
    if !done || !text.trim_start().starts_with('{') {
        let _ = fs::remove_file(&part);
        panic!(
            "content/sources/sources.json is missing and could not be downloaded from {url}. \
             Get it from https://github.com/dishant0406/akidev-sources and put it at content/sources/sources.json."
        );
    }
    fs::rename(&part, path).expect("cannot save sources.json");
}
