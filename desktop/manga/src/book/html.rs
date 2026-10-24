//! A chapter's markup and styles turned into blocks of text and pictures.
//!
//! This is not a browser. It follows the part of HTML and CSS that books use: paragraphs, headings, lists,
//! quotes, emphasis, links, pictures, and about thirty properties. Everything else is read through and ignored,
//! so a book that uses more than that still opens, with its text in order.

use super::css::{self, Sheet};
use super::{Align, Anchor, Block, Chapter, Kind, Look, Run};
use scraper::{ElementRef, Html, Node};

/// What the reader does for an element before the book says anything.
pub const DEFAULTS: &str = "
h1 { font-size: 2em; font-weight: bold; margin: .67em 0 }
h2 { font-size: 1.5em; font-weight: bold; margin: .75em 0 }
h3 { font-size: 1.17em; font-weight: bold; margin: 1em 0 }
h4 { font-weight: bold; margin: 1.3em 0 }
h5 { font-size: .83em; font-weight: bold; margin: 1.5em 0 }
h6 { font-size: .67em; font-weight: bold; margin: 1.7em 0 }
p, pre, figure, dl, address { margin: 1em 0 }
blockquote { margin: 1em 0 1em 2em }
ul, ol { margin: 1em 0; padding-left: 2em }
dd { margin-left: 2em }
pre { font-family: monospace; white-space: pre }
b, strong { font-weight: bold }
i, em, cite, dfn, var { font-style: italic }
u, ins { text-decoration: underline }
s, strike, del { text-decoration: line-through }
small { font-size: .83em }
big { font-size: 1.17em }
sup { vertical-align: super; font-size: .83em }
sub { vertical-align: sub; font-size: .83em }
code, kbd, samp, tt { font-family: monospace }
caption, th { font-weight: bold }
center { text-align: center }
head, script, style, title, meta, link, noscript, template, audio, video, object, canvas { display: none }
";

/// Tags that start a new block of text unless the book's CSS says otherwise.
const BLOCKS: &[&str] = &[
    "html",
    "body",
    "div",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "pre",
    "section",
    "article",
    "aside",
    "header",
    "footer",
    "nav",
    "main",
    "figure",
    "figcaption",
    "address",
    "hr",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "caption",
    "form",
    "fieldset",
    "details",
    "summary",
    "center",
];

/// Past this many levels of nesting the rest is not read: it is a broken or hostile file.
const MAX_DEPTH: usize = 200;

#[derive(Clone)]
struct State {
    look: Look,
    align: Align,
    /// First-line indent in em, as CSS inherits it.
    indent: f32,
    /// Distance from the left edge in em, and the heading level of the block being written.
    left: f32,
    heading: u8,
    pre: bool,
    transform: Transform,
}

#[derive(Clone, Copy, PartialEq)]
enum Transform {
    None,
    Upper,
    Lower,
}

/// How a block is set, apart from its text.
#[derive(Clone)]
struct Props {
    heading: u8,
    align: Align,
    left: f32,
    first_line: f32,
}

/// A paragraph being written: the text so far and how it is set.
struct Para {
    runs: Vec<Run>,
    length: u32,
    props: Props,
}

struct Builder<'a> {
    sheet: &'a Sheet,
    /// The chapter's path in the book, to make the paths of pictures and links whole.
    path: &'a str,
    blocks: Vec<Block>,
    anchors: Vec<Anchor>,
    /// Where the next block starts, in UTF-16 units.
    offset: u32,
    para: Option<Para>,
    /// Space waiting to go above the next block, in em.
    space: f32,
    page_break: bool,
    /// One counter for each list being read; 0 for a bulleted one.
    lists: Vec<Option<u32>>,
    marker: Option<String>,
    /// The font the body text is set in. The reader's own font replaces it; only a different one is the book's own to keep.
    body_family: Option<String>,
}

pub fn build(html: &Html, sheet: &Sheet, path: &str) -> Chapter {
    let mut builder = Builder {
        sheet,
        path,
        blocks: Vec::new(),
        anchors: Vec::new(),
        offset: 0,
        para: None,
        space: 0.0,
        page_break: false,
        lists: Vec::new(),
        marker: None,
        body_family: None,
    };
    let state = State::root();
    let start = html
        .select(&scraper::Selector::parse("body").expect("a fixed selector"))
        .next()
        .unwrap_or_else(|| html.root_element());
    builder.element(start, &state, 0);
    builder.flush();
    Chapter {
        blocks: builder.blocks,
        anchors: builder.anchors,
        fonts: Vec::new(),
    }
}

impl Builder<'_> {
    fn element(&mut self, element: ElementRef, parent: &State, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let name = element.value().name();
        let mut declarations = self.sheet.declarations(&element);
        if let Some(inline) = element.value().attr("style") {
            declarations.extend(
                css::declarations(inline)
                    .into_iter()
                    .map(|(n, v, _)| (n, v)),
            );
        }
        let mut style = Style::new(parent);
        for (property, value) in &declarations {
            style.apply(property, value, parent);
        }
        if name == "body" || name == "html" {
            // What the body is set in is the text's face, which the reader's own choice stands in for.
            if let Some(family) = style.state.look.family.take() {
                self.body_family = Some(family);
            }
        } else if style.state.look.family.is_some() && style.state.look.family == self.body_family {
            style.state.look.family = None;
        }
        if style.hidden || element.value().attr("hidden").is_some() {
            return;
        }
        let is_block = style
            .display
            .map_or(BLOCKS.contains(&name), |display| display == Display::Block);
        if is_block {
            // What came before ends here, so that an id on this block points at its start.
            self.flush();
        }
        for id in [
            element.value().attr("id"),
            element.value().attr("name").filter(|_| name == "a"),
        ]
        .into_iter()
        .flatten()
        {
            self.anchor(id);
        }
        let mut state = style.state;
        if let (true, Some(href)) = (name == "a", element.value().attr("href")) {
            state.look.link = Some(link_to(self.path, href));
        }

        match name {
            "img" => {
                let ink = element
                    .value()
                    .classes()
                    .any(|class| class.ends_with("black-on-transparent"));
                return self.image(attribute(&element, "src"), ink);
            }
            "svg" => return self.svg(element),
            "br" => return self.text("\u{2028}", &state, true),
            "hr" => {
                self.flush();
                self.space = self.space.max(style.margin_top);
                self.push(Kind::Rule, centered());
                self.space = style.margin_bottom;
                return;
            }
            _ => {}
        }

        if is_block {
            self.space = self.space.max(style.margin_top);
            self.page_break |= style.break_before;
            state.left = (parent.left + style.margin_left + style.padding_left).max(0.0);
            if name.len() == 2 && name.starts_with('h') {
                state.heading = name[1..].parse().unwrap_or(0);
            }
            match name {
                "ul" => self.lists.push(None),
                "ol" => self.lists.push(Some(0)),
                "li" => self.marker = Some(self.next_marker()),
                _ => {}
            }
        } else if name == "td" || name == "th" {
            // A table is read row by row, each cell after the one before it.
            if self.para.is_some() {
                self.text("\u{2003}\u{2003}", &state, true);
            }
        }

        for child in element.children() {
            match child.value() {
                Node::Text(text) => {
                    let text = apply_transform(text, state.transform);
                    self.text(&text, &state, state.pre);
                }
                Node::Element(_) => {
                    if let Some(child) = ElementRef::wrap(child) {
                        self.element(child, &state, depth + 1);
                    }
                }
                _ => {}
            }
        }

        if is_block {
            self.flush();
            self.space = self.space.max(style.margin_bottom);
            self.page_break |= style.break_after;
            if name == "ul" || name == "ol" {
                self.lists.pop();
            }
        }
    }

    fn next_marker(&mut self) -> String {
        match self.lists.last_mut() {
            Some(Some(count)) => {
                *count += 1;
                format!("{count}.\t")
            }
            _ => "\u{2022}\t".to_string(),
        }
    }

    fn anchor(&mut self, id: &str) {
        let offset = self.offset + self.para.as_ref().map_or(0, |para| para.length);
        self.anchors.push(Anchor {
            id: id.to_string(),
            offset,
        });
    }

    /// Adds text to the paragraph being written, starting one if there is none. Spaces are collapsed as a browser
    /// does, except in preformatted text and for the line break.
    fn text(&mut self, text: &str, state: &State, keep_spaces: bool) {
        let mut text = if keep_spaces {
            // A newline would end the paragraph; inside one, a line separator breaks the line.
            text.replace('\n', "\u{2028}")
        } else {
            collapse(text)
        };
        if text.is_empty() {
            return;
        }
        if !keep_spaces {
            // A space at the start of a paragraph or after another space shows nothing.
            let after_space = self.para.as_ref().is_none_or(|para| {
                para.runs
                    .last()
                    .is_none_or(|run| run.text.ends_with([' ', '\u{2028}']))
            });
            if after_space {
                text = text.trim_start_matches(' ').to_string();
            }
            if text.is_empty() {
                return;
            }
        }
        if self.para.is_none() {
            self.start(state);
        }
        let para = self.para.as_mut().expect("a paragraph was just started");
        para.length += units(&text);
        if let Some(run) = para.runs.last_mut().filter(|run| run.look == state.look) {
            run.text.push_str(&text);
        } else {
            para.runs.push(Run {
                text,
                look: state.look.clone(),
            });
        }
    }

    fn start(&mut self, state: &State) {
        let mut para = Para {
            runs: Vec::new(),
            length: 0,
            props: state.props(),
        };
        if let Some(marker) = self.marker.take() {
            // The marker hangs to the left of the text, which lines up after the tab.
            para.props.first_line = -1.5;
            para.length = units(&marker);
            para.runs.push(Run {
                text: marker,
                look: state.look.clone(),
            });
        }
        self.para = Some(para);
    }

    /// Ends the paragraph being written, if it has any text.
    fn flush(&mut self) {
        let Some(mut para) = self.para.take() else {
            return;
        };
        if let Some(last) = para.runs.last_mut() {
            last.text.truncate(last.text.trim_end_matches(' ').len());
        }
        para.runs.retain(|run| !run.text.is_empty());
        let shows_something = para
            .runs
            .iter()
            .any(|run| !run.text.trim().is_empty() || run.text.contains('\u{2028}'));
        if shows_something {
            self.push(Kind::Text(para.runs), para.props);
        }
    }

    fn push(&mut self, kind: Kind, props: Props) {
        let length = match &kind {
            Kind::Text(runs) => runs.iter().map(|run| units(&run.text)).sum(),
            // One placeholder character stands for the picture or rule.
            _ => 1,
        };
        self.blocks.push(Block {
            start: self.offset,
            kind,
            heading: props.heading,
            align: props.align,
            left: props.left,
            first_line: props.first_line,
            space_before: self.space,
            page_break_before: std::mem::take(&mut self.page_break),
        });
        self.space = 0.0;
        // Every block ends with a newline.
        self.offset += length + 1;
    }

    fn image(&mut self, src: Option<&str>, ink: bool) {
        let Some(src) = src.filter(|src| !src.starts_with("data:")) else {
            return;
        };
        self.flush();
        let kind = Kind::Image {
            src: join(self.path, src),
            ink,
        };
        self.push(kind, centered());
    }

    /// A cover page is often an `<svg>` around one `<image>`.
    fn svg(&mut self, svg: ElementRef) {
        for node in svg.descendants() {
            if let Some(element) = ElementRef::wrap(node).filter(|e| e.value().name() == "image") {
                self.image(attribute(&element, "href"), false);
            }
        }
    }
}

impl State {
    fn root() -> State {
        State {
            look: Look {
                bold: false,
                italic: false,
                underline: false,
                strike: false,
                small_caps: false,
                mono: false,
                shift: 0,
                size: 1.0,
                family: None,
                link: None,
            },
            align: Align::Start,
            indent: 0.0,
            left: 0.0,
            heading: 0,
            pre: false,
            transform: Transform::None,
        }
    }

    fn props(&self) -> Props {
        Props {
            heading: self.heading,
            align: self.align,
            left: self.left,
            first_line: self.indent,
        }
    }
}

/// Pictures and rules are set in the middle, at the left edge's distance of nothing.
fn centered() -> Props {
    Props {
        align: Align::Center,
        ..State::root().props()
    }
}

/// An attribute by its name without a namespace: `href` finds `xlink:href` too.
fn attribute<'a>(element: &ElementRef<'a>, name: &str) -> Option<&'a str> {
    element
        .value()
        .attrs()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// A text's length in UTF-16 units, the unit positions are counted in.
fn units(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            space = true;
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    if space {
        out.push(' ');
    }
    out
}

fn apply_transform(text: &str, transform: Transform) -> String {
    match transform {
        Transform::None => text.to_string(),
        Transform::Upper => text.to_uppercase(),
        Transform::Lower => text.to_lowercase(),
    }
}

/// A link as written in the chapter at `from`, made whole: a path inside the book with the `#id` kept, or the
/// web address as it was.
pub fn link_to(from: &str, href: &str) -> String {
    let href = href.trim();
    if is_external(href) {
        return href.to_string();
    }
    match href.split_once('#') {
        Some(("", id)) => format!("{from}#{id}"),
        Some((path, id)) => format!("{}#{id}", join(from, path)),
        None => join(from, href),
    }
}

pub fn is_external(href: &str) -> bool {
    href.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
    })
}

/// `href` as seen from the file at `from`, as a path from the top of the book, with `..` resolved and `%20`
/// and the like decoded.
pub fn join(from: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or("");
    let mut parts: Vec<&str> = if href.starts_with('/') {
        Vec::new()
    } else {
        let mut dir: Vec<&str> = from.split('/').collect();
        dir.pop();
        dir
    };
    let decoded = percent_decode(href);
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Clone, Copy, PartialEq)]
enum Display {
    Block,
    Inline,
}

/// What an element's declarations came to.
struct Style {
    state: State,
    display: Option<Display>,
    hidden: bool,
    margin_top: f32,
    margin_bottom: f32,
    margin_left: f32,
    padding_left: f32,
    break_before: bool,
    break_after: bool,
}

impl Style {
    fn new(parent: &State) -> Style {
        Style {
            state: parent.clone(),
            display: None,
            hidden: false,
            margin_top: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_left: 0.0,
            break_before: false,
            break_after: false,
        }
    }

    fn apply(&mut self, property: &str, value: &str, parent: &State) {
        let value = value.trim().to_lowercase();
        let state = &mut self.state;
        match property {
            "display" => match value.as_str() {
                "none" => self.hidden = true,
                "block" | "list-item" | "table" | "table-row" | "table-row-group" | "flex"
                | "grid" => self.display = Some(Display::Block),
                _ => self.display = Some(Display::Inline),
            },
            "visibility" if value == "hidden" => self.hidden = true,
            "font-weight" => {
                state.look.bold = match value.as_str() {
                    "bold" | "bolder" => true,
                    "normal" | "lighter" => false,
                    number => number.parse::<u32>().is_ok_and(|weight| weight >= 600),
                }
            }
            "font-style" => state.look.italic = value == "italic" || value == "oblique",
            "text-decoration" | "text-decoration-line" => {
                state.look.underline = value.contains("underline");
                state.look.strike = value.contains("line-through");
            }
            "font-variant" | "font-variant-caps" => {
                state.look.small_caps = value.contains("small-caps")
            }
            "font-family" => {
                state.look.family = css::first_family(&value);
                state.look.mono = ["mono", "courier", "consolas", "menlo"]
                    .iter()
                    .any(|name| value.contains(name))
            }
            "font-size" => {
                if let Some(size) = font_size(&value, parent.look.size) {
                    state.look.size = size.clamp(0.4, 4.0);
                }
            }
            "vertical-align" => {
                state.look.shift = match value.as_str() {
                    "super" => 1,
                    "sub" => -1,
                    "baseline" => 0,
                    _ => state.look.shift,
                }
            }
            "text-align" => {
                state.align = match value.as_str() {
                    "center" => Align::Center,
                    "right" | "end" => Align::End,
                    "justify" => Align::Justify,
                    _ => Align::Start,
                }
            }
            "text-indent" => {
                if let Some(indent) = length(&value, state.look.size) {
                    state.indent = indent.clamp(-3.0, 6.0);
                }
            }
            "white-space" => state.pre = value.starts_with("pre"),
            "text-transform" => {
                state.transform = match value.as_str() {
                    "uppercase" => Transform::Upper,
                    "lowercase" => Transform::Lower,
                    _ => Transform::None,
                }
            }
            "margin" => {
                let sides: Vec<Option<f32>> = value
                    .split_whitespace()
                    .map(|side| length(side, state.look.size))
                    .collect();
                let (top, bottom, left) = match sides.as_slice() {
                    [all] => (*all, *all, *all),
                    [vertical, horizontal] => (*vertical, *vertical, *horizontal),
                    [top, horizontal, bottom] => (*top, *bottom, *horizontal),
                    [top, _, bottom, left] => (*top, *bottom, *left),
                    _ => (None, None, None),
                };
                self.margin_top = top.unwrap_or(0.0).clamp(0.0, 6.0);
                self.margin_bottom = bottom.unwrap_or(0.0).clamp(0.0, 6.0);
                self.margin_left = left.unwrap_or(0.0).clamp(0.0, 8.0);
            }
            "margin-top" => {
                self.margin_top = length(&value, state.look.size)
                    .unwrap_or(0.0)
                    .clamp(0.0, 6.0)
            }
            "margin-bottom" => {
                self.margin_bottom = length(&value, state.look.size)
                    .unwrap_or(0.0)
                    .clamp(0.0, 6.0)
            }
            "margin-left" | "margin-inline-start" => {
                self.margin_left = length(&value, state.look.size)
                    .unwrap_or(0.0)
                    .clamp(0.0, 8.0)
            }
            "padding-left" | "padding-inline-start" => {
                self.padding_left = length(&value, state.look.size)
                    .unwrap_or(0.0)
                    .clamp(0.0, 8.0)
            }
            "page-break-before" | "break-before" => {
                self.break_before = matches!(value.as_str(), "always" | "page" | "left" | "right")
            }
            "page-break-after" | "break-after" => {
                self.break_after = matches!(value.as_str(), "always" | "page" | "left" | "right")
            }
            _ => {}
        }
    }
}

/// A length in em: `1.5em`, `12pt`, `16px`, `0`; percentages count against the font size.
fn length(value: &str, font_size: f32) -> Option<f32> {
    let number = |suffix: &str| {
        value
            .strip_suffix(suffix)
            .and_then(|n| n.trim().parse::<f32>().ok())
    };
    if value == "0" {
        return Some(0.0);
    }
    number("rem")
        .or_else(|| number("em"))
        .or_else(|| number("ex").map(|ex| ex / 2.0))
        .or_else(|| number("pt").map(|pt| pt / 12.0 / font_size))
        .or_else(|| number("px").map(|px| px / 16.0 / font_size))
        .or_else(|| number("%").map(|percent| percent / 100.0))
}

fn font_size(value: &str, parent: f32) -> Option<f32> {
    match value {
        "xx-small" => Some(0.6),
        "x-small" => Some(0.75),
        "small" => Some(0.89),
        "medium" => Some(1.0),
        "large" => Some(1.2),
        "x-large" => Some(1.5),
        "xx-large" => Some(2.0),
        "smaller" => Some(parent / 1.2),
        "larger" => Some(parent * 1.2),
        value => {
            let number = |suffix: &str| {
                value
                    .strip_suffix(suffix)
                    .and_then(|n| n.trim().parse::<f32>().ok())
            };
            number("rem")
                .or_else(|| number("em").map(|em| em * parent))
                .or_else(|| number("%").map(|percent| percent / 100.0 * parent))
                .or_else(|| number("pt").map(|pt| pt / 12.0))
                .or_else(|| number("px").map(|px| px / 16.0))
        }
    }
}
