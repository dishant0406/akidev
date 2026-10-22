//! Reading values out of a response, as an `Ex` describes.
//!
//! Everything is a list of nodes: an HTML element, a JSON value or a text. A step turns a list
//! into a list. Evaluation is synchronous and holds the parsed page only for its own duration (an
//! HTML tree is not `Send`, so it never lives across an await).

use crate::def::{AesCbc, Ex, Format, Role, Step};
use akidev_core::expression::{scalar, select};
use anyhow::{Context, bail};
use ego_tree::NodeId;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub enum Node {
    Html(Rc<Html>, NodeId),
    Json(Value),
    Text(String),
}

/// What steps may depend on besides the nodes.
pub struct Ctx<'a> {
    /// The source's address, for `abs`.
    pub base: &'a str,
    /// A request hole: `page`, `term`, `id` or a filter.
    pub hole: &'a dyn Fn(&str) -> Option<String>,
    pub now_ms: i64,
    /// The row an expression is being read from: `{^.path}` in a template reads a value in it.
    pub row: Option<&'a Node>,
}

impl<'a> Ctx<'a> {
    pub fn new(base: &'a str, hole: &'a dyn Fn(&str) -> Option<String>, now_ms: i64) -> Ctx<'a> {
        Ctx { base, hole, now_ms, row: None }
    }
}

pub fn parse_html(body: &str) -> Node {
    let document = Html::parse_document(body);
    let root = document.root_element().id();
    Node::Html(Rc::new(document), root)
}

fn parse_fragment(text: &str) -> Node {
    let document = Html::parse_fragment(text);
    let root = document.root_element().id();
    Node::Html(Rc::new(document), root)
}

pub fn parse_root(body: &str, format: Format) -> anyhow::Result<Node> {
    Ok(match format {
        Format::Html => parse_html(body),
        Format::Json => Node::Json(serde_json::from_str(body).context("the answer is not JSON")?),
    })
}

fn element(document: &Rc<Html>, id: NodeId) -> Option<ElementRef<'_>> {
    document.tree.get(id).and_then(ElementRef::wrap)
}

thread_local! {
    static SELECTORS: RefCell<HashMap<String, Selector>> = RefCell::new(HashMap::new());
}

pub fn selector(text: &str) -> anyhow::Result<Selector> {
    if let Some(found) = SELECTORS.with(|cache| cache.borrow().get(text).cloned()) {
        return Ok(found);
    }
    let parsed = Selector::parse(text).map_err(|error| anyhow::anyhow!("bad selector \"{text}\": {error}"))?;
    SELECTORS.with(|cache| cache.borrow_mut().insert(text.to_string(), parsed.clone()));
    Ok(parsed)
}

pub fn pattern(text: &str) -> anyhow::Result<Regex> {
    akidev_core::expression::pattern(text)
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An element's text: each block (a paragraph, a line break, a list item) on its own line, and
/// runs of spaces squashed, so "<p>One.</p><p>Two.</p>" reads as two lines and not "One.Two.".
fn element_text(element: ElementRef) -> String {
    const BLOCKS: [&str; 16] = ["p", "div", "br", "li", "ul", "ol", "tr", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "section", "article"];
    let mut text = String::new();
    for node in element.descendants() {
        match node.value() {
            scraper::Node::Text(piece) => text.push_str(piece),
            scraper::Node::Element(found) if BLOCKS.contains(&found.name()) => text.push('\n'),
            _ => {}
        }
    }
    text.lines().map(squash).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n")
}

/// A node as text.
pub fn text_of(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Json(Value::String(text)) => text.clone(),
        Node::Json(Value::Null) => String::new(),
        Node::Json(value) => scalar(value).unwrap_or_else(|| value.to_string()),
        Node::Html(document, id) => element(document, *id).map(|e| element_text(e)).unwrap_or_default(),
    }
}

/// The steps an expression stands for.
pub fn steps(ex: &Ex, role: Role, format: Format) -> Vec<Step> {
    match ex {
        Ex::Pipe(steps) => steps.clone(),
        Ex::Short(text) => match (format, role) {
            (Format::Json, _) => vec![Step::Path(text.clone())],
            (Format::Html, Role::Rows) => vec![Step::Css(text.clone())],
            (Format::Html, Role::Field) => match text.rsplit_once('@') {
                Some(("", attribute)) => vec![Step::Attr(attribute.to_string())],
                Some((css, attribute)) => vec![Step::Css(css.to_string()), Step::Attr(attribute.to_string())],
                None => vec![Step::Css(text.clone()), Step::Text],
            },
        },
    }
}

pub fn eval(ex: &Ex, role: Role, format: Format, from: &Node, ctx: &Ctx) -> anyhow::Result<Vec<Node>> {
    let inner = Ctx { base: ctx.base, hole: ctx.hole, now_ms: ctx.now_ms, row: Some(from) };
    run(&steps(ex, role, format), format, vec![from.clone()], &inner)
}

/// The first non-empty text an expression gives.
pub fn eval_text(ex: &Ex, format: Format, from: &Node, ctx: &Ctx) -> anyhow::Result<Option<String>> {
    Ok(eval(ex, Role::Field, format, from, ctx)?.iter().map(|node| text_of(node).trim().to_string()).find(|text| !text.is_empty()))
}

/// Every non-empty text an expression gives.
pub fn eval_texts(ex: &Ex, format: Format, from: &Node, ctx: &Ctx) -> anyhow::Result<Vec<String>> {
    Ok(eval(ex, Role::Field, format, from, ctx)?.iter().map(|node| text_of(node).trim().to_string()).filter(|text| !text.is_empty()).collect())
}

pub fn run(steps: &[Step], format: Format, mut nodes: Vec<Node>, ctx: &Ctx) -> anyhow::Result<Vec<Node>> {
    for step in steps {
        nodes = apply(step, format, nodes, ctx)?;
    }
    Ok(nodes)
}

fn texts(nodes: Vec<Node>) -> impl Iterator<Item = String> {
    nodes.into_iter().map(|node| text_of(&node))
}

fn apply(step: &Step, format: Format, nodes: Vec<Node>, ctx: &Ctx) -> anyhow::Result<Vec<Node>> {
    Ok(match step {
        Step::Css(css) => {
            let selector = selector(css)?;
            let mut found = Vec::new();
            for node in nodes {
                let (document, id) = match node {
                    Node::Html(document, id) => (document, id),
                    // A text, or a JSON string, that holds HTML (an ajax answer, a field).
                    other => match parse_fragment(&text_of(&other)) {
                        Node::Html(document, id) => (document, id),
                        _ => continue,
                    },
                };
                if let Some(root) = element(&document, id) {
                    found.extend(root.select(&selector).map(|hit| Node::Html(document.clone(), hit.id())));
                }
            }
            found
        }
        Step::Attr(names) => nodes
            .iter()
            .filter_map(|node| match node {
                Node::Html(document, id) => {
                    let found = element(document, *id)?;
                    names.split('|').find_map(|name| found.value().attr(name.trim()).filter(|value| !value.trim().is_empty()))
                }
                _ => None,
            })
            .map(|value| Node::Text(value.trim().to_string()))
            .collect(),
        Step::Text => texts(nodes).map(Node::Text).collect(),
        Step::Html => nodes
            .iter()
            .filter_map(|node| match node {
                Node::Html(document, id) => element(document, *id).map(|e| Node::Text(e.inner_html())),
                other => Some(Node::Text(text_of(other))),
            })
            .collect(),
        Step::Path(path) => nodes
            .iter()
            .filter_map(|node| match node {
                Node::Json(value) => Some(select(value, path).into_iter().cloned().map(Node::Json).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect(),
        Step::Regex(text) => {
            let regex = pattern(text)?;
            texts(nodes)
                .filter_map(|text| regex.captures(&text).map(|found| found.get(1).unwrap_or_else(|| found.get(0).unwrap()).as_str().to_string()))
                .map(Node::Text)
                .collect()
        }
        Step::Replace { from, to } => {
            let regex = pattern(from)?;
            texts(nodes).map(|text| Node::Text(regex.replace_all(&text, to.as_str()).into_owned())).collect()
        }
        Step::Trim => texts(nodes).map(|text| Node::Text(text.trim().to_string())).collect(),
        Step::Lower => texts(nodes).map(|text| Node::Text(text.to_lowercase())).collect(),
        Step::Template(template) => texts(nodes)
            .filter_map(|value| {
                let hole = |name: &str| match name {
                    "value" => Some(value.clone()),
                    other => match other.strip_prefix("^.") {
                        Some(path) => match ctx.row {
                            Some(Node::Json(row)) => select(row, path).first().and_then(|found| scalar(found)),
                            _ => None,
                        },
                        None => (ctx.hole)(other),
                    },
                };
                crate::template::fill(template, &hole, false)
            })
            .map(Node::Text)
            .collect(),
        Step::Abs => {
            let base = url::Url::parse(ctx.base).context("the source's address is not a URL")?;
            texts(nodes).filter_map(|text| base.join(text.trim()).ok()).map(|link| Node::Text(link.to_string())).collect()
        }
        Step::Date(formats) => texts(nodes).filter_map(|text| crate::date::parse(&text, formats, ctx.now_ms)).map(|ms| Node::Text(ms.to_string())).collect(),
        Step::Number => texts(nodes).filter_map(|text| crate::date::chapter_number(&text)).map(|number| Node::Text(number.to_string())).collect(),
        Step::Json => texts(nodes).filter_map(|text| serde_json::from_str::<Value>(text.trim()).ok()).map(Node::Json).collect(),
        Step::First => nodes.into_iter().take(1).collect(),
        Step::Last => nodes.into_iter().last().into_iter().collect(),
        Step::Nth(index) => nodes.into_iter().nth(*index).into_iter().collect(),
        Step::Join(separator) => {
            let all: Vec<String> = texts(nodes).filter(|text| !text.is_empty()).collect();
            if all.is_empty() { vec![] } else { vec![Node::Text(all.join(separator))] }
        }
        Step::Split(separator) => texts(nodes)
            .flat_map(|text| text.split(separator.as_str()).map(|piece| piece.trim().to_string()).collect::<Vec<_>>())
            .filter(|piece| !piece.is_empty())
            .map(Node::Text)
            .collect(),
        Step::Const(text) => vec![Node::Text(text.clone())],
        Step::Default(text) => {
            if nodes.iter().any(|node| !text_of(node).trim().is_empty()) { nodes } else { vec![Node::Text(text.clone())] }
        }
        Step::NonEmpty => nodes.into_iter().filter(|node| !text_of(node).trim().is_empty()).collect(),
        Step::Exists => {
            if nodes.iter().any(|node| !matches!(node, Node::Text(text) if text.is_empty())) { vec![Node::Text("true".into())] } else { vec![] }
        }
        Step::Map(table) => texts(nodes)
            .filter_map(|text| {
                let lower = text.to_lowercase();
                table.iter().find(|(needle, _)| lower.contains(&needle.to_lowercase())).map(|(_, value)| Node::Text(value.clone()))
            })
            .collect(),
        Step::Where(test) => nodes.into_iter().filter(|node| matches_where(node, test, ctx)).collect(),
        Step::PlainText => texts(nodes).map(|text| Node::Text(text_of(&parse_fragment(&text)))).collect(),
        Step::Sort => {
            let mut all: Vec<String> = texts(nodes).collect();
            all.sort_by(|a, b| natural_cmp(a, b));
            all.into_iter().map(Node::Text).collect()
        }
        Step::Unique => {
            let mut seen = std::collections::HashSet::new();
            nodes.into_iter().filter(|node| seen.insert(text_of(node))).collect()
        }
        Step::AesCbc(spec) => decrypt(nodes, spec, ctx)?,
        Step::Coalesce(options) => {
            for option in options {
                let found = run(&steps(option, Role::Field, format), format, nodes.clone(), ctx)?;
                if found.iter().any(|node| !text_of(node).trim().is_empty()) {
                    return Ok(found);
                }
            }
            vec![]
        }
    })
}

/// One payload as a site sends it: the ciphertext, and usually the salt and IV it went with.
#[derive(serde::Deserialize)]
struct Payload {
    ct: String,
    #[serde(default)]
    s: Option<String>,
    #[serde(default)]
    iv: Option<String>,
}

/// Decrypts each node as AES-CBC. A node is either the payload a site sends (an object, or the JSON text of one)
/// or bare ciphertext; the salt arrives in the payload, so it is read from there when the definition does not
/// state one, and the key is worked out once and shared by every node of that call.
fn decrypt(nodes: Vec<Node>, spec: &AesCbc, ctx: &Ctx) -> anyhow::Result<Vec<Node>> {
    let mut key: Option<crate::crypt::Key> = None;
    let mut out = Vec::new();
    for node in nodes {
        let payload = payload_of(&node)?;
        let key = match &key {
            Some(key) => key,
            None => {
                key = Some(key_for(spec, payload.as_ref(), ctx)?);
                key.as_ref().unwrap()
            }
        };
        let ct = match &payload {
            Some(payload) => payload.ct.clone(),
            None => text_of(&node),
        };
        let data = crate::crypt::bytes(&ct, spec.hex).with_context(|| format!("the value to decrypt is not {}", if spec.hex { "hexadecimal" } else { "base64" }))?;
        out.push(Node::Text(String::from_utf8_lossy(&crate::crypt::decrypt(key, &data)?).into_owned()));
    }
    Ok(out)
}

/// The payload object a node holds, when it holds one. A node that looks like a payload but is not one is an
/// error rather than an empty result: a site that changed its payload is worth saying out loud.
fn payload_of(node: &Node) -> anyhow::Result<Option<Payload>> {
    let value = match node {
        Node::Json(value) => value.clone(),
        _ => {
            let text = text_of(node);
            let text = text.trim();
            if !text.starts_with('{') {
                return Ok(None);
            }
            serde_json::from_str(text).with_context(|| "the payload is not JSON: has the site changed?")?
        }
    };
    Ok(match serde_json::from_value(value) {
        Ok(payload) => Some(payload),
        // An object with no `ct` is not a payload at all; one with `ct` but a wrong field type is a site change.
        Err(_) => bail!("the payload has no ciphertext (`ct`): has the site changed?"),
    })
}

fn key_for(spec: &AesCbc, payload: Option<&Payload>, ctx: &Ctx) -> anyhow::Result<crate::crypt::Key> {
    let fill = |text: &str| crate::template::fill(text, ctx.hole, false).with_context(|| format!("{text} needs a value this call does not have"));
    if let Some(passphrase) = &spec.passphrase {
        // The salt is usually in the payload; a definition that states one uses it instead.
        let salt = match &spec.salt {
            Some(salt) => fill(salt)?,
            None => payload.and_then(|payload| payload.s.clone()).context("aes_cbc needs a salt: none was stated and the payload has none")?,
        };
        let salt = crate::crypt::bytes(&salt, true).context("the salt is not hexadecimal")?;
        let mut derived = crate::crypt::derive(&fill(passphrase)?, &salt);
        // A payload may carry its own IV where a site varies it per chapter.
        if let Some(iv) = payload.and_then(|payload| payload.iv.clone()) {
            derived.iv = crate::crypt::bytes(fill(&iv)?.as_str(), true).context("the IV is not hexadecimal")?;
        }
        return Ok(derived);
    }
    let (Some(key), Some(iv)) = (&spec.key, &spec.iv) else {
        bail!("aes_cbc needs a passphrase, or a key and an iv");
    };
    crate::crypt::fixed(&fill(key)?, &fill(iv)?)
}

/// Compares as people read file names: runs of digits by value, the rest by text.
pub(crate) fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(text: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();
        while start < bytes.len() {
            let digit = bytes[start].is_ascii_digit();
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() == digit {
                end += 1;
            }
            out.push((digit, &text[start..end]));
            start = end;
        }
        out
    }
    let (left, right) = (chunks(a), chunks(b));
    for (x, y) in left.iter().zip(&right) {
        let order = match (x.0, y.0) {
            (true, true) => {
                let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
            }
            _ => x.1.cmp(y.1),
        };
        if order != std::cmp::Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn matches_where(node: &Node, test: &crate::def::Where, ctx: &Ctx) -> bool {
    let Node::Json(value) = node else { return false };
    let found = select(value, &test.path).into_iter().find(|found| !found.is_null());
    let text = found.and_then(scalar);
    if test.absent.is_some_and(|absent| absent != text.is_none()) {
        return false;
    }
    let have = text.as_deref().unwrap_or("");
    // A wanted value may name a hole of the request ("{id}").
    let fill = |wanted: &str| if wanted.contains('{') { crate::template::fill(wanted, ctx.hole, false).unwrap_or_default() } else { wanted.to_string() };
    test.equals.as_deref().is_none_or(|want| have == fill(want))
        && test.not_equals.as_deref().is_none_or(|not| have != fill(not))
        && test.one_of.as_ref().is_none_or(|list| list.iter().any(|item| *item == have || fill(item) == have))
}

/// Rejects what can be known to be wrong without a response: a bad selector, pattern or date format.
pub fn check_steps(list: &[Step], format: Format) -> anyhow::Result<()> {
    for step in list {
        match step {
            Step::Css(css) => {
                selector(css)?;
            }
            Step::Regex(text) => {
                pattern(text)?;
            }
            Step::Replace { from, .. } => {
                pattern(from)?;
            }
            Step::Date(formats) => {
                for format in formats {
                    if !crate::date::check(format) {
                        bail!("the date format \"{format}\" can never read a date (it needs a year)");
                    }
                }
            }
            Step::Coalesce(options) => {
                for option in options {
                    check_ex(option, Role::Field, format)?;
                }
            }
            Step::AesCbc(spec) => {
                // Rejected now rather than at the site: a definition that can never decrypt is a mistake.
                let stated = spec.key.is_some() && spec.iv.is_some();
                if spec.passphrase.is_none() && !stated {
                    bail!("aes_cbc needs a passphrase, or a key and an iv");
                }
                if spec.key.is_some() != spec.iv.is_some() {
                    bail!("aes_cbc needs both a key and an iv");
                }
                if spec.passphrase.is_some() && spec.key.is_some() {
                    bail!("aes_cbc takes a passphrase or a key, not both");
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn check_ex(ex: &Ex, role: Role, format: Format) -> anyhow::Result<()> {
    check_steps(&steps(ex, role, format), format)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(hole: &'a dyn Fn(&str) -> Option<String>) -> Ctx<'a> {
        Ctx::new("https://example.org", hole, 0)
    }

    fn ex(json: &str) -> Ex {
        serde_json::from_str(json).unwrap()
    }

    const PAGE: &str = r#"<div class="card"><a href="/series/solo/"><h3> Solo  Leveling </h3><img data-src="/c/solo.jpg" src="data:x"></a></div>
        <div class="card"><a href="/series/tower/"><h3>Tower</h3><img src="/c/tower.jpg"></a></div>"#;

    #[test]
    fn shorthand_reads_text_and_attributes() {
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let root = parse_html(PAGE);
        let rows = eval(&ex(r#""div.card""#), Role::Rows, Format::Html, &root, &ctx).unwrap();
        assert_eq!(rows.len(), 2);
        let title = eval_text(&ex(r#""h3""#), Format::Html, &rows[0], &ctx).unwrap();
        assert_eq!(title.as_deref(), Some("Solo Leveling"));
        let href = eval_text(&ex(r#""a@href""#), Format::Html, &rows[1], &ctx).unwrap();
        assert_eq!(href.as_deref(), Some("/series/tower/"));
    }

    #[test]
    fn a_pipe_extracts_ids_and_falls_back_between_attributes() {
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let rows = eval(&ex(r#""div.card""#), Role::Rows, Format::Html, &parse_html(PAGE), &ctx).unwrap();
        let id = ex(r#"[{"css":"a"},{"attr":"href"},{"regex":"/series/([^/]+)/"}]"#);
        assert_eq!(eval_text(&id, Format::Html, &rows[0], &ctx).unwrap().as_deref(), Some("solo"));
        let cover = ex(r#"[{"coalesce":[[{"css":"img"},{"attr":"data-src"}],[{"css":"img"},{"attr":"src"}]]},"abs"]"#);
        assert_eq!(eval_text(&cover, Format::Html, &rows[0], &ctx).unwrap().as_deref(), Some("https://example.org/c/solo.jpg"));
        assert_eq!(eval_text(&cover, Format::Html, &rows[1], &ctx).unwrap().as_deref(), Some("https://example.org/c/tower.jpg"));
    }

    #[test]
    fn json_paths_and_a_script_blob_are_read_alike() {
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let body = r#"{"data":[{"id":"a","attributes":{"title":{"en":"One"}}},{"id":"b","attributes":{"title":{"en":"Two"}}}]}"#;
        let root = parse_root(body, Format::Json).unwrap();
        let rows = eval(&ex(r#""data""#), Role::Rows, Format::Json, &root, &ctx).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(eval_text(&ex(r#""attributes.title.en""#), Format::Json, &rows[1], &ctx).unwrap().as_deref(), Some("Two"));

        let html = parse_html(r#"<script id="d">{"pages":["/1.jpg","/2.jpg"]}</script>"#);
        let pages = ex(r##"[{"css":"script#d"},"text","json",{"path":"pages.*"},"text"]"##);
        let found = eval_texts(&pages, Format::Html, &html, &ctx).unwrap();
        assert_eq!(found, vec!["/1.jpg", "/2.jpg"]);
    }

    #[test]
    fn templates_see_the_value_and_the_request_holes() {
        let hole = |name: &str| (name == "id").then(|| "solo".to_string());
        let ctx = ctx(&hole);
        let from = Node::Text("12".into());
        let out = run(&[Step::Template("{id}-ch-{value}".into())], Format::Html, vec![from], &ctx).unwrap();
        assert_eq!(text_of(&out[0]), "solo-ch-12");
    }

    #[test]
    fn blocks_are_lines_and_inline_markup_is_not() {
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let root = parse_html("<div id=d><p>One <b>bo</b>ld.</p><p>Two.</p><br>Three</div>");
        let found = eval_text(&ex(r##""#d""##), Format::Html, &root, &ctx).unwrap();
        assert_eq!(found.as_deref(), Some("One bold.\nTwo.\nThree"));
    }

    #[test]
    fn files_sort_the_way_people_read_them_and_where_can_name_a_hole() {
        let hole = |name: &str| (name == "id").then(|| "ep2".to_string());
        let ctx = ctx(&hole);
        let from = Node::Json(serde_json::json!({"files": ["p10.jpg", "p2.jpg", "p1.jpg", "cover.jpg"]}));
        let sorted = eval(&ex(r#"[{"path":"files"},"sort"]"#), Role::Field, Format::Json, &from, &ctx).unwrap();
        assert_eq!(sorted.iter().map(text_of).collect::<Vec<_>>(), ["cover.jpg", "p1.jpg", "p2.jpg", "p10.jpg"]);
        let rows = Node::Json(serde_json::json!([{"name": "ep1", "t": "one"}, {"name": "ep2", "t": "two"}]));
        let found = eval(&ex(r#"[{"path":""},{"where":{"path":"name","equals":"{id}"}},{"path":"t"}]"#), Role::Field, Format::Json, &rows, &ctx).unwrap();
        assert_eq!(found.iter().map(text_of).collect::<Vec<_>>(), ["two"]);
    }

    /// The status word a site gives has to survive `map`, because the Madara theme passes every status through it.
    /// `map` keeps a node only when a needle is found, so an unlisted word disappears silently — which is how a
    /// status of "unknown" used to appear with no explanation.
    #[test]
    fn a_status_word_maps_and_an_unlisted_one_disappears() {
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let step = Step::Map(vec![
            ("not yet released".into(), "upcoming".into()),
            ("ongoing".into(), "ongoing".into()),
            ("completed".into(), "completed".into()),
            ("complete".into(), "completed".into()),
            ("on hold".into(), "hiatus".into()),
        ]);
        let mapped = |text: &str| run(std::slice::from_ref(&step), Format::Html, vec![Node::Text(text.into())], &ctx).unwrap().iter().map(text_of).collect::<Vec<_>>();
        assert_eq!(mapped("Ongoing"), ["ongoing"]);
        assert_eq!(mapped("OnGoing"), ["ongoing"]);
        assert_eq!(mapped("Completed"), ["completed"]);
        assert_eq!(mapped("On Hold"), ["hiatus"]);
        assert_eq!(mapped("Not Yet Released"), ["upcoming"]);
        // A word nobody listed gives nothing, so a site that renames its statuses is caught by its fixture
        // rather than reported to the person as an empty status.
        assert_eq!(mapped("Serialised"), Vec::<String>::new());
    }

    #[test]
    fn a_bad_selector_is_caught_by_check() {
        assert!(check_steps(&[Step::Css("div[".into())], Format::Html).is_err());
        assert!(check_steps(&[Step::Regex("(".into())], Format::Html).is_err());
        assert!(check_steps(&[Step::Date(vec!["MMM d".into()])], Format::Html).is_err());
    }

    /// AES-256-CBC with PKCS#7, the way a site's script encrypts a payload: what a definition must undo.
    fn encrypt(key: &[u8], iv: &[u8], plain: &[u8]) -> Vec<u8> {
        use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
        let mut buffer = plain.to_vec();
        buffer.resize(plain.len() + 16 - plain.len() % 16, 0);
        let length = buffer.len();
        cbc::Encryptor::<aes::Aes256>::new_from_slices(key, iv).unwrap().encrypt_padded_mut::<Pkcs7>(&mut buffer[..length], plain.len()).unwrap().to_vec()
    }

    fn hex(text: &str) -> Vec<u8> {
        crate::crypt::bytes(text, true).unwrap()
    }

    fn base64(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    /// A chapter protector's payload, made the way the site's own script makes it (an MD5-stretched passphrase and
    /// the salt the payload carries, AES-256-CBC, base64 in `ct`, hex in `s`), then read back through a pipe
    /// exactly as a definition would: no hole for the salt, because the payload holds it.
    #[test]
    fn an_encrypted_chapter_list_is_decrypted_with_a_passphrase_and_the_salt_in_the_payload() {
        let salt = "0001020304050607";
        let key = crate::crypt::derive("s3cret", &hex(salt));
        let plain = br#"["https://img.test/1.jpg","https://img.test/2.jpg"]"#;
        let payload = serde_json::json!({ "ct": base64(&encrypt(&key.bytes, &key.iv, plain)), "s": salt }).to_string();
        let none = |_: &str| None;
        let ctx = ctx(&none);
        let pipe = ex(r#"["json",{"aes_cbc":{"passphrase":"s3cret"}}]"#);
        let out = run(&steps(&pipe, Role::Field, Format::Html), Format::Html, vec![Node::Text(payload.clone())], &ctx).unwrap();
        assert_eq!(out.iter().map(text_of).collect::<Vec<_>>(), [String::from_utf8_lossy(plain)]);
        // The same payload arriving as JSON rather than as text reads the same way.
        let out = run(&steps(&pipe, Role::Field, Format::Html), Format::Html, vec![parse_root(&payload, Format::Json).unwrap()], &ctx).unwrap();
        assert_eq!(out.iter().map(text_of).collect::<Vec<_>>(), [String::from_utf8_lossy(plain)]);
        // A payload whose `ct` is missing, and one that is not JSON at all, both say so rather than blaming
        // base64. Read without the `json` step first, because that step drops what it cannot parse.
        let direct = ex(r#"[{"aes_cbc":{"passphrase":"s3cret"}}]"#);
        for (broken, wanted) in [("{not json", "not JSON"), (r#"{"s":"00"}"#, "no ciphertext")] {
            let error = match run(&steps(&direct, Role::Field, Format::Html), Format::Html, vec![Node::Text(broken.into())], &ctx) {
                Err(error) => format!("{error:#}"),
                Ok(_) => panic!("{broken} must not decrypt"),
            };
            assert!(error.contains(wanted), "{broken}: {error}");
        }
        // No salt anywhere is said plainly, not decrypted into nonsense.
        let bare = serde_json::json!({ "ct": base64(&encrypt(&key.bytes, &key.iv, plain)) }).to_string();
        let error = match run(&steps(&pipe, Role::Field, Format::Html), Format::Html, vec![Node::Text(bare)], &ctx) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("a payload with no salt must not decrypt"),
        };
        assert!(error.contains("needs a salt"), "{error}");
    }

    #[test]
    fn a_stated_key_decrypts_and_a_half_specified_one_is_caught_by_check() {
        let (key, iv) = ("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff", "000102030405060708090a0b0c0d0e0f");
        let plain = b"https://img.test/page.jpg";
        let from = Node::Text(base64(&encrypt(&hex(key), &hex(iv), plain)));
        let none = |_: &str| None;
        let step = |key: Option<&str>, iv: Option<&str>| Step::AesCbc(AesCbc { passphrase: None, salt: None, key: key.map(Into::into), iv: iv.map(Into::into), hex: false });
        let out = run(&[step(Some(key), Some(iv))], Format::Html, vec![from], &ctx(&none)).unwrap();
        assert_eq!(text_of(&out[0]), String::from_utf8_lossy(plain));

        assert!(check_steps(&[step(Some(key), None)], Format::Html).is_err(), "half a key pair");
        assert!(check_steps(&[step(None, None)], Format::Html).is_err(), "no key at all");
        let wrong = Step::AesCbc(AesCbc { passphrase: None, salt: None, key: Some("ff".repeat(32)), iv: Some(iv.into()), hex: false });
        assert!(run(&[wrong], Format::Html, vec![Node::Text(base64(&encrypt(&hex(key), &hex(iv), plain)))], &ctx(&none)).is_err());
    }
}
