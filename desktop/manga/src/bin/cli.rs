//! `akidev-manga-cli`: check a source definition against recorded pages, or ask a live site.
//!
//!   check <definition.json> <fixtures-dir> [--themes <dir>] [--bless]
//!   run   <definition.json> <popular|latest|search TERM|details ID|chapters ID|pages SERIES CHAPTER> [--themes <dir>]

use akidev_manga::theme::Themes;
use akidev_manga::{Source, check};
use anyhow::{Context, bail};
use std::path::{Path, PathBuf};

fn themes(dir: Option<&Path>) -> anyhow::Result<Themes> {
    let mut themes = Themes::default();
    if let Some(dir) = dir {
        for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let name = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().to_string();
                themes.insert(&name, serde_json::from_str(&std::fs::read_to_string(&path)?).with_context(|| format!("{} is not JSON", path.display()))?);
            }
        }
    }
    Ok(themes)
}

fn load(path: &Path, themes_dir: Option<&Path>) -> anyhow::Result<akidev_manga::SourceDef> {
    let value = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?)
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
    let bless = args.iter().position(|arg| arg == "--bless").map(|at| args.remove(at)).is_some();
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 3 => {
            let def = load(Path::new(&args[1]), themes_dir.as_deref())?;
            let name = def.name.clone();
            let outcomes = check::run(def, Path::new(&args[2]), bless).await?;
            let failed = outcomes.iter().filter(|outcome| outcome.error.is_some()).count();
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
        Some("run") if args.len() >= 3 => {
            let source = Source::live(load(Path::new(&args[1]), themes_dir.as_deref())?)?;
            let out = match (args[2].as_str(), &args[3..]) {
                ("popular", _) => serde_json::to_value(source.popular(1).await?)?,
                ("latest", _) => serde_json::to_value(source.latest(1).await?)?,
                ("search", [term, ..]) => serde_json::to_value(source.search(term, 1, &Default::default()).await?)?,
                ("details", [id, ..]) => serde_json::to_value(source.details(id).await?)?,
                ("chapters", [id, ..]) => serde_json::to_value(source.chapters(id).await?)?,
                ("pages", [series, chapter, ..]) => serde_json::to_value(source.pages(series, chapter).await?)?,
                _ => bail!("usage: run <definition.json> popular|latest|search TERM|details ID|chapters ID|pages SERIES CHAPTER"),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => bail!("usage:\n  check <definition.json> <fixtures-dir> [--themes <dir>] [--bless]\n  run <definition.json> <op> [args] [--themes <dir>]"),
    }
    Ok(())
}
