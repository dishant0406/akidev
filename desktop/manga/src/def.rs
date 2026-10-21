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
    /// Keeps the JSON nodes whose value at `path` passes: equals, differs from, is one of, or is absent.
    Where(Where),
    /// Drops texts already seen.
    Unique,
    /// HTML in a text (a JSON field that holds markup) as plain text: tags gone, blocks on their own lines.
    PlainText,
    /// Orders texts the way people read file names: runs of digits by their value (`p2` before `p10`).
    Sort,
}

/// A test on a JSON node, by a value inside it. All the tests given must hold. A value may hold request
/// holes (`"equals": "{id}"`: the row for the id being asked about).
#[derive(Debug, Clone, Deserialize)]
pub struct Where {
    pub path: String,
    #[serde(default)]
    pub equals: Option<String>,
    #[serde(default)]
    pub not_equals: Option<String>,
    #[serde(default, rename = "in")]
    pub one_of: Option<Vec<String>>,
    /// True keeps nodes with nothing at `path` (or null); false keeps nodes with something there.
    #[serde(default)]
    pub absent: Option<bool>,
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

/// A request made before a section's own, whose answer gives values (`vars`) that later requests and the
/// section's expressions use as holes: a page that holds a nonce, a lookup that gives the server to ask next.
#[derive(Debug, Clone, Deserialize)]
pub struct Prepare {
    pub request: Request,
    #[serde(default)]
    pub format: Format,
    pub vars: BTreeMap<String, Ex>,
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
    /// How many titles the source has in all (with `page_size`, this says whether another page follows).
    #[serde(default)]
    pub total: Option<Ex>,
    #[serde(default)]
    pub prepare: Vec<Prepare>,
    /// Values read from the answer, usable as holes in the expressions below.
    #[serde(default)]
    pub vars: BTreeMap<String, Ex>,
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
    #[serde(default)]
    pub prepare: Vec<Prepare>,
    #[serde(default)]
    pub vars: BTreeMap<String, Ex>,
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
    /// For a source that pages its chapter list: ask again with `{offset}` raised until `total` is reached.
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub prepare: Vec<Prepare>,
    #[serde(default)]
    pub vars: BTreeMap<String, Ex>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repeat {
    /// Chapters asked for each time; the next request's `{offset}` is this much higher.
    pub size: u32,
    /// How many chapters there are in all, read from each answer.
    pub total: Ex,
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
    /// A header whose value is empty is left out (`"referer": ""`).
    #[serde(default)]
    pub image_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub prepare: Vec<Prepare>,
    #[serde(default)]
    pub vars: BTreeMap<String, Ex>,
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

/// A choice the person makes for a source (a language, a content rating). Its value fills the hole named
/// by its id in requests; options and the default are the definition's.
#[derive(Debug, Clone, Deserialize)]
pub struct SettingDef {
    pub id: String,
    pub name: String,
    pub options: Vec<FilterOption>,
    pub default: String,
}

fn page_size() -> u32 {
    20
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
    /// Sent instead of the browser one, for a source that asks clients to say who they are.
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Titles on a page, for `{offset}` (and `total`): `(page - 1) * page_size`.
    #[serde(default = "page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub settings: Vec<SettingDef>,
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
