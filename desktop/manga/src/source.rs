//! A source: a definition and the means to ask its site the six questions every reader asks.

use crate::date;
use crate::def::{Ex, Format, ListDef, Order, Prepare, Request, Role, SourceDef, SCHEMA};
use crate::eval::{self, Ctx, check_ex, eval, eval_text, eval_texts};
use crate::fetch::{Fetcher, Req};
use crate::model::{Card, Chapter, Detail, Page, PageList, Reading, Status};
use crate::template::fill;
use anyhow::{Context, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::RwLock;

/// What a request template's holes are filled with.
#[derive(Default, Clone)]
pub struct Holes {
    pub page: u32,
    pub page_size: u32,
    /// Set while a source's chapter list is asked for in several goes.
    pub offset: Option<u32>,
    pub term: Option<String>,
    pub id: Option<String>,
    pub series: Option<String>,
    /// Values read from earlier answers (`prepare` and `vars`).
    pub vars: BTreeMap<String, String>,
    /// The filters the person chose, by filter id.
    pub filters: BTreeMap<String, String>,
    /// The source's settings: the person's choice, or the default.
    pub settings: BTreeMap<String, String>,
}

impl Holes {
    fn lookup(&self, name: &str) -> Option<String> {
        match name {
            "page" => Some(self.page.max(1).to_string()),
            "page0" => Some(self.page.max(1).saturating_sub(1).to_string()),
            "offset" => Some(self.offset.unwrap_or_else(|| self.page.max(1).saturating_sub(1) * self.page_size.max(1)).to_string()),
            "term" => self.term.clone(),
            "id" => self.id.clone(),
            "series" => self.series.clone(),
            other => self.vars.get(other).or_else(|| self.filters.get(other)).or_else(|| self.settings.get(other)).cloned(),
        }
    }
}

/// What an app needs to list a source before it asks it anything.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub nsfw: bool,
    pub version: u64,
    pub can_popular: bool,
    pub can_latest: bool,
    pub can_search: bool,
    pub has_settings: bool,
}

/// A setting as an app shows it: the choices and what is chosen now.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingState {
    pub id: String,
    pub name: String,
    pub options: Vec<(String, String)>,
    pub value: String,
}

pub struct Source {
    def: SourceDef,
    fetcher: Fetcher,
    now_ms: Option<i64>,
    /// The person's choices, by setting id.
    chosen: RwLock<BTreeMap<String, String>>,
}

fn is_http(address: &str) -> bool {
    url::Url::parse(address).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

impl Source {
    pub fn new(def: SourceDef, fetcher: Fetcher) -> anyhow::Result<Source> {
        validate(&def)?;
        if let Some(agent) = &def.user_agent {
            fetcher.set_user_agent(agent);
        }
        Ok(Source { def, fetcher, now_ms: None, chosen: RwLock::default() })
    }

    pub fn live(def: SourceDef) -> anyhow::Result<Source> {
        let fetcher = Fetcher::live(def.rate_per_second);
        Source::new(def, fetcher)
    }

    /// Fixes "now", for tests of relative dates.
    pub fn at(mut self, now_ms: i64) -> Source {
        self.now_ms = Some(now_ms);
        self
    }

    pub fn def(&self) -> &SourceDef {
        &self.def
    }

    pub fn fetcher(&self) -> &Fetcher {
        &self.fetcher
    }

    pub fn info(&self) -> Info {
        let def = &self.def;
        Info {
            id: def.id.clone(),
            name: def.name.clone(),
            languages: def.lang.clone(),
            nsfw: def.nsfw,
            version: def.version,
            can_popular: def.popular.is_some(),
            can_latest: def.latest.is_some(),
            can_search: def.search.is_some(),
            has_settings: !def.settings.is_empty(),
        }
    }

    // --- settings ------------------------------------------------------------------------------

    fn effective_settings(&self) -> BTreeMap<String, String> {
        let chosen = self.chosen.read().unwrap();
        self.def.settings.iter().map(|setting| (setting.id.clone(), chosen.get(&setting.id).cloned().unwrap_or_else(|| setting.default.clone()))).collect()
    }

    pub fn settings(&self) -> Vec<SettingState> {
        let value = self.effective_settings();
        self.def
            .settings
            .iter()
            .map(|setting| SettingState {
                id: setting.id.clone(),
                name: setting.name.clone(),
                options: setting.options.iter().map(|option| (option.value.clone(), option.name.clone())).collect(),
                value: value[&setting.id].clone(),
            })
            .collect()
    }

    /// Chooses one of a setting's options.
    pub fn choose(&self, id: &str, value: &str) -> anyhow::Result<()> {
        let setting = self.def.settings.iter().find(|setting| setting.id == id).with_context(|| format!("{} has no setting {id}", self.def.name))?;
        ensure!(setting.options.iter().any(|option| option.value == value), "{value} is not one of the choices for {}", setting.name);
        self.chosen.write().unwrap().insert(id.to_string(), value.to_string());
        Ok(())
    }

    fn holes(&self) -> Holes {
        Holes { page_size: self.def.page_size, settings: self.effective_settings(), ..Holes::default() }
    }

    fn now(&self) -> i64 {
        self.now_ms.unwrap_or_else(akidev_core::calendar::now_ms)
    }

    // --- asking --------------------------------------------------------------------------------

    fn hosts(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.def.base_url.as_str()).chain(self.def.mirrors.iter().map(String::as_str))
    }

    /// The request for one host, or `None` when a hole it needs has nothing.
    fn build(&self, request: &Request, host: &str, holes: &Holes) -> Option<Req> {
        let host = host.trim_end_matches('/');
        let lookup = |name: &str| if name == "base" { Some(host.to_string()) } else { holes.lookup(name) };
        let rest = request.url.strip_prefix("{base}")?;
        let url = format!("{host}{}", fill(rest, &lookup, true)?);
        let text = |value: &str| fill(value, &lookup, false);
        let mut headers: Vec<(String, String)> = Vec::new();
        for (name, value) in self.def.headers.iter().chain(&request.headers) {
            headers.retain(|(have, _)| !have.eq_ignore_ascii_case(name));
            headers.push((name.clone(), text(value)?));
        }
        let mut form = Vec::new();
        for (name, value) in &request.form {
            // A form field with nothing to fill it is left out, not sent empty.
            if let Some(value) = text(value) {
                form.push((name.clone(), value));
            }
        }
        let json = match &request.json {
            Some(json) => Some(fill_json(json, &lookup)?),
            None => None,
        };
        Some(Req { method: request.method.to_uppercase(), url, headers, form, json })
    }

    /// Asks the site, trying each mirror when one cannot be reached. A site that answers with an
    /// error or a check is not retried elsewhere.
    async fn ask(&self, request: &Request, holes: &Holes) -> anyhow::Result<(String, &str)> {
        let mut last = None;
        for host in self.hosts() {
            let Some(req) = self.build(request, host, holes) else {
                bail!("the request needs a value this call does not have: {}", request.url);
            };
            match self.fetcher.send(&req).await {
                Ok(body) => return Ok((body, host)),
                Err(error) if error.to_string().starts_with("could not reach") => last = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no address to ask")))
    }

    /// Values an answer gives, as `vars` say.
    fn read_vars(&self, vars: &BTreeMap<String, Ex>, format: Format, body: &str, host: &str, holes: &Holes) -> anyhow::Result<Vec<(String, String)>> {
        if vars.is_empty() {
            return Ok(Vec::new());
        }
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx::new(host, &lookup, self.now());
        let root = eval::parse_root(body, format)?;
        let mut found = Vec::new();
        for (name, ex) in vars {
            if let Some(value) = eval_text(ex, format, &root, &ctx)? {
                found.push((name.clone(), value));
            }
        }
        Ok(found)
    }

    /// Makes the requests that come before a section's own, keeping what they say as holes.
    async fn prepare(&self, steps: &[Prepare], holes: &mut Holes) -> anyhow::Result<()> {
        for step in steps {
            let (body, host) = self.ask(&step.request, holes).await?;
            let found = self.read_vars(&step.vars, step.format, &body, host, holes)?;
            holes.vars.extend(found);
        }
        Ok(())
    }

    // --- lists ---------------------------------------------------------------------------------

    async fn list(&self, list: Option<&ListDef>, what: &str, mut holes: Holes) -> anyhow::Result<Page<Card>> {
        let list = list.with_context(|| format!("{} has no {what}", self.def.name))?;
        self.prepare(&list.prepare, &mut holes).await?;
        let (body, host) = self.ask(&list.request, &holes).await?;
        self.read_list(list, &body, host, holes)
    }

    fn read_list(&self, list: &ListDef, body: &str, host: &str, mut holes: Holes) -> anyhow::Result<Page<Card>> {
        let found = self.read_vars(&list.vars, list.format, body, host, &holes)?;
        holes.vars.extend(found);
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx::new(host, &lookup, self.now());
        let root = eval::parse_root(body, list.format)?;
        let rows = eval(&list.rows, Role::Rows, list.format, &root, &ctx)?;
        let mut items = Vec::new();
        for row in &rows {
            let id = eval_text(&list.card.id, list.format, row, &ctx)?;
            let title = eval_text(&list.card.title, list.format, row, &ctx)?;
            let (Some(id), Some(title)) = (id, title) else { continue };
            let cover = match &list.card.cover {
                Some(cover) => eval_text(cover, list.format, row, &ctx)?.and_then(|link| absolute(host, &link)),
                None => None,
            };
            if !items.iter().any(|have: &Card| have.id == id) {
                items.push(Card { id, title, cover });
            }
        }
        let has_next = if let Some(total) = &list.total {
            let total = eval_text(total, list.format, &root, &ctx)?.and_then(|text| text.parse::<f64>().ok()).unwrap_or(0.0);
            let size = holes.page_size.max(1);
            f64::from(holes.page.max(1) * size) < total
        } else if let Some(next) = &list.next {
            !eval(next, Role::Field, list.format, &root, &ctx)?.is_empty()
        } else {
            !rows.is_empty()
        };
        Ok(Page { items, has_next })
    }

    pub async fn popular(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list(self.def.popular.as_ref(), "popular list", Holes { page, ..self.holes() }).await
    }

    pub async fn latest(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list(self.def.latest.as_ref(), "latest list", Holes { page, ..self.holes() }).await
    }

    pub async fn search(&self, term: &str, page: u32, filters: &BTreeMap<String, String>) -> anyhow::Result<Page<Card>> {
        let holes = Holes { page, term: Some(term.to_string()), filters: filters.clone(), ..self.holes() };
        self.list(self.def.search.as_ref(), "search", holes).await
    }

    // --- one series ----------------------------------------------------------------------------

    pub async fn details(&self, id: &str) -> anyhow::Result<Detail> {
        let def = &self.def.detail;
        let mut holes = Holes { id: Some(id.to_string()), ..self.holes() };
        self.prepare(&def.prepare, &mut holes).await?;
        let (body, host) = self.ask(&def.request, &holes).await?;
        let found = self.read_vars(&def.vars, def.format, &body, host, &holes)?;
        holes.vars.extend(found);
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx::new(host, &lookup, self.now());
        let root = eval::parse_root(&body, def.format)?;
        let one = |ex: &Option<Ex>| -> anyhow::Result<Option<String>> {
            match ex {
                Some(ex) => eval_text(ex, def.format, &root, &ctx),
                None => Ok(None),
            }
        };
        let many = |ex: &Option<Ex>| -> anyhow::Result<Vec<String>> {
            match ex {
                Some(ex) => eval_texts(ex, def.format, &root, &ctx),
                None => Ok(Vec::new()),
            }
        };
        let title = eval_text(&def.title, def.format, &root, &ctx)?.context("the page has no title: has the site changed?")?;
        let status = one(&def.status)?.map_or(Status::Unknown, |text| Status::parse(&text));
        Ok(Detail {
            title,
            cover: one(&def.cover)?.and_then(|link| absolute(host, &link)),
            description: one(&def.description)?,
            authors: many(&def.authors)?,
            tags: many(&def.tags)?,
            status,
        })
    }

    /// Every chapter, newest first whatever order the site lists them in.
    pub async fn chapters(&self, id: &str) -> anyhow::Result<Vec<Chapter>> {
        let def = &self.def.chapters;
        let mut holes = Holes { id: Some(id.to_string()), series: Some(id.to_string()), ..self.holes() };
        self.prepare(&def.prepare, &mut holes).await?;
        let mut chapters: Vec<Chapter> = Vec::new();
        let mut offset = 0u32;
        // A source that pages its list is asked again until it has given them all (and never without end).
        for _ in 0..60 {
            holes.offset = def.repeat.as_ref().map(|_| offset);
            let (body, host) = self.ask(&def.request, &holes).await?;
            let (rows, total) = self.read_chapters(&body, host, holes.clone(), &mut chapters)?;
            let Some(repeat) = &def.repeat else { break };
            offset += repeat.size;
            if rows == 0 || total.is_none_or(|total| u64::from(offset) >= total) {
                break;
            }
        }
        if def.order == Order::OldestFirst {
            chapters.reverse();
        }
        Ok(chapters)
    }

    /// Adds the chapters in one answer; says how many rows it had and the total, if the source gives one.
    fn read_chapters(&self, body: &str, host: &str, mut holes: Holes, chapters: &mut Vec<Chapter>) -> anyhow::Result<(usize, Option<u64>)> {
        let def = &self.def.chapters;
        let found = self.read_vars(&def.vars, def.format, body, host, &holes)?;
        holes.vars.extend(found);
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx::new(host, &lookup, self.now());
        let root = eval::parse_root(body, def.format)?;
        let rows = eval(&def.rows, Role::Rows, def.format, &root, &ctx)?;
        let total = match &def.repeat {
            Some(repeat) => eval_text(&repeat.total, def.format, &root, &ctx)?.and_then(|text| text.parse::<u64>().ok()),
            None => None,
        };
        for row in &rows {
            let Some(chapter_id) = eval_text(&def.chapter.id, def.format, row, &ctx)? else { continue };
            let title = match &def.chapter.title {
                Some(ex) => eval_text(ex, def.format, row, &ctx)?,
                None => None,
            };
            let number = match &def.chapter.number {
                Some(ex) => eval_text(ex, def.format, row, &ctx)?.and_then(|text| text.parse::<f64>().ok().or_else(|| date::chapter_number(&text))),
                None => None,
            }
            .or_else(|| title.as_deref().and_then(date::chapter_number));
            let date_ms = match &def.chapter.date {
                Some(ex) => eval_text(ex, def.format, row, &ctx)?.and_then(|text| text.parse::<i64>().ok()),
                None => None,
            };
            let scanlator = match &def.chapter.scanlator {
                Some(ex) => eval_text(ex, def.format, row, &ctx)?,
                None => None,
            };
            if !chapters.iter().any(|have| have.id == chapter_id) {
                chapters.push(Chapter { id: chapter_id, number, title, date_ms, scanlator });
            }
        }
        Ok((rows.len(), total))
    }

    pub async fn pages(&self, series: &str, chapter: &str) -> anyhow::Result<PageList> {
        let def = &self.def.pages;
        let mut holes = Holes { id: Some(chapter.to_string()), series: Some(series.to_string()), ..self.holes() };
        self.prepare(&def.prepare, &mut holes).await?;
        let (body, host) = self.ask(&def.request, &holes).await?;
        let found = self.read_vars(&def.vars, def.format, &body, host, &holes)?;
        holes.vars.extend(found);
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx::new(host, &lookup, self.now());
        let root = eval::parse_root(&body, def.format)?;
        let images: Vec<String> = eval_texts(&def.images, def.format, &root, &ctx)?.iter().filter_map(|link| absolute(host, link)).collect();
        ensure!(!images.is_empty(), "the chapter has no pages: has the site changed?");
        let reading = match def.reading.as_deref() {
            Some("longstrip") => Reading::LongStrip,
            Some("paged") => Reading::Paged,
            Some("paged_rtl") => Reading::PagedRtl,
            _ => Reading::Unknown,
        };
        let mut headers = BTreeMap::new();
        let base = host.trim_end_matches('/').to_string();
        let image_lookup = |name: &str| if name == "base" { Some(base.clone()) } else { holes.lookup(name) };
        headers.insert("referer".to_string(), format!("{base}/"));
        for (name, value) in &def.image_headers {
            match fill(value, &image_lookup, false) {
                Some(value) if !value.is_empty() => {
                    headers.insert(name.to_lowercase(), value);
                }
                // An empty header is how a definition says "none": a referer some image servers refuse.
                _ => {
                    headers.remove(&name.to_lowercase());
                }
            }
        }
        Ok(PageList { images, reading, headers })
    }
}

fn absolute(base: &str, link: &str) -> Option<String> {
    let link = link.trim();
    if link.is_empty() || link.starts_with("data:") {
        return None;
    }
    url::Url::parse(base).ok()?.join(link).ok().map(|joined| joined.to_string())
}

fn fill_json(value: &Value, lookup: &dyn Fn(&str) -> Option<String>) -> Option<Value> {
    Some(match value {
        Value::String(text) => Value::String(fill(text, lookup, false)?),
        Value::Array(items) => Value::Array(items.iter().map(|item| fill_json(item, lookup)).collect::<Option<_>>()?),
        Value::Object(map) => Value::Object(map.iter().filter_map(|(key, value)| Some((key.clone(), fill_json(value, lookup)?))).collect()),
        other => other.clone(),
    })
}

fn plain(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn check_vars(vars: &BTreeMap<String, Ex>, format: Format, what: &str) -> anyhow::Result<()> {
    for (name, ex) in vars {
        ensure!(plain(name), "{what}: a var's name is lowercase letters, digits, dashes and underscores");
        check_ex(ex, Role::Field, format).with_context(|| format!("{what}.vars.{name}"))?;
    }
    Ok(())
}

fn check_prepare(steps: &[Prepare], what: &str) -> anyhow::Result<()> {
    for (index, step) in steps.iter().enumerate() {
        ensure!(step.request.url.starts_with("{base}"), "{what}.prepare[{index}]: a request's url must start with {{base}}");
        ensure!(matches!(step.request.method.to_uppercase().as_str(), "GET" | "POST"), "{what}.prepare[{index}]: the method must be GET or POST");
        check_vars(&step.vars, step.format, &format!("{what}.prepare[{index}]"))?;
    }
    Ok(())
}

/// Rejects a definition that can be known to be wrong without asking its site.
pub fn validate(def: &SourceDef) -> anyhow::Result<()> {
    ensure!(def.schema == SCHEMA, "schema {} is not one this app reads (it reads {SCHEMA})", def.schema);
    ensure!(!def.id.is_empty() && def.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "the id must be lowercase letters, digits and dashes");
    ensure!(is_http(&def.base_url), "base_url must be an http(s) address");
    for mirror in &def.mirrors {
        ensure!(is_http(mirror), "mirror {mirror} must be an http(s) address");
    }
    if let Some(rate) = def.rate_per_second {
        ensure!(rate > 0.0 && rate <= 20.0, "rate_per_second must be above 0 and at most 20");
    }
    ensure!((1..=500).contains(&def.page_size), "page_size must be from 1 to 500");
    for setting in &def.settings {
        ensure!(plain(&setting.id), "a setting's id is lowercase letters, digits, dashes and underscores");
        ensure!(setting.options.iter().any(|option| option.value == setting.default), "setting {}: the default is not one of its options", setting.id);
    }
    let mut requests: Vec<(&str, &Request)> = vec![("detail", &def.detail.request), ("chapters", &def.chapters.request), ("pages", &def.pages.request)];
    let mut lists: Vec<(&str, &ListDef)> = Vec::new();
    for (name, list) in [("popular", &def.popular), ("latest", &def.latest), ("search", &def.search)] {
        if let Some(list) = list {
            lists.push((name, list));
        }
    }
    for (name, list) in &lists {
        requests.push((name, &list.request));
    }
    ensure!(!lists.is_empty(), "a source needs at least one of popular, latest and search");
    for (name, request) in requests {
        ensure!(request.url.starts_with("{base}"), "{name}: a request's url must start with {{base}} (a source can only ask its own address)");
        ensure!(matches!(request.method.to_uppercase().as_str(), "GET" | "POST"), "{name}: the method must be GET or POST");
    }
    for (name, list) in lists {
        let format = list.format;
        check_ex(&list.rows, Role::Rows, format).with_context(|| format!("{name}.rows"))?;
        check_ex(&list.card.id, Role::Field, format).with_context(|| format!("{name}.card.id"))?;
        check_ex(&list.card.title, Role::Field, format).with_context(|| format!("{name}.card.title"))?;
        if let Some(cover) = &list.card.cover {
            check_ex(cover, Role::Field, format).with_context(|| format!("{name}.card.cover"))?;
        }
        if let Some(next) = &list.next {
            check_ex(next, Role::Field, format).with_context(|| format!("{name}.next"))?;
        }
        if let Some(total) = &list.total {
            check_ex(total, Role::Field, format).with_context(|| format!("{name}.total"))?;
        }
        check_prepare(&list.prepare, name)?;
        check_vars(&list.vars, format, name)?;
    }
    let detail = &def.detail;
    for (name, ex) in [("title", Some(&detail.title)), ("cover", detail.cover.as_ref()), ("description", detail.description.as_ref()), ("authors", detail.authors.as_ref()), ("tags", detail.tags.as_ref()), ("status", detail.status.as_ref())] {
        if let Some(ex) = ex {
            check_ex(ex, Role::Field, detail.format).with_context(|| format!("detail.{name}"))?;
        }
    }
    check_prepare(&detail.prepare, "detail")?;
    check_vars(&detail.vars, detail.format, "detail")?;
    let chapters = &def.chapters;
    check_ex(&chapters.rows, Role::Rows, chapters.format).context("chapters.rows")?;
    check_ex(&chapters.chapter.id, Role::Field, chapters.format).context("chapters.chapter.id")?;
    for (name, ex) in [("number", &chapters.chapter.number), ("title", &chapters.chapter.title), ("date", &chapters.chapter.date), ("scanlator", &chapters.chapter.scanlator)] {
        if let Some(ex) = ex {
            check_ex(ex, Role::Field, chapters.format).with_context(|| format!("chapters.chapter.{name}"))?;
        }
    }
    if let Some(repeat) = &chapters.repeat {
        ensure!((1..=1000).contains(&repeat.size), "chapters.repeat.size must be from 1 to 1000");
        check_ex(&repeat.total, Role::Field, chapters.format).context("chapters.repeat.total")?;
    }
    check_prepare(&chapters.prepare, "chapters")?;
    check_vars(&chapters.vars, chapters.format, "chapters")?;
    check_ex(&def.pages.images, Role::Field, def.pages.format).context("pages.images")?;
    check_prepare(&def.pages.prepare, "pages")?;
    check_vars(&def.pages.vars, def.pages.format, "pages")?;
    if let Some(reading) = &def.pages.reading {
        ensure!(matches!(reading.as_str(), "longstrip" | "paged" | "paged_rtl"), "pages.reading must be longstrip, paged or paged_rtl");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn source(extra: Value, answers: &[(&str, &str)]) -> Source {
        let mut def = json!({
            "schema": 1, "id": "t", "name": "T", "version": 1, "base_url": "https://t.test",
            "settings": [{"id": "lang", "name": "Language", "default": "en", "options": [{"value": "en", "name": "English"}, {"value": "fr", "name": "French"}]}],
            "popular": {"request": {"url": "{base}/list?l={lang}"}, "format": "json", "rows": "items", "card": {"id": "id", "title": "name"}},
            "detail": {"request": {"url": "{base}/d/{id}"}, "format": "json", "title": "t"},
            "chapters": {"request": {"url": "{base}/c/{id}"}, "format": "json", "rows": "rows", "chapter": {"id": "id"}},
            "pages": {"request": {"url": "{base}/p/{id}"}, "format": "json", "images": "images"}
        });
        for (key, value) in extra.as_object().unwrap() {
            def[key] = value.clone();
        }
        let answers: HashMap<String, String> = answers.iter().map(|(url, body)| (format!("GET https://t.test{url}"), body.to_string())).collect();
        Source::new(serde_json::from_value(def).unwrap(), Fetcher::recorded(answers)).unwrap()
    }

    #[tokio::test]
    async fn a_paged_chapter_list_is_asked_again_until_the_total_is_reached() {
        let chapters = json!({
            "request": {"url": "{base}/c/{id}?o={offset}"}, "format": "json", "rows": "rows", "chapter": {"id": "id", "number": "n"},
            "repeat": {"size": 2, "total": "total"}
        });
        let source = source(json!({"chapters": chapters}), &[
            ("/c/s?o=0", r#"{"total":3,"rows":[{"id":"a","n":3},{"id":"b","n":2}]}"#),
            ("/c/s?o=2", r#"{"total":3,"rows":[{"id":"c","n":1}]}"#),
        ]);
        let list = source.chapters("s").await.unwrap();
        assert_eq!(list.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(source.fetcher().seen(), ["GET https://t.test/c/s?o=0", "GET https://t.test/c/s?o=2"]);
    }

    #[tokio::test]
    async fn a_prepare_request_gives_values_the_next_request_uses() {
        let pages = json!({
            "prepare": [{"request": {"url": "{base}/token"}, "format": "json", "vars": {"token": "t"}}],
            "request": {"url": "{base}/p/{id}?k={token}"}, "format": "json",
            "vars": {"cdn": "cdn"},
            "images": [{"path": "images"}, {"template": "{cdn}/{value}"}]
        });
        let source = source(json!({"pages": pages}), &[
            ("/token", r#"{"t":"abc"}"#),
            ("/p/c1?k=abc", r#"{"cdn":"https://img.test","images":["1.png","2.png"]}"#),
        ]);
        let list = source.pages("s", "c1").await.unwrap();
        assert_eq!(list.images, ["https://img.test/1.png", "https://img.test/2.png"]);
    }

    #[tokio::test]
    async fn settings_fill_holes_and_only_take_their_own_options() {
        let source = source(json!({}), &[
            ("/list?l=en", r#"{"items":[{"id":"1","name":"English"}]}"#),
            ("/list?l=fr", r#"{"items":[{"id":"2","name":"Francais"}]}"#),
        ]);
        assert_eq!(source.popular(1).await.unwrap().items[0].title, "English");
        source.choose("lang", "fr").unwrap();
        assert_eq!(source.popular(1).await.unwrap().items[0].title, "Francais");
        assert!(source.choose("lang", "xx").is_err());
        assert!(source.choose("nope", "en").is_err());
        assert_eq!(source.settings()[0].value, "fr");
    }

    #[tokio::test]
    async fn the_total_decides_whether_another_page_follows() {
        let list = json!({"request": {"url": "{base}/list?o={offset}"}, "format": "json", "rows": "items", "total": "total", "card": {"id": "id", "title": "name"}});
        let source = source(json!({"popular": list, "page_size": 2}), &[
            ("/list?o=0", r#"{"total":3,"items":[{"id":"1","name":"a"},{"id":"2","name":"b"}]}"#),
            ("/list?o=2", r#"{"total":3,"items":[{"id":"3","name":"c"}]}"#),
        ]);
        assert!(source.popular(1).await.unwrap().has_next);
        assert!(!source.popular(2).await.unwrap().has_next);
    }

    #[test]
    fn a_request_to_anywhere_but_the_source_is_refused() {
        let bad = json!({"request": {"url": "https://evil.test/x"}, "format": "json", "rows": "items", "card": {"id": "id", "title": "name"}});
        let mut def = json!({
            "schema": 1, "id": "t", "name": "T", "version": 1, "base_url": "https://t.test", "popular": bad,
            "detail": {"request": {"url": "{base}/d"}, "title": "t"}, "chapters": {"request": {"url": "{base}/c"}, "rows": "r", "chapter": {"id": "i"}},
            "pages": {"request": {"url": "{base}/p"}, "images": "i"}
        });
        assert!(validate(&serde_json::from_value(def.clone()).unwrap()).is_err());
        def["popular"]["request"]["url"] = json!("{base}/x");
        def["pages"]["prepare"] = json!([{"request": {"url": "https://evil.test/t"}, "vars": {"a": "b"}}]);
        assert!(validate(&serde_json::from_value(def).unwrap()).is_err());
    }
}
