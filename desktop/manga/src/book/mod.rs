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
}

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
    /// Line height as a multiple of the font size, when the book sets it.
    pub line_height: Option<f32>,
    /// The book wants this block on a new page.
    pub page_break_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Text(Vec<Run>),
    /// A picture: its path inside the book and what it shows in words.
    Image {
        src: String,
        alt: String,
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
    /// 0xRRGGBB, when the book sets one.
    pub color: Option<u32>,
    /// Where the text links to: a path inside the book with an optional `#id`, or a web address.
    pub link: Option<String>,
}

/// A book file the reader can open.
pub trait BookFile: Send {
    fn meta(&self) -> &Meta;
    /// How many chapters it has, in reading order.
    fn chapters(&self) -> usize;
    fn contents(&self) -> &[Entry];
    fn chapter(&mut self, index: usize) -> anyhow::Result<Chapter>;
    /// The bytes of a picture or other file inside the book.
    fn resource(&mut self, path: &str) -> anyhow::Result<Vec<u8>>;
    /// The path of the cover picture inside the book.
    fn cover(&self) -> Option<String>;
    /// Which chapter a link written in the book points into, and where in it.
    fn target(&self, link: &str) -> Option<(usize, Option<String>)>;
    /// Where in a chapter an anchor falls: the start, when it is not there.
    fn offset_of(&mut self, index: usize, anchor: &str) -> anyhow::Result<u32> {
        let chapter = self.chapter(index)?;
        Ok(chapter
            .anchors
            .iter()
            .find(|found| found.id == anchor)
            .map_or(0, |found| found.offset))
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
        std::fs::copy(path, &copy)?;
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
