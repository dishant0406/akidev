//! EPUB: a zip with a package file that lists the book's files and the order to read them in.
//!
//! Opening reads three small files (the container, the package and the contents). A chapter is read when it is
//! asked for. Books locked with DRM are refused; the font scrambling the standard describes is not DRM.

use super::css::Sheet;
use super::html::{self, join};
use super::{Anchor, Block, BookFile, Chapter, Entry, Meta};
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
/// A chapter file larger than this is read as several chapters of about this size, cut between paragraphs, so a book that is
/// one enormous file is still set in type a piece at a time.
const PIECE_BYTES: u64 = 96 * 1024;

pub struct Epub {
    zip: zip::ZipArchive<std::fs::File>,
    /// Real names in the zip by lower-case name, because books are sloppy about case.
    names: HashMap<String, String>,
    meta: Meta,
    /// The chapters' paths, in reading order.
    spine: Vec<Part>,
    /// The last large file read, whole, while its pieces are being asked for one after another.
    whole: Option<(String, Chapter)>,
    sizes: Vec<u64>,
    /// Where each file's first chapter is, by lower-case path.
    first: HashMap<String, usize>,
    contents: Vec<Entry>,
    cover: Option<String>,
    /// How the book's files that are scrambled are scrambled (fonts, by the standard's two ways), by lower-case path.
    scrambled: HashMap<String, Scrambling>,
    /// The book's identifier, which the scrambling is keyed with.
    identifier: String,
}

#[derive(Clone, Copy)]
enum Scrambling {
    /// The first 1040 bytes, with the SHA-1 of the identifier.
    Idpf,
    /// The first 1024 bytes, with the identifier read as a UUID.
    Adobe,
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
            whole: None,
            sizes: Vec::new(),
            first: HashMap::new(),
            contents: Vec::new(),
            cover: None,
            scrambled: HashMap::new(),
            identifier: String::new(),
        };
        epub.refuse_drm()?;
        epub.read_package()?;
        Ok(epub)
    }

    /// Notes which files are scrambled fonts, and refuses a book whose files are locked in any other way.
    fn refuse_drm(&mut self) -> anyhow::Result<()> {
        let Ok(bytes) = self.read("META-INF/encryption.xml", 1024 * 1024) else {
            return Ok(());
        };
        let document = parse(&bytes);
        let mut locked = false;
        for data in document.select(&selector("encrypteddata")) {
            let algorithm = data
                .select(&selector("encryptionmethod"))
                .next()
                .and_then(|method| method.value().attr("algorithm"))
                .unwrap_or("");
            let scrambling = match algorithm {
                "http://www.idpf.org/2008/embedded-fonts" => Scrambling::Idpf,
                "http://ns.adobe.com/pdf/enc#RC" => Scrambling::Adobe,
                _ => {
                    locked = true;
                    continue;
                }
            };
            if let Some(uri) = data
                .select(&selector("cipherreference"))
                .next()
                .and_then(|cipher| cipher.value().attr("uri"))
            {
                self.scrambled
                    .insert(html::join("", uri).to_lowercase(), scrambling);
            }
        }
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
        // The identifier the package points at as its unique one.
        let unique = package
            .select(&selector("package"))
            .next()
            .and_then(|package| package.value().attr("unique-identifier"));
        self.identifier = package
            .tree
            .nodes()
            .filter_map(ElementRef::wrap)
            .filter(|element| element.value().name().rsplit(':').next() == Some("identifier"))
            .find(|element| unique.is_none_or(|id| element.value().attr("id") == Some(id)))
            .map(|element| element.text().collect::<String>())
            .unwrap_or_default();
        let paths: Vec<String> = package
            .select(&selector("spine itemref"))
            .filter_map(|itemref| manifest.get(itemref.value().attr("idref")?))
            .filter(|item| item.kind.contains("html"))
            .map(|item| item.path.clone())
            .take(MAX_SPINE)
            .collect();
        anyhow::ensure!(!paths.is_empty(), "the book has no chapters");
        for path in paths {
            let size = self
                .names
                .get(&path.to_lowercase())
                .and_then(|name| self.zip.by_name(name).ok())
                .map_or(0, |file| file.size());
            // A file too large to read at all is one chapter that says so, not hundreds of pieces.
            let pieces = if size > MAX_CHAPTER_BYTES {
                1
            } else {
                size.div_ceil(PIECE_BYTES).max(1) as usize
            };
            self.first
                .entry(path.to_lowercase())
                .or_insert(self.spine.len());
            for piece in 0..pieces {
                self.spine.push(Part {
                    path: path.clone(),
                    piece,
                    pieces,
                });
                self.sizes.push(size / pieces as u64);
            }
        }
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
            // A cover that is a page of markup is not a picture to show.
            .filter(|item| item.kind.starts_with("image/"))
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
    fn entry(&mut self, from: &str, href: &str, title: String, depth: u32) -> Option<Entry> {
        let (chapter, anchor) = self.target(&html::link_to(from, href))?;
        Some(Entry {
            title,
            depth,
            chapter: chapter as u32,
            anchor,
        })
    }

    fn nav_entries(&mut self, document: &Html, path: &str) -> Vec<Entry> {
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

    fn ncx_entries(&mut self, document: &Html, path: &str) -> Vec<Entry> {
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

    fn parse_file(&mut self, path: &str) -> anyhow::Result<Chapter> {
        let document = self.read_markup(path, MAX_CHAPTER_BYTES)?;
        let sheet = self.sheet(&document, path);
        let mut chapter = html::build(&document, &sheet, path);
        chapter.fonts = sheet.faces;
        Ok(chapter)
    }

    /// Reads a large file whole, unless it was the last one read.
    fn load_whole(&mut self, path: &str) -> anyhow::Result<()> {
        if self.whole.as_ref().is_none_or(|(kept, _)| kept != path) {
            self.whole = None;
            let chapter = self.parse_file(path)?;
            self.whole = Some((path.to_string(), chapter));
        }
        Ok(())
    }

    /// The stylesheets a chapter names, with the reader's own defaults underneath.
    fn sheet(&mut self, document: &Html, path: &str) -> Sheet {
        let mut sheet = Sheet::default();
        sheet.add_default(html::DEFAULTS);
        for element in document.select(&selector("link, style")) {
            if element.value().name() == "style" {
                sheet.add(&element.text().collect::<String>(), path);
                continue;
            }
            let is_stylesheet = element
                .value()
                .attr("rel")
                .is_some_and(|rel| rel.to_lowercase().contains("stylesheet"));
            let Some(href) = element.value().attr("href").filter(|_| is_stylesheet) else {
                continue;
            };
            let stylesheet = join(path, href);
            if let Ok(bytes) = self.read(&stylesheet, 2 * 1024 * 1024) {
                sheet.add(&String::from_utf8_lossy(&bytes), &stylesheet);
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

    fn sizes(&self) -> &[u64] {
        &self.sizes
    }

    fn chapter(&mut self, index: usize) -> anyhow::Result<Chapter> {
        let part = self
            .spine
            .get(index)
            .context("there is no such chapter")?
            .clone();
        if part.pieces == 1 {
            return self.parse_file(&part.path);
        }
        self.load_whole(&part.path)?;
        let whole = &self.whole.as_ref().expect("just loaded").1;
        Ok(slice(whole, part.piece, part.pieces))
    }

    fn resource(&mut self, path: &str) -> anyhow::Result<Vec<u8>> {
        let mut bytes = self.read(path, MAX_RESOURCE_BYTES)?;
        if let Some(scrambling) = self.scrambled.get(&path.to_lowercase()) {
            unscramble(&mut bytes, *scrambling, &self.identifier);
        }
        Ok(bytes)
    }

    fn cover(&self) -> Option<String> {
        self.cover.clone()
    }

    fn target(&mut self, link: &str) -> Option<(usize, Option<String>)> {
        let (path, anchor) = match link.split_once('#') {
            Some((path, anchor)) => (path, Some(anchor.to_string()).filter(|a| !a.is_empty())),
            None => (link, None),
        };
        let wanted = path.to_lowercase();
        let first = *self.first.get(&wanted)?;
        let pieces = self.spine[first].pieces;
        let Some(id) = anchor.as_ref().filter(|_| pieces > 1) else {
            return Some((first, anchor));
        };
        // The id is in one of the pieces of a large file: the one it falls in.
        let path = self.spine[first].path.clone();
        self.load_whole(&path).ok()?;
        let whole = &self.whole.as_ref()?.1;
        let offset = whole
            .anchors
            .iter()
            .find(|a| &a.id == id)
            .map_or(0, |a| a.offset);
        let starts = piece_starts(whole, pieces);
        let piece = (0..pieces)
            .rev()
            .find(|&p| starts[p] <= offset)
            .unwrap_or(0);
        Some((first + piece, anchor))
    }
}

/// Undoes the standard's scrambling of a font file in place: the start of the file is XORed with a key made from the book's
/// identifier, so a font cannot be lifted out of the book and used on its own.
fn unscramble(bytes: &mut [u8], how: Scrambling, identifier: &str) {
    use sha1::{Digest, Sha1};
    let (key, length): (Vec<u8>, usize) = match how {
        Scrambling::Idpf => {
            let clean: String = identifier
                .chars()
                .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
                .collect();
            (Sha1::digest(clean.as_bytes()).to_vec(), 1040)
        }
        Scrambling::Adobe => {
            // The identifier's hexadecimal digits, two to a byte; anything else in it is left out.
            let digits: Vec<u8> = identifier
                .to_lowercase()
                .trim_start_matches("urn:uuid:")
                .chars()
                .filter_map(|c| c.to_digit(16).map(|digit| digit as u8))
                .collect();
            let key: Vec<u8> = digits
                .chunks_exact(2)
                .map(|pair| pair[0] * 16 + pair[1])
                .collect();
            // A key that is not a whole UUID is no key: the font is left as it is.
            (if key.len() == 16 { key } else { Vec::new() }, 1024)
        }
    };
    if key.is_empty() {
        return;
    }
    for (index, byte) in bytes.iter_mut().take(length).enumerate() {
        *byte ^= key[index % key.len()];
    }
}

/// One chapter of the reading order: a whole file, or one piece of a large one.
#[derive(Clone)]
struct Part {
    path: String,
    piece: usize,
    pieces: usize,
}

/// Where each piece of a chapter begins, as an offset into its text: the first paragraph at or after an even share of it.
fn piece_starts(whole: &Chapter, pieces: usize) -> Vec<u32> {
    let total = whole.blocks.last().map_or(0, |block| block.start + 1);
    (0..pieces)
        .map(|piece| {
            let wanted = (u64::from(total) * piece as u64 / pieces as u64) as u32;
            let at = whole.blocks.partition_point(|block| block.start < wanted);
            whole.blocks.get(at).map_or(total, |block| block.start)
        })
        .collect()
}

/// One piece of a large chapter, its positions counted from its own start.
fn slice(whole: &Chapter, piece: usize, pieces: usize) -> Chapter {
    let starts = piece_starts(whole, pieces);
    let from = if piece == 0 { 0 } else { starts[piece] };
    let to = starts.get(piece + 1).copied().unwrap_or(u32::MAX);
    let first = if piece == 0 { 0 } else { from };
    Chapter {
        blocks: whole
            .blocks
            .iter()
            .filter(|block| block.start >= from && block.start < to)
            .map(|block| Block {
                start: block.start - first,
                ..block.clone()
            })
            .collect(),
        anchors: whole
            .anchors
            .iter()
            .filter(|anchor| anchor.offset >= from && anchor.offset < to)
            .map(|anchor| Anchor {
                id: anchor.id.clone(),
                offset: anchor.offset - first,
            })
            .collect(),
        fonts: whole.fonts.clone(),
        vertical: whole.vertical,
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
pub(super) fn parse(bytes: &[u8]) -> Html {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_odd_identifier_leaves_a_font_alone_instead_of_failing() {
        let font = vec![7u8; 2000];
        for identifier in [
            "urn:uuid:é–✓-not-hex",
            "",
            "URN:UUID:123e4567-e89b-12d3-a456-426614174000x",
        ] {
            let mut bytes = font.clone();
            unscramble(&mut bytes, Scrambling::Adobe, identifier);
            assert_eq!(bytes.len(), font.len());
        }
        // A whole UUID, in any case, makes a key.
        let mut upper = font.clone();
        unscramble(
            &mut upper,
            Scrambling::Adobe,
            "URN:UUID:123E4567-E89B-12D3-A456-426614174000",
        );
        let mut lower = font.clone();
        unscramble(
            &mut lower,
            Scrambling::Adobe,
            "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        );
        assert_eq!(upper, lower);
        assert_ne!(upper, font);
        assert_eq!(upper[1024..], font[1024..]);
    }
}
