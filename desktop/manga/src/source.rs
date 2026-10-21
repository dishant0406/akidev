//! A source: a definition and the means to ask its site the six questions every reader asks.

use crate::date;
use crate::def::{ListDef, Order, Request, Role, SourceDef, SCHEMA};
use crate::eval::{self, Ctx, check_ex, eval, eval_text, eval_texts};
use crate::fetch::{Fetcher, Req};
use crate::model::{Card, Chapter, Detail, Page, PageList, Reading, Status};
use crate::template::fill;
use anyhow::{Context, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;

/// What a request template's holes are filled with.
#[derive(Default, Clone)]
pub struct Holes {
    pub page: u32,
    pub term: Option<String>,
    pub id: Option<String>,
    pub series: Option<String>,
    /// The filters the person chose, by filter id.
    pub filters: BTreeMap<String, String>,
}

impl Holes {
    fn lookup(&self, name: &str) -> Option<String> {
        match name {
            "page" => Some(self.page.max(1).to_string()),
            "page0" => Some(self.page.max(1).saturating_sub(1).to_string()),
            "term" => self.term.clone(),
            "id" => self.id.clone(),
            "series" => self.series.clone(),
            other => self.filters.get(other).cloned(),
        }
    }
}

pub struct Source {
    def: SourceDef,
    fetcher: Fetcher,
    now_ms: Option<i64>,
}

fn is_http(address: &str) -> bool {
    url::Url::parse(address).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

impl Source {
    pub fn new(def: SourceDef, fetcher: Fetcher) -> anyhow::Result<Source> {
        validate(&def)?;
        Ok(Source { def, fetcher, now_ms: None })
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

    fn now(&self) -> i64 {
        self.now_ms.unwrap_or_else(akidev_core::calendar::now_ms)
    }

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

    async fn list(&self, list: Option<&ListDef>, what: &str, holes: Holes) -> anyhow::Result<Page<Card>> {
        let list = list.with_context(|| format!("{} has no {what}", self.def.name))?;
        let (body, host) = self.ask(&list.request, &holes).await?;
        self.read_list(list, &body, host, &holes)
    }

    fn read_list(&self, list: &ListDef, body: &str, host: &str, holes: &Holes) -> anyhow::Result<Page<Card>> {
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx { base: host, hole: &lookup, now_ms: self.now() };
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
        let has_next = match &list.next {
            Some(next) => !eval(next, Role::Field, list.format, &root, &ctx)?.is_empty(),
            None => !rows.is_empty(),
        };
        Ok(Page { items, has_next })
    }

    pub async fn popular(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list(self.def.popular.as_ref(), "popular list", Holes { page, ..Holes::default() }).await
    }

    pub async fn latest(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list(self.def.latest.as_ref(), "latest list", Holes { page, ..Holes::default() }).await
    }

    pub async fn search(&self, term: &str, page: u32, filters: &BTreeMap<String, String>) -> anyhow::Result<Page<Card>> {
        let holes = Holes { page, term: Some(term.to_string()), filters: filters.clone(), ..Holes::default() };
        self.list(self.def.search.as_ref(), "search", holes).await
    }

    pub async fn details(&self, id: &str) -> anyhow::Result<Detail> {
        let def = &self.def.detail;
        let holes = Holes { id: Some(id.to_string()), ..Holes::default() };
        let (body, host) = self.ask(&def.request, &holes).await?;
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx { base: host, hole: &lookup, now_ms: self.now() };
        let root = eval::parse_root(&body, def.format)?;
        let one = |ex: &Option<crate::def::Ex>| -> anyhow::Result<Option<String>> {
            match ex {
                Some(ex) => eval_text(ex, def.format, &root, &ctx),
                None => Ok(None),
            }
        };
        let many = |ex: &Option<crate::def::Ex>| -> anyhow::Result<Vec<String>> {
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
        let holes = Holes { id: Some(id.to_string()), series: Some(id.to_string()), ..Holes::default() };
        let (body, host) = self.ask(&def.request, &holes).await?;
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx { base: host, hole: &lookup, now_ms: self.now() };
        let root = eval::parse_root(&body, def.format)?;
        let rows = eval(&def.rows, Role::Rows, def.format, &root, &ctx)?;
        let mut chapters: Vec<Chapter> = Vec::new();
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
        if def.order == Order::OldestFirst {
            chapters.reverse();
        }
        Ok(chapters)
    }

    pub async fn pages(&self, series: &str, chapter: &str) -> anyhow::Result<PageList> {
        let def = &self.def.pages;
        let holes = Holes { id: Some(chapter.to_string()), series: Some(series.to_string()), ..Holes::default() };
        let (body, host) = self.ask(&def.request, &holes).await?;
        let lookup = |name: &str| holes.lookup(name);
        let ctx = Ctx { base: host, hole: &lookup, now_ms: self.now() };
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
        let image_lookup = |name: &str| if name == "base" { Some(base.clone()) } else { None };
        headers.insert("referer".to_string(), format!("{base}/"));
        for (name, value) in &def.image_headers {
            if let Some(value) = fill(value, &image_lookup, false) {
                headers.insert(name.to_lowercase(), value);
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
    }
    let detail = &def.detail;
    for (name, ex) in [("title", Some(&detail.title)), ("cover", detail.cover.as_ref()), ("description", detail.description.as_ref()), ("authors", detail.authors.as_ref()), ("tags", detail.tags.as_ref()), ("status", detail.status.as_ref())] {
        if let Some(ex) = ex {
            check_ex(ex, Role::Field, detail.format).with_context(|| format!("detail.{name}"))?;
        }
    }
    let chapters = &def.chapters;
    check_ex(&chapters.rows, Role::Rows, chapters.format).context("chapters.rows")?;
    check_ex(&chapters.chapter.id, Role::Field, chapters.format).context("chapters.chapter.id")?;
    for (name, ex) in [("number", &chapters.chapter.number), ("title", &chapters.chapter.title), ("date", &chapters.chapter.date), ("scanlator", &chapters.chapter.scanlator)] {
        if let Some(ex) = ex {
            check_ex(ex, Role::Field, chapters.format).with_context(|| format!("chapters.chapter.{name}"))?;
        }
    }
    check_ex(&def.pages.images, Role::Field, def.pages.format).context("pages.images")?;
    if let Some(reading) = &def.pages.reading {
        ensure!(matches!(reading.as_str(), "longstrip" | "paged" | "paged_rtl"), "pages.reading must be longstrip, paged or paged_rtl");
    }
    Ok(())
}
