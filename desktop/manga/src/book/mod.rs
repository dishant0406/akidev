//! Books: a file on the device turned into text the reader can lay out.
//!
//! A format (today only EPUB) implements `BookFile`: it knows how to list its chapters and contents and how to
//! turn one chapter into a `Chapter`, the one shape every renderer reads. The renderer never sees a format, so a
//! new format is one new implementation and nothing else changes.
//!
//! Memory: opening a book reads its table of contents and nothing else. A chapter is read, laid out and dropped
//! on request, so a large book costs what its current chapter costs.

mod css;
mod epub;
mod html;
pub mod opds;

use std::path::Path;

/// What a book says about itself.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    pub title: String,
    pub authors: Vec<String>,
    pub language: Option<String>,
    /// The book reads right to left.
    pub rtl: bool,
}

/// One line of the table of contents.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub title: String,
    /// 0 for a top-level entry.
    pub depth: u32,
    /// The chapter it opens, counting from 0 in reading order.
    pub chapter: u32,
    /// The place in that chapter, when it is not the start: the id of an `Anchor`.
    pub anchor: Option<String>,
}

/// A chapter, as blocks of text and pictures in reading order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chapter {
    pub blocks: Vec<Block>,
    /// Places that links can point at: an id in the file and where it falls in the text.
    pub anchors: Vec<Anchor>,
    /// The fonts the book carries that this chapter's styles name.
    pub fonts: Vec<Face>,
}

pub use css::Face;

#[derive(Debug, Clone, PartialEq)]
pub struct Anchor {
    pub id: String,
    pub offset: u32,
}

/// Positions in a chapter are counted in UTF-16 units over its text, where every block ends with one newline: the
/// unit Swift strings count in, so the same number means the same place on both sides.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub start: u32,
    pub kind: Kind,
    /// A heading level from 1 to 6; 0 for anything else.
    pub heading: u8,
    pub align: Align,
    /// Distance from the left edge, and the extra shift of the first line (negative hangs a list marker), in em.
    pub left: f32,
    pub first_line: f32,
    /// Space above, in em: already the larger of the one above and the one below, as CSS joins them.
    pub space_before: f32,
    /// The book wants this block on a new page.
    pub page_break_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Text(Vec<Run>),
    /// A picture: its path inside the book, and whether it is dark ink on a clear background
    /// (a title page, an ornament), which must be drawn in the text's colour to be seen on a dark page.
    Image {
        src: String,
        ink: bool,
    },
    /// A horizontal rule.
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Justify,
}

/// A stretch of text that looks one way.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub text: String,
    pub look: Look,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub small_caps: bool,
    pub mono: bool,
    /// Raised or lowered, as for footnote marks: 1, -1 or 0.
    pub shift: i8,
    /// Size relative to the reader's body size.
    pub size: f32,
    /// The book's own font for this text, when it names one other than the font its body text uses.
    pub family: Option<String>,
    /// Where the text links to: a path inside the book with an optional `#id`, or a web address.
    pub link: Option<String>,
}

/// A book file the reader can open.
pub trait BookFile: Send {
    fn meta(&self) -> &Meta;
    /// How many chapters it has, in reading order.
    fn chapters(&self) -> usize;
    fn contents(&self) -> &[Entry];
    /// How big each chapter is, in bytes of its file: the weights for "how far through the book", read without
    /// reading the chapters.
    fn sizes(&self) -> &[u64];
    fn chapter(&mut self, index: usize) -> anyhow::Result<Chapter>;
    /// The bytes of a picture or other file inside the book.
    fn resource(&mut self, path: &str) -> anyhow::Result<Vec<u8>>;
    /// The path of the cover picture inside the book.
    fn cover(&self) -> Option<String>;
    /// Which chapter a link written in the book points into, and where in it.
    fn target(&mut self, link: &str) -> Option<(usize, Option<String>)>;

    /// Every place in one chapter where `query` is written, ignoring case.
    fn search(&mut self, index: usize, query: &str) -> anyhow::Result<Vec<Hit>> {
        let needle: Vec<char> = query.trim().chars().map(fold).collect();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let chapter = self.chapter(index)?;
        let mut hits = Vec::new();
        for block in &chapter.blocks {
            let Kind::Text(runs) = &block.kind else {
                continue;
            };
            let chars: Vec<char> = runs.iter().flat_map(|run| run.text.chars()).collect();
            let folded: Vec<char> = chars.iter().copied().map(fold).collect();
            let mut at = 0;
            while at + needle.len() <= folded.len() {
                if folded[at..at + needle.len()] != needle[..] {
                    at += 1;
                    continue;
                }
                let before: usize = chars[..at].iter().map(|c| c.len_utf16()).sum();
                hits.push(Hit {
                    chapter: index as u32,
                    offset: block.start + before as u32,
                    snippet: snippet(&chars, at, needle.len()),
                });
                at += needle.len();
            }
        }
        Ok(hits)
    }

    /// The text of the note a link points at, when it points at a short passage that is not a heading: what a footnote
    /// mark shows in a card instead of taking the reader away.
    fn note(&mut self, link: &str) -> Option<String> {
        let (index, anchor) = self.target(link)?;
        let chapter = self.chapter(index).ok()?;
        let offset = chapter
            .anchors
            .iter()
            .find(|found| found.id == anchor.as_deref().unwrap_or(""))?
            .offset;
        let next = chapter
            .anchors
            .iter()
            .map(|found| found.offset)
            .filter(|&at| at > offset)
            .min()
            .unwrap_or(u32::MAX);
        let first = chapter
            .blocks
            .partition_point(|block| block.start <= offset)
            .checked_sub(1)?;
        let mut text = String::new();
        for block in &chapter.blocks[first..] {
            let Kind::Text(runs) = &block.kind else {
                break;
            };
            if block.heading > 0 || (!text.is_empty() && block.start >= next) {
                break;
            }
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.extend(runs.iter().map(|run| run.text.as_str()));
            if text.chars().count() > NOTE_LIMIT {
                return None;
            }
        }
        let text = clean_note(&text);
        Some(text).filter(|text| !text.is_empty())
    }
}

/// A note longer than this is a chapter, not a note.
const NOTE_LIMIT: usize = 700;

/// One place a search found: the chapter, the position in it, and the words around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub chapter: u32,
    pub offset: u32,
    pub snippet: String,
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The words around a match, about a line's worth, with … where it is cut.
fn snippet(chars: &[char], at: usize, length: usize) -> String {
    let from = at.saturating_sub(32);
    let to = (at + length + 48).min(chars.len());
    let text: String = chars[from..to]
        .iter()
        .map(|c| if *c == '\u{2028}' { ' ' } else { *c })
        .collect();
    format!(
        "{}{}{}",
        if from > 0 { "…" } else { "" },
        text.trim(),
        if to < chars.len() { "…" } else { "" }
    )
}

/// A note's text without its number ("1.", "[2]", "*") in front or the way back (↩) behind.
fn clean_note(text: &str) -> String {
    let text = text
        .trim()
        .trim_end_matches(['↩', '\u{fe0e}', '\u{fe0f}', ' '])
        .trim();
    let (first, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let is_mark = first.chars().count() <= 5
        && first.chars().all(|c| {
            c.is_ascii_digit() || matches!(c, '[' | ']' | '.' | ')' | '(' | '*' | '†' | '‡')
        });
    if is_mark && !rest.trim().is_empty() {
        rest.trim().to_string()
    } else {
        text.to_string()
    }
}

/// Opens the book at `path`, by what kind of file it is.
pub fn open(path: &Path) -> anyhow::Result<Box<dyn BookFile>> {
    let name = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    match name.to_lowercase().as_str() {
        "epub" => Ok(Box::new(epub::Epub::open(path)?)),
        _ => anyhow::bail!("this kind of file cannot be read as a book yet"),
    }
}

/// A book added to the device.
#[derive(Debug, PartialEq)]
pub struct Imported {
    /// A hash of the file, so the same file added twice is one book.
    pub id: String,
    /// The copy's name in the books folder.
    pub file: String,
    /// The cover's name in the covers folder.
    pub cover: Option<String>,
    pub meta: Meta,
    pub bytes: u64,
}

/// Copies the book at `path` into `books_dir`, checks it can be read and puts its cover in `covers_dir`. A file
/// that cannot be read as a book leaves nothing behind.
pub fn import(path: &Path, books_dir: &Path, covers_dir: &Path) -> anyhow::Result<Imported> {
    use anyhow::Context;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    anyhow::ensure!(extension == "epub", "only EPUB books can be added for now");
    let mut source =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut bytes = 0u64;
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        bytes += read as u64;
    }
    let id: String = hash
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = format!("{id}.{extension}");
    std::fs::create_dir_all(books_dir)?;
    let copy = books_dir.join(&file);
    let is_new = !copy.exists();
    if is_new {
        // Copied under another name first, so a copy cut short is never taken for the book.
        let partial = books_dir.join(format!("{file}.partial"));
        std::fs::copy(path, &partial)?;
        std::fs::rename(&partial, &copy)?;
    }
    let read = (|| {
        let mut book = open(&copy)?;
        let meta = book.meta().clone();
        let cover = match book.cover().map(|path| (book.resource(&path), path)) {
            Some((Ok(picture), path)) => {
                let ext = Path::new(&path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("jpg")
                    .to_lowercase();
                let name = format!("book-{id}.{ext}");
                std::fs::create_dir_all(covers_dir)?;
                std::fs::write(covers_dir.join(&name), picture)?;
                Some(name)
            }
            _ => None,
        };
        anyhow::Ok((meta, cover))
    })();
    match read {
        Ok((mut meta, cover)) => {
            if meta.title.is_empty() {
                meta.title = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            Ok(Imported {
                id,
                file,
                cover,
                meta,
                bytes,
            })
        }
        Err(error) => {
            if is_new {
                let _ = std::fs::remove_file(&copy);
            }
            Err(error)
        }
    }
}
