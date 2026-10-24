//! EPUB: a zip with a package file that lists the book's files and the order to read them in.
//!
//! Opening reads three small files (the container, the package and the contents). A chapter is read when it is
//! asked for. Books locked with DRM are refused; the font scrambling the standard describes is not DRM.

use super::css::Sheet;
use super::html::{self, join};
use super::{BookFile, Chapter, Entry, Meta};
use anyhow::Context;
use regex::{Captures, Regex};
use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

const MAX_ENTRIES: usize = 20_000;
const MAX_CHAPTER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SPINE: usize = 5_000;

/// The two ways the standard scrambles embedded fonts. Anything else in `encryption.xml` is DRM.
const FONT_SCRAMBLING: [&str; 2] = [
    "http://www.idpf.org/2008/embedded-fonts",
    "http://ns.adobe.com/pdf/enc#RC",
];

pub struct Epub {
    zip: zip::ZipArchive<std::fs::File>,
    /// Real names in the zip by lower-case name, because books are sloppy about case.
    names: HashMap<String, String>,
    meta: Meta,
    /// The chapters' paths, in reading order.
    spine: Vec<String>,
    contents: Vec<Entry>,
    cover: Option<String>,
}

impl Epub {
    pub fn open(path: &Path) -> anyhow::Result<Epub> {
        let file =
            std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        let zip = zip::ZipArchive::new(file).context("that is not an EPUB file")?;
        anyhow::ensure!(
            zip.len() <= MAX_ENTRIES,
            "this book holds far more files than a book does"
        );
        let names = zip
            .file_names()
            .filter_map(Result::ok)
            .map(|name| (name.to_lowercase(), name.to_string()))
            .collect();
        let mut epub = Epub {
            zip,
            names,
            meta: Meta::default(),
            spine: Vec::new(),
            contents: Vec::new(),
            cover: None,
        };
        epub.refuse_drm()?;
        epub.read_package()?;
        Ok(epub)
    }

    fn refuse_drm(&mut self) -> anyhow::Result<()> {
        let Ok(bytes) = self.read("META-INF/encryption.xml", 1024 * 1024) else {
            return Ok(());
        };
        let text = String::from_utf8_lossy(&bytes);
        let locked = text
            .split("Algorithm=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .any(|algorithm| !FONT_SCRAMBLING.contains(&algorithm));
        anyhow::ensure!(
            !locked,
            "this book is protected by DRM, which cannot be opened here"
        );
        Ok(())
    }

    fn read(&mut self, path: &str, limit: u64) -> anyhow::Result<Vec<u8>> {
        let name = self
            .names
            .get(&path.to_lowercase())
            .with_context(|| format!("{path} is not in the book"))?
            .clone();
        let mut bytes = Vec::new();
        self.zip
            .by_name(&name)?
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() as u64 <= limit,
            "{path} is larger than a book's file should be"
        );
        Ok(bytes)
    }

    fn read_markup(&mut self, path: &str, limit: u64) -> anyhow::Result<Html> {
        Ok(parse(&self.read(path, limit)?))
    }

    fn read_package(&mut self) -> anyhow::Result<()> {
        let container = self.read_markup("META-INF/container.xml", 1024 * 1024)?;
        let package_path = container
            .select(&selector("rootfile"))
            .find_map(|root| root.value().attr("full-path"))
            .context("the book does not say where its contents are")?
            .to_string();
        let package = self.read_markup(&package_path, 8 * 1024 * 1024)?;

        // Every file the book lists, by its id.
        let mut manifest: HashMap<String, Item> = HashMap::new();
        for item in package.select(&selector("manifest item")) {
            let value = item.value();
            if let (Some(id), Some(href)) = (value.attr("id"), value.attr("href")) {
                manifest.insert(
                    id.to_string(),
                    Item {
                        path: join(&package_path, href),
                        kind: value.attr("media-type").unwrap_or("").to_string(),
                        properties: value.attr("properties").unwrap_or("").to_string(),
                    },
                );
            }
        }

        let spine_element = package.select(&selector("spine")).next();
        self.spine = package
            .select(&selector("spine itemref"))
            .filter_map(|itemref| manifest.get(itemref.value().attr("idref")?))
            .filter(|item| item.kind.contains("html"))
            .map(|item| item.path.clone())
            .take(MAX_SPINE)
            .collect();
        anyhow::ensure!(!self.spine.is_empty(), "the book has no chapters");

        // The details are `dc:title` and the like; a selector cannot name a prefix, so they are found by the
        // name after it.
        let details = |name: &str| -> Vec<String> {
            package
                .tree
                .nodes()
                .filter_map(ElementRef::wrap)
                .filter(|element| element.value().name().rsplit(':').next() == Some(name))
                .map(|element| element.text().collect::<String>().trim().to_string())
                .filter(|text| !text.is_empty())
                .collect()
        };
        self.meta = Meta {
            title: details("title").into_iter().next().unwrap_or_default(),
            authors: details("creator"),
            language: details("language").into_iter().next(),
            rtl: spine_element
                .and_then(|spine| spine.value().attr("page-progression-direction"))
                .is_some_and(|direction| direction == "rtl"),
        };

        self.cover = manifest
            .values()
            .find(|item| {
                item.properties
                    .split_whitespace()
                    .any(|property| property == "cover-image")
            })
            .or_else(|| {
                let id = package
                    .select(&selector("meta"))
                    .find(|meta| meta.value().attr("name") == Some("cover"))?
                    .value()
                    .attr("content")?;
                manifest.get(id)
            })
            .map(|item| item.path.clone());

        let nav = manifest
            .values()
            .find(|item| {
                item.properties
                    .split_whitespace()
                    .any(|property| property == "nav")
            })
            .map(|item| (item.path.clone(), true));
        let ncx = spine_element
            .and_then(|spine| spine.value().attr("toc"))
            .and_then(|id| manifest.get(id))
            .map(|item| (item.path.clone(), false));
        for (path, is_nav) in [nav, ncx].into_iter().flatten() {
            let Ok(document) = self.read_markup(&path, 4 * 1024 * 1024) else {
                continue;
            };
            let entries = if is_nav {
                self.nav_entries(&document, &path)
            } else {
                self.ncx_entries(&document, &path)
            };
            if !entries.is_empty() {
                self.contents = entries;
                break;
            }
        }
        if self.contents.is_empty() {
            self.contents = (0..self.spine.len())
                .map(|index| Entry {
                    title: format!("Part {}", index + 1),
                    depth: 0,
                    chapter: index as u32,
                    anchor: None,
                })
                .collect();
        }
        if self.meta.title.is_empty() {
            self.meta.title = Path::new(&package_path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        Ok(())
    }

    /// Where a link in a contents file goes: the chapter and the id in it.
    fn entry(&self, from: &str, href: &str, title: String, depth: u32) -> Option<Entry> {
        let (chapter, anchor) = self.target(&html::link_to(from, href))?;
        Some(Entry {
            title,
            depth,
            chapter: chapter as u32,
            anchor,
        })
    }

    fn nav_entries(&self, document: &Html, path: &str) -> Vec<Entry> {
        let navs: Vec<ElementRef> = document.select(&selector("nav")).collect();
        let Some(nav) = navs
            .iter()
            .find(|nav| {
                nav.value()
                    .attrs()
                    .any(|(key, value)| key.ends_with("type") && value.contains("toc"))
            })
            .or(navs.first())
        else {
            return Vec::new();
        };
        nav.select(&selector("a"))
            .filter_map(|link| {
                let depth = link
                    .ancestors()
                    .filter_map(ElementRef::wrap)
                    .take_while(|ancestor| ancestor.value().name() != "nav")
                    .filter(|ancestor| ancestor.value().name() == "ol")
                    .count()
                    .saturating_sub(1) as u32;
                self.entry(path, link.value().attr("href")?, words(&link), depth)
            })
            .collect()
    }

    fn ncx_entries(&self, document: &Html, path: &str) -> Vec<Entry> {
        document
            .select(&selector("navpoint"))
            .filter_map(|point| {
                let depth = point
                    .ancestors()
                    .filter_map(ElementRef::wrap)
                    .filter(|ancestor| ancestor.value().name() == "navpoint")
                    .count() as u32;
                // The label and link of this point; a point inside it comes later in the list.
                let label = point.select(&selector("navlabel text")).next()?;
                let content = point.select(&selector("content")).next()?;
                self.entry(path, content.value().attr("src")?, words(&label), depth)
            })
            .collect()
    }

    /// The stylesheets a chapter names, with the reader's own defaults underneath.
    fn sheet(&mut self, document: &Html, path: &str) -> Sheet {
        let mut sheet = Sheet::default();
        sheet.add_default(html::DEFAULTS);
        for element in document.select(&selector("link, style")) {
            if element.value().name() == "style" {
                sheet.add(&element.text().collect::<String>());
                continue;
            }
            let is_stylesheet = element
                .value()
                .attr("rel")
                .is_some_and(|rel| rel.to_lowercase().contains("stylesheet"));
            let href = element.value().attr("href").filter(|_| is_stylesheet);
            if let Some(Ok(bytes)) = href.map(|href| self.read(&join(path, href), 2 * 1024 * 1024))
            {
                sheet.add(&String::from_utf8_lossy(&bytes));
            }
        }
        sheet
    }
}

impl BookFile for Epub {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn chapters(&self) -> usize {
        self.spine.len()
    }

    fn contents(&self) -> &[Entry] {
        &self.contents
    }

    fn chapter(&mut self, index: usize) -> anyhow::Result<Chapter> {
        let path = self
            .spine
            .get(index)
            .context("there is no such chapter")?
            .clone();
        let document = self.read_markup(&path, MAX_CHAPTER_BYTES)?;
        let sheet = self.sheet(&document, &path);
        Ok(html::build(&document, &sheet, &path))
    }

    fn resource(&mut self, path: &str) -> anyhow::Result<Vec<u8>> {
        self.read(path, MAX_RESOURCE_BYTES)
    }

    fn cover(&self) -> Option<String> {
        self.cover.clone()
    }

    fn target(&self, link: &str) -> Option<(usize, Option<String>)> {
        let (path, anchor) = match link.split_once('#') {
            Some((path, anchor)) => (path, Some(anchor.to_string()).filter(|a| !a.is_empty())),
            None => (link, None),
        };
        let wanted = path.to_lowercase();
        let chapter = self
            .spine
            .iter()
            .position(|spine| spine.to_lowercase() == wanted)?;
        Some((chapter, anchor))
    }
}

struct Item {
    path: String,
    kind: String,
    properties: String,
}

fn selector(text: &str) -> Selector {
    Selector::parse(text).expect("a fixed selector")
}

fn words(element: &ElementRef) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Elements that have no end tag; every other `<x/>` is read as `<x></x>`, which is what the XML says and what the
/// HTML parser would otherwise get wrong.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

static SELF_CLOSED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<([A-Za-z][\w:.-]*)((?:[^<>"']|"[^"]*"|'[^']*')*?)/>"#).expect("a fixed pattern")
});

/// Markup as the HTML parser should see it.
fn parse(bytes: &[u8]) -> Html {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let text = SELF_CLOSED.replace_all(text, |found: &Captures| {
        let name = &found[1];
        if VOID.contains(&name.to_lowercase().as_str()) {
            found[0].to_string()
        } else {
            format!("<{name}{}></{name}>", &found[2])
        }
    });
    Html::parse_document(&text)
}
