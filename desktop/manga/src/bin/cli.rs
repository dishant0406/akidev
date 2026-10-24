//! `akidev-manga-cli`: check a source definition against recorded pages, or ask a live site.
//!
//!   check <definition.json> <fixtures-dir> [--themes <dir>] [--bless]
//!   index <repo-dir> [--key <secret-file>]     write index.json (and index.json.sig) for a repository folder
//!   record <definition.json> <fixtures-dir> <call>... [--themes <dir>]   ask the live site and save its answers as fixtures
//!                                              calls: popular, latest, search=TERM, details=ID, chapters=ID, pages=SERIES,CHAPTER
//!   keygen <secret-file>                       make a signing key, print its public half
//!   run   <definition.json> <popular|latest|search TERM|details ID|chapters ID|pages SERIES CHAPTER> [--themes <dir>]

use akidev_manga::theme::Themes;
use akidev_manga::{Source, check};
use anyhow::{Context, bail};
use std::path::{Path, PathBuf};

fn themes(dir: Option<&Path>) -> anyhow::Result<Themes> {
    let mut themes = Themes::default();
    if let Some(dir) = dir {
        for entry in
            std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?
        {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let name = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default()
                    .to_string();
                themes.insert(
                    &name,
                    serde_json::from_str(&std::fs::read_to_string(&path)?)
                        .with_context(|| format!("{} is not JSON", path.display()))?,
                );
            }
        }
    }
    Ok(themes)
}

fn load(path: &Path, themes_dir: Option<&Path>) -> anyhow::Result<akidev_manga::SourceDef> {
    let value = serde_json::from_str(
        &std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?,
    )
    .with_context(|| format!("{} is not JSON", path.display()))?;
    themes(themes_dir)?.resolve(value)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |args: &mut Vec<String>, name: &str| -> Option<String> {
        let at = args.iter().position(|arg| arg == name)?;
        args.remove(at);
        (at < args.len()).then(|| args.remove(at))
    };
    let themes_dir = flag(&mut args, "--themes").map(PathBuf::from);
    let key = flag(&mut args, "--key");
    let bless = args
        .iter()
        .position(|arg| arg == "--bless")
        .map(|at| args.remove(at))
        .is_some();
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 3 => {
            let def = load(Path::new(&args[1]), themes_dir.as_deref())?;
            let name = def.name.clone();
            let outcomes = check::run(def, Path::new(&args[2]), bless).await?;
            let failed = outcomes
                .iter()
                .filter(|outcome| outcome.error.is_some())
                .count();
            for outcome in &outcomes {
                match &outcome.error {
                    None => println!("ok    {}", outcome.name),
                    Some(error) => println!("FAIL  {}\n{error}", outcome.name),
                }
            }
            println!("{name}: {} calls, {failed} failed", outcomes.len());
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Some("record") if args.len() >= 4 => {
            let def = load(Path::new(&args[1]), themes_dir.as_deref())?;
            let dir = Path::new(&args[2]);
            let source = Source::recording(def.clone())?;
            let mut calls = Vec::new();
            for call in &args[3..] {
                let (op, arg) = call.split_once('=').unwrap_or((call.as_str(), ""));
                let result = match op {
                    "popular" => source.popular(1).await.map(|_| ()),
                    "latest" => source.latest(1).await.map(|_| ()),
                    "search" => source.search(arg, 1, &Default::default()).await.map(|_| ()),
                    "details" => source.details(arg).await.map(|_| ()),
                    "chapters" => source.chapters(arg).await.map(|_| ()),
                    "pages" => {
                        let (series, chapter) = arg
                            .split_once(',')
                            .with_context(|| "pages=SERIES,CHAPTER")?;
                        source.pages(series, chapter).await.map(|_| ())
                    }
                    other => bail!("unknown call {other}"),
                };
                match result {
                    Ok(()) => {
                        println!("recorded {call}");
                        let entry = match op {
                            "popular" | "latest" => serde_json::json!({ "op": op, "page": 1 }),
                            "search" => serde_json::json!({ "op": op, "term": arg }),
                            "details" | "chapters" => serde_json::json!({ "op": op, "id": arg }),
                            _ => {
                                let (series, chapter) = arg.split_once(',').unwrap_or_default();
                                serde_json::json!({ "op": op, "series": series, "id": chapter })
                            }
                        };
                        calls.push(entry);
                    }
                    Err(error) => println!("SKIPPED {call}: {error:#}"),
                }
            }
            std::fs::create_dir_all(dir)?;
            let mut responses = serde_json::Map::new();
            for (index, (key, body)) in source.fetcher().taken().into_iter().enumerate() {
                let extension = if body.trim_start().starts_with(['{', '[']) {
                    "json"
                } else {
                    "html"
                };
                let file = format!("r{}.{extension}", index + 1);
                std::fs::write(dir.join(&file), &body)?;
                responses.insert(key, serde_json::Value::String(file));
            }
            let fixtures = serde_json::json!({ "now_ms": akidev_core::calendar::now_ms(), "responses": responses, "calls": calls });
            std::fs::write(
                dir.join("fixtures.json"),
                serde_json::to_string_pretty(&fixtures)? + "\n",
            )?;
            let outcomes = check::run(def, dir, true).await?;
            println!(
                "{} calls written as fixtures in {}",
                outcomes.len(),
                dir.display()
            );
            println!("read the expected answers in fixtures.json before committing them");
        }
        Some("keygen") if args.len() == 2 => {
            let (secret, public) = akidev_manga::repo::generate_key()?;
            std::fs::write(&args[1], format!("{secret}\n"))?;
            println!(
                "secret key written to {} (keep it out of the repository)\npublic key: {public}",
                args[1]
            );
        }
        Some("index") if args.len() == 2 => {
            let dir = Path::new(&args[1]);
            let secret = match &key {
                Some(path) => Some(
                    std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?,
                ),
                None => None,
            };
            let public = secret
                .as_deref()
                .map(akidev_manga::repo::public_of)
                .transpose()?;
            let index = akidev_manga::repo::build_index(dir, public)?;
            let text = serde_json::to_string_pretty(&index)? + "\n";
            std::fs::write(dir.join("index.json"), &text)?;
            match secret {
                Some(secret) => std::fs::write(
                    dir.join("index.json.sig"),
                    akidev_manga::repo::sign(text.as_bytes(), &secret)? + "\n",
                )?,
                None => {
                    let _ = std::fs::remove_file(dir.join("index.json.sig"));
                }
            }
            println!(
                "{}: {} sources, {} themes, {} voices{}",
                index.name,
                index.sources.len(),
                index.themes.len(),
                index.voices.len(),
                if key.is_some() {
                    ", signed"
                } else {
                    ", unsigned"
                }
            );
        }
        Some("run") if args.len() >= 3 => {
            let source = Source::live(load(Path::new(&args[1]), themes_dir.as_deref())?)?;
            let out = match (args[2].as_str(), &args[3..]) {
                ("popular", _) => serde_json::to_value(source.popular(1).await?)?,
                ("latest", _) => serde_json::to_value(source.latest(1).await?)?,
                ("search", [term, ..]) => {
                    serde_json::to_value(source.search(term, 1, &Default::default()).await?)?
                }
                ("details", [id, ..]) => serde_json::to_value(source.details(id).await?)?,
                ("chapters", [id, ..]) => serde_json::to_value(source.chapters(id).await?)?,
                ("pages", [series, chapter, ..]) => {
                    serde_json::to_value(source.pages(series, chapter).await?)?
                }
                _ => bail!(
                    "usage: run <definition.json> popular|latest|search TERM|details ID|chapters ID|pages SERIES CHAPTER"
                ),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => bail!(
            "usage:\n  record <definition.json> <fixtures-dir> <call>... [--themes <dir>]\n  index <repo-dir> [--key <secret-file>]\n  keygen <secret-file>\n  check <definition.json> <fixtures-dir> [--themes <dir>] [--bless]\n  run <definition.json> <op> [args] [--themes <dir>]"
        ),
    }
    Ok(())
}
