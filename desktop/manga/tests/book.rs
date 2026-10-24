//! Reads books made in the test, the way the reader reads a file from the device.

use akidev_manga::book::{self, Align, Kind};
use std::io::Write;
use std::path::PathBuf;

const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

const PACKAGE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>The Test Book</dc:title>
    <dc:creator>Ada Writer</dc:creator>
    <dc:creator>Bo Editor</dc:creator>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style/book.css" media-type="text/css"/>
    <item id="cover" href="images/Cover.jpg" media-type="image/jpeg" properties="cover-image"/>
    <item id="c1" href="text/one.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="text/two.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine page-progression-direction="ltr">
    <itemref idref="c1"/>
    <itemref idref="c2"/>
  </spine>
</package>"#;

const NAV: &str = r##"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
  <nav epub:type="landmarks"><ol><li><a href="text/one.xhtml">Start</a></li></ol></nav>
  <nav epub:type="toc"><ol>
    <li><a href="text/one.xhtml">One</a>
      <ol><li><a href="text/one.xhtml#second">A part of one</a></li></ol></li>
    <li><a href="text/two.xhtml">Two</a></li>
  </ol></nav>
</body></html>"##;

const CSS: &str = "
/* the book's own look */
body { font-family: serif }
p { margin: 0; text-indent: 1.5em; text-align: justify }
p.first { text-indent: 0 }
.quiet { font-size: 0.8em; font-style: italic }
.hidden { display: none }
h1 { text-align: center; page-break-before: always }
";

const ONE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>One</title>
<link rel="stylesheet" type="text/css" href="../style/book.css"/></head>
<body>
<h1 id="top">Chapter   One</h1>
<p class="first">It was <b>a dark</b> and <i>stormy</i>
night.<a id="mark"/> Caf&#233;.<sup><a href="two.xhtml#note1">1</a></sup></p>
<p>Second<br/>line, with a <span class="quiet">quiet</span> word<span class="hidden"> unseen</span>.</p>
<h2 id="second">Part two</h2>
<ul><li>apples</li><li>pears</li></ul>
<ol><li>first</li><li>second</li></ol>
<p><img src="../images/Pic%201.png" alt="A picture"/></p>
<table><tr><td>a</td><td>b</td></tr></table>
<p>The end. 😀 ok</p>
</body></html>"##;

const TWO: &str = r##"<html xmlns="http://www.w3.org/1999/xhtml"><body>
<p id="note1">A note.</p>
<p>Unclosed <b>bold
</body></html>"##;

fn write_book(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aki-book-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (file, content) in files {
        zip.start_file(*file, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    path
}

fn standard(name: &str) -> PathBuf {
    write_book(
        name,
        &[
            ("mimetype", "application/epub+zip"),
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", PACKAGE),
            ("OEBPS/nav.xhtml", NAV),
            ("OEBPS/style/book.css", CSS),
            ("OEBPS/images/Cover.jpg", "cover bytes"),
            ("OEBPS/images/Pic 1.png", "pic bytes"),
            ("OEBPS/text/one.xhtml", ONE),
            ("OEBPS/text/two.xhtml", TWO),
        ],
    )
}

fn text_of(block: &book::Block) -> String {
    match &block.kind {
        Kind::Text(runs) => runs.iter().map(|run| run.text.as_str()).collect(),
        _ => "\u{fffc}".to_string(),
    }
}

#[test]
fn the_book_says_what_it_is() {
    let book = book::open(&standard("a.epub")).unwrap();
    assert_eq!(book.meta().title, "The Test Book");
    assert_eq!(book.meta().authors, ["Ada Writer", "Bo Editor"]);
    assert_eq!(book.meta().language.as_deref(), Some("en"));
    assert!(!book.meta().rtl);
    assert_eq!(book.chapters(), 2);
    assert_eq!(book.sizes(), [ONE.len() as u64, TWO.len() as u64]);
    assert_eq!(book.cover().as_deref(), Some("OEBPS/images/Cover.jpg"));
}

#[test]
fn the_contents_come_from_the_toc_nav_with_depths() {
    let book = book::open(&standard("b.epub")).unwrap();
    let list: Vec<(String, u32, u32, Option<String>)> = book
        .contents()
        .iter()
        .map(|e| (e.title.clone(), e.depth, e.chapter, e.anchor.clone()))
        .collect();
    assert_eq!(
        list,
        [
            ("One".to_string(), 0, 0, None),
            (
                "A part of one".to_string(),
                1,
                0,
                Some("second".to_string())
            ),
            ("Two".to_string(), 0, 1, None),
        ]
    );
}

#[test]
fn a_chapter_keeps_its_text_styles_and_order() {
    let mut book = book::open(&standard("c.epub")).unwrap();
    let chapter = book.chapter(0).unwrap();
    let texts: Vec<String> = chapter.blocks.iter().map(text_of).collect();
    assert_eq!(texts[0], "Chapter One");
    assert_eq!(texts[1], "It was a dark and stormy night. Café.1");
    assert_eq!(texts[2], "Second\u{2028}line, with a quiet word.");
    assert_eq!(texts[3], "Part two");
    assert_eq!(&texts[4..6], ["•\tapples", "•\tpears"]);
    assert_eq!(&texts[6..8], ["1.\tfirst", "2.\tsecond"]);
    assert_eq!(texts[8], "\u{fffc}");
    assert_eq!(
        texts[9],
        "\u{2003}\u{2003}a\u{2003}\u{2003}b".trim_start_matches('\u{2003}')
    );
    assert_eq!(texts[10], "The end. 😀 ok");

    // The book's own rules: the chapter title is centred, bold and starts a page; the first paragraph has no
    // indent and the next does; the quiet word is small and italic; the footnote mark is raised and links on.
    let title = &chapter.blocks[0];
    assert_eq!(title.heading, 1);
    assert_eq!(title.align, Align::Center);
    assert!(title.page_break_before);
    let Kind::Text(runs) = &title.kind else {
        panic!()
    };
    assert!(runs[0].look.bold);
    assert!((runs[0].look.size - 2.0).abs() < 0.01);
    assert_eq!(chapter.blocks[1].first_line, 0.0);
    assert_eq!(chapter.blocks[1].align, Align::Justify);
    assert_eq!(chapter.blocks[2].first_line, 1.5);
    let Kind::Text(runs) = &chapter.blocks[1].kind else {
        panic!()
    };
    let bold = runs.iter().find(|run| run.text == "a dark").unwrap();
    assert!(bold.look.bold && !bold.look.italic);
    let mark = runs.last().unwrap();
    assert_eq!(
        (mark.look.shift, mark.look.link.as_deref()),
        (1, Some("OEBPS/text/two.xhtml#note1"))
    );
    let Kind::Text(runs) = &chapter.blocks[2].kind else {
        panic!()
    };
    let quiet = runs.iter().find(|run| run.text == "quiet").unwrap();
    assert!(quiet.look.italic && (quiet.look.size - 0.8).abs() < 0.01);
    // The picture's path is whole and decoded.
    assert!(
        matches!(&chapter.blocks[8].kind, Kind::Image { src, ink: false } if src == "OEBPS/images/Pic 1.png")
    );
}

#[test]
fn positions_count_utf16_units_over_the_text_with_a_newline_per_block() {
    let mut book = book::open(&standard("d.epub")).unwrap();
    let chapter = book.chapter(0).unwrap();
    let mut at = 0u32;
    for block in &chapter.blocks {
        assert_eq!(block.start, at, "block {:?}", text_of(block));
        at += text_of(block).encode_utf16().count() as u32 + 1;
    }
    // An id inside a paragraph points into it, an id on a block points at its start.
    let anchor = |id: &str| chapter.anchors.iter().find(|a| a.id == id).unwrap().offset;
    assert_eq!(anchor("top"), 0);
    assert_eq!(anchor("second"), chapter.blocks[3].start);
    let mark =
        chapter.blocks[1].start + "It was a dark and stormy night.".encode_utf16().count() as u32;
    assert_eq!(anchor("mark"), mark);
}

#[test]
fn links_and_resources_resolve_inside_the_book() {
    let mut book = book::open(&standard("e.epub")).unwrap();
    assert_eq!(
        book.target("OEBPS/text/two.xhtml#note1"),
        Some((1, Some("note1".to_string())))
    );
    assert_eq!(book.target("OEBPS/TEXT/ONE.xhtml"), Some((0, None)));
    assert_eq!(book.target("OEBPS/nowhere.xhtml"), None);
    assert_eq!(
        book.resource("OEBPS/images/Cover.jpg").unwrap(),
        b"cover bytes"
    );
    assert!(book.resource("OEBPS/images/none.png").is_err());
    assert_eq!(text_of(&book.chapter(1).unwrap().blocks[0]), "A note.");
}

#[test]
fn broken_markup_still_opens() {
    let mut book = book::open(&standard("f.epub")).unwrap();
    let chapter = book.chapter(1).unwrap();
    let last = text_of(chapter.blocks.last().unwrap());
    assert_eq!(last, "Unclosed bold");
}

#[test]
fn old_books_take_their_contents_from_the_ncx() {
    let opf = PACKAGE
        .replace(r#" properties="nav""#, "")
        .replace("<spine ", r#"<spine toc="ncx" "#)
        .replace(
            "</manifest>",
            r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest>"#,
        );
    let ncx = r#"<?xml version="1.0"?><ncx><navMap>
      <navPoint id="a"><navLabel><text>First</text></navLabel><content src="text/one.xhtml"/>
        <navPoint id="b"><navLabel><text>Inner</text></navLabel><content src="text/one.xhtml#second"/></navPoint>
      </navPoint>
      <navPoint id="c"><navLabel><text>Last</text></navLabel><content src="text/two.xhtml"/></navPoint>
    </navMap></ncx>"#;
    let path = write_book(
        "g.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", &opf),
            ("OEBPS/toc.ncx", ncx),
            ("OEBPS/text/one.xhtml", ONE),
            ("OEBPS/text/two.xhtml", TWO),
            ("OEBPS/style/book.css", CSS),
        ],
    );
    let book = book::open(&path).unwrap();
    let list: Vec<(&str, u32, u32)> = book
        .contents()
        .iter()
        .map(|e| (e.title.as_str(), e.depth, e.chapter))
        .collect();
    assert_eq!(list, [("First", 0, 0), ("Inner", 1, 0), ("Last", 0, 1)]);
}

#[test]
fn a_book_without_contents_gets_numbered_parts() {
    let opf = PACKAGE.replace(
        r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#,
        "",
    );
    let path = write_book(
        "h.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", &opf),
            ("OEBPS/text/one.xhtml", ONE),
            ("OEBPS/text/two.xhtml", TWO),
        ],
    );
    let book = book::open(&path).unwrap();
    assert_eq!(
        book.contents()
            .iter()
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>(),
        ["Part 1", "Part 2"]
    );
}

#[test]
fn drm_is_refused_but_font_scrambling_is_not() {
    let drm = r#"<encryption><EncryptedData><EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/></EncryptedData></encryption>"#;
    let fonts = r#"<encryption><EncryptedData><EncryptionMethod Algorithm="http://www.idpf.org/2008/embedded-fonts"/></EncryptedData></encryption>"#;
    let with = |name: &str, encryption: &str| {
        write_book(
            name,
            &[
                ("META-INF/container.xml", CONTAINER),
                ("META-INF/encryption.xml", encryption),
                ("OEBPS/content.opf", PACKAGE),
                ("OEBPS/text/one.xhtml", ONE),
                ("OEBPS/text/two.xhtml", TWO),
            ],
        )
    };
    let error = book::open(&with("i.epub", drm)).err().unwrap();
    assert!(error.to_string().contains("DRM"), "{error}");
    assert!(book::open(&with("j.epub", fonts)).is_ok());
}

#[test]
fn files_that_are_not_books_are_refused_with_a_reason() {
    let dir = std::env::temp_dir().join(format!("aki-book-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("k.epub"), "not a zip").unwrap();
    assert!(
        book::open(&dir.join("k.epub"))
            .err()
            .unwrap()
            .to_string()
            .contains("not an EPUB")
    );
    std::fs::write(dir.join("l.txt"), "hello").unwrap();
    assert!(book::open(&dir.join("l.txt")).is_err());
    let empty = write_book(
        "m.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", "<package/>"),
        ],
    );
    assert!(
        book::open(&empty)
            .err()
            .unwrap()
            .to_string()
            .contains("no chapters")
    );
}

#[test]
fn adding_a_book_copies_it_once_and_a_bad_file_leaves_nothing() {
    let source = standard("n.epub");
    let out = std::env::temp_dir().join(format!("aki-book-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let (books, covers) = (out.join("books"), out.join("covers"));
    let first = book::import(&source, &books, &covers).unwrap();
    assert_eq!(first.meta.title, "The Test Book");
    assert_eq!(first.file, format!("{}.epub", first.id));
    assert_eq!(first.id.len(), 32);
    assert_eq!(first.cover, Some(format!("book-{}.jpg", first.id)));
    assert_eq!(
        std::fs::read(covers.join(first.cover.as_ref().unwrap())).unwrap(),
        b"cover bytes"
    );
    assert_eq!(first.bytes, std::fs::metadata(&source).unwrap().len());
    assert!(book::open(&books.join(&first.file)).is_ok());
    // The same file under another name is the same book.
    let copy = source.with_file_name("renamed.epub");
    std::fs::copy(&source, &copy).unwrap();
    assert_eq!(book::import(&copy, &books, &covers).unwrap().id, first.id);
    assert_eq!(std::fs::read_dir(&books).unwrap().count(), 1);

    let bad = source.with_file_name("bad.epub");
    std::fs::write(&bad, "not a zip").unwrap();
    assert!(book::import(&bad, &books, &covers).is_err());
    assert_eq!(std::fs::read_dir(&books).unwrap().count(), 1);
    let text = source.with_file_name("notes.txt");
    std::fs::write(&text, "hello").unwrap();
    assert!(
        book::import(&text, &books, &covers)
            .unwrap_err()
            .to_string()
            .contains("only EPUB")
    );
    std::fs::remove_dir_all(&out).unwrap();
}

/// Every `.epub` in the folder named by `AKI_BOOK_CORPUS` is opened and every chapter read: nothing may fail, every position
/// must agree with the text, and the speed is printed. Run with `cargo test --test book corpus -- --ignored --nocapture`.
#[test]
#[ignore = "needs real books: set AKI_BOOK_CORPUS to a folder of them"]
fn corpus() {
    let folder = std::env::var("AKI_BOOK_CORPUS").expect("AKI_BOOK_CORPUS");
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "epub") {
            continue;
        }
        let started = std::time::Instant::now();
        let mut book = book::open(&path).unwrap();
        let opened = started.elapsed();
        let (mut blocks, mut characters, mut slowest) = (0, 0, std::time::Duration::ZERO);
        for index in 0..book.chapters() {
            let started = std::time::Instant::now();
            let chapter = book.chapter(index).unwrap();
            slowest = slowest.max(started.elapsed());
            let mut at = 0u32;
            for block in &chapter.blocks {
                assert_eq!(block.start, at, "{} chapter {index}", path.display());
                at += text_of(block).encode_utf16().count() as u32 + 1;
            }
            blocks += chapter.blocks.len();
            characters += at;
        }
        println!(
            "{:<18} open {:>6.1?}  {:>3} chapters  {:>6} blocks  {:>8} chars  slowest chapter {:>7.1?}  contents {}  cover {}",
            path.file_name().unwrap().to_string_lossy(),
            opened,
            book.chapters(),
            blocks,
            characters,
            slowest,
            book.contents().len(),
            book.cover().is_some()
        );
    }
}

#[test]
fn a_very_large_chapter_file_is_read_as_pieces_cut_between_paragraphs() {
    let paragraphs: Vec<String> = (0..4000)
        .map(|n| format!("Paragraph number {n} says something quite ordinary."))
        .collect();
    let body: String = paragraphs
        .iter()
        .enumerate()
        .map(|(n, text)| {
            if n == 2500 {
                format!("<p id=\"mid\">{text}</p>")
            } else {
                format!("<p>{text}</p>")
            }
        })
        .collect();
    let page = format!("<html><body>{body}</body></html>");
    let opf = PACKAGE
        .replace(
            r#"<item id="c2" href="text/two.xhtml" media-type="application/xhtml+xml"/>"#,
            "",
        )
        .replace(r#"<itemref idref="c2"/>"#, "");
    let path = write_book(
        "o.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", &opf),
            ("OEBPS/text/one.xhtml", &page),
        ],
    );
    let mut book = book::open(&path).unwrap();
    let pieces = book.chapters();
    assert!(pieces >= 2, "{pieces}");
    assert_eq!(
        book.sizes().iter().sum::<u64>() / 100,
        page.len() as u64 / 100
    );

    // Every paragraph is in exactly one piece, in order, and each piece counts positions from its own start.
    let mut heard = Vec::new();
    let mut anchor_in = None;
    for index in 0..pieces {
        let chapter = book.chapter(index).unwrap();
        let mut at = 0;
        for block in &chapter.blocks {
            assert_eq!(block.start, at);
            at += text_of(block).encode_utf16().count() as u32 + 1;
            heard.push(text_of(block));
        }
        if let Some(found) = chapter.anchors.iter().find(|a| a.id == "mid") {
            anchor_in = Some((
                index,
                found.offset,
                chapter.blocks.iter().position(|b| b.start == found.offset),
            ));
        }
    }
    assert_eq!(heard, paragraphs);
    // A link to the id lands in the piece that holds it, and the id is at that paragraph's start.
    let (piece, offset, block) = anchor_in.unwrap();
    assert!(piece > 0);
    assert!(
        block.is_some(),
        "the id is at a paragraph's start: {offset}"
    );
    assert_eq!(
        book.target("OEBPS/text/one.xhtml#mid"),
        Some((piece, Some("mid".to_string())))
    );
    assert_eq!(book.target("OEBPS/text/one.xhtml"), Some((0, None)));
}

#[test]
fn odd_inputs_do_not_break_a_book() {
    // A single-quoted DRM algorithm is still DRM; an oversize number in CSS is ignored; a cover that is a page is no cover.
    let drm = "<encryption><EncryptionMethod Algorithm='http://www.w3.org/2001/04/xmlenc#aes128-cbc'/></encryption>";
    let path = write_book(
        "p.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("META-INF/encryption.xml", drm),
            ("OEBPS/content.opf", PACKAGE),
            ("OEBPS/text/one.xhtml", ONE),
            ("OEBPS/text/two.xhtml", TWO),
        ],
    );
    assert!(book::open(&path).err().unwrap().to_string().contains("DRM"));

    let opf = PACKAGE.replace(
        r#"href="images/Cover.jpg" media-type="image/jpeg""#,
        r#"href="text/two.xhtml" media-type="application/xhtml+xml""#,
    );
    let odd = ONE.replace(
        "<body>",
        r#"<body style="font-size: nanem; text-indent: infem; margin-left: NaN">"#,
    );
    let path = write_book(
        "q.epub",
        &[
            ("META-INF/container.xml", CONTAINER),
            ("OEBPS/content.opf", &opf),
            ("OEBPS/text/one.xhtml", &odd),
            ("OEBPS/text/two.xhtml", TWO),
        ],
    );
    let mut book = book::open(&path).unwrap();
    assert_eq!(book.cover(), None);
    let chapter = book.chapter(0).unwrap();
    assert!(
        chapter
            .blocks
            .iter()
            .all(|block| block.first_line.is_finite() && block.left.is_finite())
    );
}
