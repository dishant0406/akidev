//! The shapes a source definition is made of. Plain data: nothing here runs.
//!
//! A definition says which requests to make and how to read the answer. Reading a value is an
//! `Ex`: a short text, or a list of steps (a pipe) that each turn the nodes before it into new ones.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    #[default]
    Html,
    Json,
}

/// What an expression is for: rows are nodes to read fields from, a field is read down to text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Rows,
    Field,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Ex {
    /// HTML: a CSS selector, `selector@attribute`, or `@attribute`. JSON: a dotted path.
    Short(String),
    Pipe(Vec<Step>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// The elements matching a selector, below each node (or in a text that is HTML).
    Css(String),
    /// An attribute, or the first of several (`data-src|src`) that is not empty.
    Attr(String),
    Text,
    /// An element's inner HTML.
    Html,
    /// A dotted path into JSON (`*` is every value).
    Path(String),
    /// The first capture group, or the whole match; a node that does not match is dropped.
    Regex(String),
    Replace { from: String, to: String },
    Trim,
    Lower,
    /// Text with `{value}` (the node) and the request's holes; `{? ... ?}` groups vanish when empty.
    Template(String),
    /// A link made absolute against the source's address.
    Abs,
    /// Dates, tried in order: a pattern (`MMM d, yyyy`), `iso` or `relative`. Gives milliseconds.
    Date(Vec<String>),
    /// The chapter or other number in a text.
    Number,
    /// Text parsed as JSON (a `<script>` blob, for one).
    Json,
    First,
    Last,
    Nth(usize),
    /// All texts as one.
    Join(String),
    /// Splits each text and keeps the pieces.
    Split(String),
    Const(String),
    /// A text when there is none.
    Default(String),
    /// Drops empty texts.
    NonEmpty,
    /// "true" when there is anything, nothing otherwise.
    Exists,
    /// The value of the first key the lowercased text contains.
    Map(Vec<(String, String)>),
    /// The first expression that gives something.
    Coalesce(Vec<Ex>),
}

fn get() -> String {
    "GET".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    #[serde(default = "get")]
    pub method: String,
    /// Must start with `{base}`: a request can only go to the source's own address.
    pub url: String,
    #[serde(default)]
    pub form: BTreeMap<String, String>,
    #[serde(default)]
    pub json: Option<Value>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardDef {
    pub id: Ex,
    pub title: Ex,
    #[serde(default)]
    pub cover: Option<Ex>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListDef {
    pub request: Request,
    #[serde(default)]
    pub format: Format,
    pub rows: Ex,
    pub card: CardDef,
    /// Something that exists when another page follows. Without it, a page with rows has a next one.
    #[serde(default)]
    pub next: Option<Ex>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetailDef {
    pub request: Request,
    #[serde(default)]
    pub format: Format,
    pub title: Ex,
    #[serde(default)]
    pub cover: Option<Ex>,
    #[serde(default)]
    pub description: Option<Ex>,
    #[serde(default)]
    pub authors: Option<Ex>,
    #[serde(default)]
    pub tags: Option<Ex>,
    #[serde(default)]
    pub status: Option<Ex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Order {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterDef {
    pub id: Ex,
    #[serde(default)]
    pub number: Option<Ex>,
    #[serde(default)]
    pub title: Option<Ex>,
    #[serde(default)]
    pub date: Option<Ex>,
    #[serde(default)]
    pub scanlator: Option<Ex>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChaptersDef {
    pub request: Request,
    #[serde(default)]
    pub format: Format,
    pub rows: Ex,
    pub chapter: ChapterDef,
    #[serde(default)]
    pub order: Order,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PagesDef {
    pub request: Request,
    #[serde(default)]
    pub format: Format,
    pub images: Ex,
    /// `longstrip`, `paged` or `paged_rtl`; manhwa sources say `longstrip`.
    #[serde(default)]
    pub reading: Option<String>,
    /// Headers for every image; a referer of the source's address is sent when none is given.
    #[serde(default)]
    pub image_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilterOption {
    pub value: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKind {
    Select,
    Multi,
    Text,
    Sort,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilterDef {
    pub id: String,
    pub name: String,
    pub kind: FilterKind,
    #[serde(default)]
    pub options: Vec<FilterOption>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceDef {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: u64,
    #[serde(default)]
    pub lang: Vec<String>,
    #[serde(default)]
    pub nsfw: bool,
    pub base_url: String,
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// Requests per second this source is asked for at most.
    #[serde(default)]
    pub rate_per_second: Option<f64>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub popular: Option<ListDef>,
    #[serde(default)]
    pub latest: Option<ListDef>,
    #[serde(default)]
    pub search: Option<ListDef>,
    #[serde(default)]
    pub filters: Vec<FilterDef>,
    pub detail: DetailDef,
    pub chapters: ChaptersDef,
    pub pages: PagesDef,
}

pub const SCHEMA: u32 = 1;
