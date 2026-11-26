//! The shelf through the facade: add a book, open it, read a chapter, keep the place, take it off.

use akimanga::{BookKind, Engine};
use std::io::Write;

fn epub(path: &std::path::Path) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let files = [
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="b.opf"/></rootfiles></container>"#,
        ),
        (
            "b.opf",
            r#"<package><metadata xmlns:dc="x"><dc:title>Short</dc:title><dc:creator>Me</dc:creator></metadata>
            <manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/></manifest>
            <spine><itemref idref="c"/></spine></package>"#,
        ),
        (
            "c.xhtml",
            "<html><body><h1 id=\"t\">Hello</h1><p>World</p></body></html>",
        ),
    ];
    for (name, content) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn a_book_goes_on_the_shelf_is_read_remembered_and_removed() {
    let dir = std::env::temp_dir().join(format!("aki-shelf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("short.epub");
    epub(&file);
    let engine = Engine::new(dir.join("data").to_string_lossy().into()).unwrap();

    let info = engine.import_book(file.to_string_lossy().into()).unwrap();
    assert_eq!(
        (info.title.as_str(), info.authors.as_slice()),
        ("Short", ["Me".to_string()].as_slice())
    );
    assert_eq!(
        engine
            .import_book(file.to_string_lossy().into())
            .unwrap()
            .id,
        info.id
    );
    assert_eq!(engine.books().unwrap().len(), 1);

    let reader = engine.open_book(info.id.clone()).unwrap();
    assert_eq!(reader.chapter_count(), 1);
    assert_eq!((reader.start_chapter(), reader.sections().len()), (0, 1));
    let chapter = reader.chapter(0).unwrap();
    let BookKind::Text { runs } = &chapter.blocks[1].kind else {
        panic!()
    };
    assert_eq!(runs[0].text, "World");
    assert!(reader.chapter(5).is_err());
    let speech = reader.speech(0).unwrap();
    let said: Vec<(&str, u32, bool)> = speech
        .iter()
        .map(|unit| (unit.text.as_str(), unit.start, unit.heading))
        .collect();
    assert_eq!(said, [("Hello", 0, true), ("World", 6, false)]);
    assert!(reader.speech(5).is_err());

    engine
        .save_book_position(info.id.clone(), 0, 6, 0.5, false)
        .unwrap();
    // A new engine on the same folder finds the book where it was left.
    let again = Engine::new(dir.join("data").to_string_lossy().into()).unwrap();
    let kept = again.book_info(info.id.clone()).unwrap();
    assert_eq!((kept.offset, kept.progress), (6, 0.5));
    assert!(kept.opened_ms.is_some());

    again.remove_book(info.id.clone()).unwrap();
    assert!(again.books().unwrap().is_empty());
    assert!(again.open_book(info.id).is_err());
    assert_eq!(
        std::fs::read_dir(dir.join("data/books")).unwrap().count(),
        0
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_book_takes_a_chosen_cover_and_the_categories_series_use() {
    let dir = std::env::temp_dir().join(format!("aki-shelf-cover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("short.epub");
    epub(&file);
    let engine = Engine::new(dir.join("data").to_string_lossy().into()).unwrap();
    let id = engine
        .import_book(file.to_string_lossy().into())
        .unwrap()
        .id;

    // Not a picture: refused, and nothing is kept.
    assert!(
        engine
            .set_book_custom_cover(id.clone(), b"text".to_vec())
            .is_err()
    );
    assert!(!engine.book_info(id.clone()).unwrap().custom_cover);

    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
    engine
        .set_book_custom_cover(id.clone(), jpeg.clone())
        .unwrap();
    let info = engine.book_info(id.clone()).unwrap();
    assert!(info.custom_cover);
    let shown = std::path::PathBuf::from(info.cover.unwrap().trim_start_matches("file://"));
    assert_eq!(std::fs::read(&shown).unwrap(), jpeg);
    // A second choice replaces the first, and its file goes.
    engine
        .set_book_custom_cover(id.clone(), vec![0xFF, 0xD8, 0xFF, 9])
        .unwrap();
    assert!(!shown.exists());
    engine.clear_book_custom_cover(id.clone()).unwrap();
    let info = engine.book_info(id.clone()).unwrap();
    assert!(!info.custom_cover);
    assert_eq!(
        std::fs::read_dir(dir.join("data/covers"))
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );

    let reading = engine.add_category("Reading".into()).unwrap();
    engine.set_book_category(id.clone(), reading, true).unwrap();
    assert_eq!(engine.book_info(id.clone()).unwrap().categories, [reading]);
    assert_eq!(engine.categories().unwrap()[0].books, 1);
    engine
        .set_book_category(id.clone(), reading, false)
        .unwrap();
    assert!(engine.book_info(id).unwrap().categories.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_book_is_finished_reset_and_its_highlights_are_listed_with_it() {
    let dir = std::env::temp_dir().join(format!("aki-shelf-state-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("short.epub");
    epub(&file);
    let engine = Engine::new(dir.join("data").to_string_lossy().into()).unwrap();
    let id = engine
        .import_book(file.to_string_lossy().into())
        .unwrap()
        .id;
    assert!(engine.book_info(id.clone()).unwrap().opened_ms.is_none());

    engine.finish_book(id.clone()).unwrap();
    let info = engine.book_info(id.clone()).unwrap();
    assert!(info.finished && info.opened_ms.is_some() && info.progress == 1.0);
    engine.reset_book(id.clone()).unwrap();
    let info = engine.book_info(id.clone()).unwrap();
    assert!(!info.finished && info.opened_ms.is_none() && info.progress == 0.0);

    engine
        .add_book_mark(id.clone(), 0, 0, 5, 1, "Hello".into())
        .unwrap();
    let marks = engine.recent_highlights(5).unwrap();
    assert_eq!(
        (marks[0].title.as_str(), marks[0].highlight.quote.as_str()),
        ("Short", "Hello")
    );
    assert_eq!(marks[0].book, id);
    std::fs::remove_dir_all(&dir).unwrap();
}
