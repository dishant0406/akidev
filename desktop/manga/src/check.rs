//! Testing a definition against recorded pages: a `fixtures.json` lists the answers a site gave
//! (by request) and the calls to make, with what each call must return.

use crate::fetch::Fetcher;
use crate::source::Source;
use crate::def::SourceDef;
use anyhow::{Context, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Deserialize)]
struct Fixtures {
    /// "GET https://site/path" to the file holding the answer, beside `fixtures.json`.
    responses: BTreeMap<String, String>,
    /// The fixed "now", in milliseconds, so relative dates read the same every run.
    now_ms: i64,
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    op: String,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    term: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    series: Option<String>,
    /// What the call must return; empty on a new call, filled in by `bless`.
    #[serde(default)]
    expect: Option<Value>,
}

pub struct Outcome {
    pub name: String,
    pub error: Option<String>,
}

async fn call(source: &Source, call: &Call) -> anyhow::Result<Value> {
    let page = call.page.unwrap_or(1);
    let id = || call.id.clone().context("this call needs an id");
    Ok(match call.op.as_str() {
        "popular" => serde_json::to_value(source.popular(page).await?)?,
        "latest" => serde_json::to_value(source.latest(page).await?)?,
        "search" => serde_json::to_value(source.search(call.term.as_deref().context("search needs a term")?, page, &BTreeMap::new()).await?)?,
        "details" => serde_json::to_value(source.details(&id()?).await?)?,
        "chapters" => serde_json::to_value(source.chapters(&id()?).await?)?,
        "pages" => serde_json::to_value(source.pages(call.series.as_deref().unwrap_or_default(), &id()?).await?)?,
        other => bail!("unknown op {other}"),
    })
}

/// Runs every call in `dir/fixtures.json` against `def`. With `bless`, a call with nothing expected
/// (or a wrong answer) is rewritten to what the definition returns.
pub async fn run(def: SourceDef, dir: &Path, bless: bool) -> anyhow::Result<Vec<Outcome>> {
    let path = dir.join("fixtures.json");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let fixtures: Fixtures = serde_json::from_str(&raw).with_context(|| format!("{} is not a valid fixtures file", path.display()))?;
    let mut answers = HashMap::new();
    for (key, file) in &fixtures.responses {
        answers.insert(key.clone(), std::fs::read_to_string(dir.join(file)).with_context(|| format!("cannot read {file}"))?);
    }
    let source = Source::new(def, Fetcher::recorded(answers))?.at(fixtures.now_ms);
    let mut outcomes = Vec::new();
    let mut document: Value = serde_json::from_str(&raw)?;
    for (index, wanted) in fixtures.calls.iter().enumerate() {
        let name = format!("{} {}", wanted.op, wanted.id.as_deref().or(wanted.term.as_deref()).unwrap_or(""));
        let name = name.trim().to_string();
        match call(&source, wanted).await {
            Err(error) => outcomes.push(Outcome { name, error: Some(format!("{error:#}")) }),
            Ok(got) => {
                if wanted.expect.as_ref() == Some(&got) {
                    outcomes.push(Outcome { name, error: None });
                } else if bless {
                    document["calls"][index]["expect"] = got;
                    outcomes.push(Outcome { name: format!("{name} (written)"), error: None });
                } else {
                    let want = serde_json::to_string_pretty(&wanted.expect.clone().unwrap_or(json!(null)))?;
                    let have = serde_json::to_string_pretty(&got)?;
                    outcomes.push(Outcome { name, error: Some(format!("expected\n{want}\ngot\n{have}")) });
                }
            }
        }
    }
    if bless {
        std::fs::write(&path, serde_json::to_string_pretty(&document)? + "\n")?;
    }
    Ok(outcomes)
}
