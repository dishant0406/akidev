//! The part of CSS a book's look comes from: which rules apply to an element, as declarations in the order the
//! cascade applies them. Selectors are matched by `scraper`; this file only reads the stylesheet text.

use scraper::{ElementRef, Selector};
use std::collections::HashMap;

struct Rule {
    selector: Selector,
    /// Importance, then whose rule it is (the reader's own defaults lose to the book's), then how specific the
    /// selector is, then position: the cascade order.
    rank: (bool, u8, u32, u32),
    declarations: Vec<(String, String)>,
}

/// Every rule of a chapter's stylesheets, indexed by the part of the selector that is cheapest to look up.
#[derive(Default)]
pub struct Sheet {
    rules: Vec<Rule>,
    by_tag: HashMap<String, Vec<usize>>,
    by_class: HashMap<String, Vec<usize>>,
    by_id: HashMap<String, Vec<usize>>,
    anything: Vec<usize>,
    /// The fonts the book carries, as its stylesheets name them.
    pub faces: Vec<Face>,
}

/// A font the book carries: the family name its CSS uses for it, where the file is in the book, and which style it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub family: String,
    pub path: String,
    pub bold: bool,
    pub italic: bool,
}

impl Sheet {
    /// A rule of the book's own, from the stylesheet at `at` in the book (the fonts it names are found from there).
    pub fn add(&mut self, css: &str, at: &str) {
        self.add_from(css, 1, at);
    }

    /// The reader's defaults for elements, which any rule of the book overrides.
    pub fn add_default(&mut self, css: &str) {
        self.add_from(css, 0, "");
    }

    fn add_from(&mut self, css: &str, origin: u8, at: &str) {
        let css = strip_statements(&strip_comments(css));
        let mut rest = css.as_str();
        while let Some(open) = rest.find('{') {
            let prelude = rest[..open].trim();
            let Some(close) = matching_brace(rest, open) else {
                break;
            };
            let body = &rest[open + 1..close];
            if prelude.starts_with('@') {
                // @media and the like are not applied; @font-face says which fonts the book carries.
                if prelude.starts_with("@font-face") {
                    self.add_face(body, at);
                }
                rest = &rest[close + 1..];
                continue;
            }
            let declarations = declarations(body);
            for selector in prelude.split(',') {
                self.push(selector.trim(), &declarations, origin);
            }
            rest = &rest[close + 1..];
        }
    }

    fn add_face(&mut self, body: &str, at: &str) {
        let declarations = declarations(body);
        let value = |name: &str| {
            declarations
                .iter()
                .rev()
                .find(|(n, _, _)| n == name)
                .map(|(_, v, _)| v.to_lowercase())
        };
        let family = declarations
            .iter()
            .rev()
            .find(|(n, _, _)| n == "font-family")
            .map(|(_, v, _)| first_family(v));
        let src = declarations
            .iter()
            .rev()
            .find(|(n, _, _)| n == "src")
            .and_then(|(_, v, _)| first_url(v));
        if let (Some(Some(family)), Some(src)) = (family, src) {
            self.faces.push(Face {
                family,
                path: super::html::join(at, &src),
                bold: value("font-weight")
                    .is_some_and(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600)),
                italic: value("font-style").is_some_and(|s| s == "italic" || s == "oblique"),
            });
        }
    }

    fn push(&mut self, text: &str, declarations: &[(String, String, bool)], origin: u8) {
        if text.is_empty() || text.contains("::") {
            return;
        }
        let Ok(selector) = Selector::parse(text) else {
            return;
        };
        let index = self.rules.len();
        let key = key_of(text);
        // One rule per importance, so `!important` declarations rank above the rest.
        for important in [false, true] {
            let these: Vec<(String, String)> = declarations
                .iter()
                .filter(|(_, _, flag)| *flag == important)
                .map(|(name, value, _)| (name.clone(), value.clone()))
                .collect();
            if these.is_empty() {
                continue;
            }
            let id = self.rules.len();
            self.rules.push(Rule {
                selector: selector.clone(),
                rank: (important, origin, specificity(text), index as u32),
                declarations: these,
            });
            match &key {
                Key::Id(name) => self.by_id.entry(name.clone()).or_default().push(id),
                Key::Class(name) => self.by_class.entry(name.clone()).or_default().push(id),
                Key::Tag(name) => self.by_tag.entry(name.clone()).or_default().push(id),
                Key::Any => self.anything.push(id),
            }
        }
    }

    /// What the stylesheets say about `element`, weakest first.
    pub fn declarations(&self, element: &ElementRef) -> Vec<(String, String)> {
        let value = element.value();
        let mut candidates: Vec<usize> = self.anything.clone();
        if let Some(rules) = self.by_tag.get(value.name()) {
            candidates.extend(rules);
        }
        for class in value.classes() {
            if let Some(rules) = self.by_class.get(class) {
                candidates.extend(rules);
            }
        }
        if let Some(rules) = value.id().and_then(|id| self.by_id.get(id)) {
            candidates.extend(rules);
        }
        let mut matched: Vec<&Rule> = candidates
            .into_iter()
            .map(|index| &self.rules[index])
            .filter(|rule| rule.selector.matches(element))
            .collect();
        matched.sort_by_key(|rule| rule.rank);
        matched
            .into_iter()
            .flat_map(|rule| rule.declarations.iter().cloned())
            .collect()
    }
}

/// The name of the first font in a `font-family` value, without quotes; nothing for the generic names, which name no font file.
pub fn first_family(value: &str) -> Option<String> {
    let first = value
        .split(',')
        .next()?
        .trim()
        .trim_matches(['"', '\''])
        .trim();
    let generic = [
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "inherit",
        "initial",
        "unset",
        "ui-serif",
        "ui-sans-serif",
        "ui-monospace",
    ];
    (!first.is_empty() && !generic.contains(&first.to_lowercase().as_str()))
        .then(|| first.to_string())
}

/// The address in the first `url(...)` of an `src` value.
fn first_url(value: &str) -> Option<String> {
    let start = value.find("url(")? + 4;
    let end = start + value[start..].find(')')?;
    let address = value[start..end].trim().trim_matches(['"', '\'']);
    (!address.is_empty() && !address.starts_with("data:")).then(|| address.to_string())
}

/// `color: red; margin: 0 !important` as (name, value, important).
pub fn declarations(text: &str) -> Vec<(String, String, bool)> {
    text.split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            let value = value.trim();
            let (value, important) = match value.strip_suffix("!important") {
                Some(value) => (value.trim(), true),
                None => (value, false),
            };
            Some((name.trim().to_lowercase(), value.to_string(), important))
        })
        .collect()
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Drops `@import`, `@charset` and `@namespace`, the at-rules that end in `;` instead of a block.
fn strip_statements(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find('@') {
        out.push_str(&rest[..start]);
        let at = &rest[start..];
        let statement = ["@import", "@charset", "@namespace"]
            .iter()
            .any(|name| at.starts_with(name));
        rest = if statement {
            at.find(';').map_or("", |end| &at[end + 1..])
        } else {
            out.push('@');
            &at[1..]
        };
    }
    out.push_str(rest);
    out
}

/// The index of the `}` that closes the `{` at `open`.
fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    for (index, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }
    None
}

enum Key {
    Id(String),
    Class(String),
    Tag(String),
    Any,
}

/// What the last part of the selector (the element itself) names, to look rules up by.
fn key_of(selector: &str) -> Key {
    let last = selector
        .rsplit(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~')
        .next()
        .unwrap_or("");
    // Anything after a `:` or `[` is a condition, and could be about something else (`:not(.x)`).
    let compound = last.split([':', '[']).next().unwrap_or("");
    let name = |from: char| -> Option<String> {
        let start = compound.find(from)? + 1;
        let end = compound[start..]
            .find(['.', '#'])
            .map_or(compound.len(), |end| start + end);
        Some(compound[start..end].to_string())
    };
    if let Some(id) = name('#') {
        return Key::Id(id);
    }
    if let Some(class) = name('.') {
        return Key::Class(class);
    }
    let tag: String = compound
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    if tag.is_empty() {
        Key::Any
    } else {
        // Element names reach the matcher lower case.
        Key::Tag(tag.to_lowercase())
    }
}

fn specificity(selector: &str) -> u32 {
    let mut ids = 0;
    let mut classes = 0;
    let mut tags = 0;
    let mut at_start = true;
    for c in selector.chars() {
        match c {
            '#' => ids += 1,
            '.' | '[' => classes += 1,
            ':' => classes += 1,
            c if c.is_whitespace() || c == '>' || c == '+' || c == '~' => {
                at_start = true;
                continue;
            }
            c if at_start && (c.is_alphabetic()) => tags += 1,
            _ => {}
        }
        at_start = false;
    }
    ids * 10_000 + classes * 100 + tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    fn first<'a>(html: &'a Html, selector: &str) -> ElementRef<'a> {
        html.select(&Selector::parse(selector).unwrap())
            .next()
            .unwrap()
    }

    #[test]
    fn more_specific_rules_win_and_important_beats_all() {
        let mut sheet = Sheet::default();
        sheet.add("p { color: red } p.note { color: blue } /* c */ #x { color: green } .note { margin: 0 !important } p { margin: 1em }", "");
        let html = Html::parse_fragment(r#"<p class="note" id="x">hi</p>"#);
        let found = sheet.declarations(&first(&html, "p"));
        let last = |name: &str| {
            found
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(last("color"), Some("green"));
        assert_eq!(last("margin"), Some("0"));
    }

    #[test]
    fn at_rules_and_unmatchable_selectors_are_skipped() {
        let mut sheet = Sheet::default();
        sheet.add("@import url(a.css); @media print { p { color: red } } @font-face { font-family: X; src: url(../fonts/X.otf) } p::first-letter { color: red } ns|p { color: red } div p { color: blue }", "");
        let html = Html::parse_fragment("<div><p>hi</p></div><p id=\"b\">no</p>");
        let in_div = sheet.declarations(&first(&html, "div p"));
        assert_eq!(in_div, [("color".to_string(), "blue".to_string())]);
        assert!(sheet.declarations(&first(&html, "#b")).is_empty());
    }

    #[test]
    fn fonts_the_book_carries_are_found_from_the_stylesheet_that_names_them() {
        let mut sheet = Sheet::default();
        sheet.add(
            "@font-face { font-family: 'Fancy Face'; font-weight: bold; src: url('../fonts/Fancy%20Bold.otf') format('opentype') } @font-face { font-family: Plain; font-style: italic; src: local(x), url(p.ttf) }",
            "OEBPS/css/book.css",
        );
        assert_eq!(
            sheet.faces,
            [
                Face {
                    family: "Fancy Face".into(),
                    path: "OEBPS/fonts/Fancy Bold.otf".into(),
                    bold: true,
                    italic: false
                },
                Face {
                    family: "Plain".into(),
                    path: "OEBPS/css/p.ttf".into(),
                    bold: false,
                    italic: true
                },
            ]
        );
        assert_eq!(
            first_family("\"Fancy Face\", serif").as_deref(),
            Some("Fancy Face")
        );
        assert_eq!(first_family("Georgia, serif").as_deref(), Some("Georgia"));
        assert_eq!(first_family("serif"), None);
    }

    #[test]
    fn a_not_condition_does_not_become_the_lookup_key() {
        let mut sheet = Sheet::default();
        sheet.add("p:not(.x) { color: red }", "");
        let html = Html::parse_fragment("<p>hi</p>");
        assert_eq!(sheet.declarations(&first(&html, "p")).len(), 1);
    }
}
