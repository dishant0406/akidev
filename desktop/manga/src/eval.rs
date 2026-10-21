//! Reading values out of a response, as an `Ex` describes.
//!
//! Everything is a list of nodes: an HTML element, a JSON value or a text. A step turns a list
//! into a list. Evaluation is synchronous and holds the parsed page only for its own duration (an
//! HTML tree is not `Send`, so it never lives across an await).

use crate::def::{Ex, Format, Role, Step};
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
    run(&steps(ex, role, format), format, vec![from.clone()], ctx)
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
                let hole = |name: &str| if name == "value" { Some(value.clone()) } else { (ctx.hole)(name) };
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
        Ctx { base: "https://example.org", hole, now_ms: 0 }
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
    fn a_bad_selector_is_caught_by_check() {
        assert!(check_steps(&[Step::Css("div[".into())], Format::Html).is_err());
        assert!(check_steps(&[Step::Regex("(".into())], Format::Html).is_err());
        assert!(check_steps(&[Step::Date(vec!["MMM d".into()])], Format::Html).is_err());
    }
}
