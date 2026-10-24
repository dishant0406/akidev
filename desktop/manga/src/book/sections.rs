//! Which part of a book each chapter is: the cover, the contents page, the front matter (copyright, dedication, foreword), the
//! book itself, or what follows it (notes, index, colophon).
//!
//! EPUB says so in three places, tried in this order: `epub:type` (and `role`) on a chapter's own markup, the landmarks list of the
//! navigation document, and the `<guide>` of older books. When a book says nothing, the file's name is read for the usual words
//! (titlepage, colophon, endnotes). A chapter nothing is known about is the book.

/// What a chapter is, for the reader to open the book at its start and to fold the rest away in the contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Cover,
    /// The contents page and lists of figures: the app has its own.
    Contents,
    Front,
    Body,
    Back,
}

/// What one word of the structural vocabulary says.
enum Say {
    Is(Section),
    /// Front or back, by where the book begins (an acknowledgements page can be either).
    Either,
}

fn say(token: &str) -> Option<Say> {
    Some(match token {
        "cover" => Say::Is(Section::Cover),
        "toc" | "lot" | "loi" | "lof" => Say::Is(Section::Contents),
        "frontmatter" | "titlepage" | "halftitlepage" | "title-page" | "copyright-page"
        | "imprint" | "uncopyright" | "dedication" | "epigraph" | "foreword" | "preface"
        | "contributors" | "credits" | "errata" | "other-credits" => Say::Is(Section::Front),
        "backmatter" | "appendix" | "afterword" | "index" | "glossary" | "bibliography"
        | "colophon" | "endnotes" | "endnote" | "footnotes" | "rearnotes" | "notes" => {
            Say::Is(Section::Back)
        }
        "bodymatter" | "chapter" | "part" | "volume" | "division" | "prologue" | "epilogue"
        | "introduction" | "conclusion" => Say::Is(Section::Body),
        "acknowledgments" | "acknowledgements" => Say::Either,
        _ => return None,
    })
}

/// The words a file's name says in the vocabulary: `titlepage.xhtml` is "titlepage", `endnotes.xhtml` is "endnotes".
pub fn words_of_name(path: &str) -> Vec<String> {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let stem = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(stem, _)| stem);
    let stem = stem.replace(['_', ' '], "-");
    let mut words = Vec::new();
    for (needle, token) in [
        ("cover", "cover"),
        ("titlepage", "titlepage"),
        ("title-page", "titlepage"),
        ("halftitle", "titlepage"),
        ("copyright", "copyright-page"),
        ("imprint", "imprint"),
        ("colophon", "colophon"),
        // Project Gutenberg's own pages: its notice before the book, its licence after.
        ("pg-header", "imprint"),
        ("pg-footer", "colophon"),
        ("endnote", "endnotes"),
        ("footnote", "endnotes"),
        ("dedication", "dedication"),
        ("epigraph", "epigraph"),
        ("acknowledg", "acknowledgments"),
        ("foreword", "foreword"),
        ("preface", "preface"),
        ("glossary", "glossary"),
        ("bibliograph", "bibliography"),
        ("appendix", "appendix"),
        ("afterword", "afterword"),
    ] {
        if stem.contains(needle) {
            words.push(token.to_string());
        }
    }
    if stem == "toc" || stem == "contents" || stem == "nav" || stem.starts_with("toc-") {
        words.push("toc".into());
    }
    if stem == "notes" || stem.ends_with("-notes") {
        words.push("endnotes".into());
    }
    if stem == "index" {
        words.push("index".into());
    }
    words
}

/// The words a page's own title or first heading says in the vocabulary: "CONTENTS", "Table of Contents", "Acknowledgements", "Index".
/// Only a heading that is that and nothing more counts: a chapter called "Notes on a Whale" is not the notes.
pub fn words_of_title(title: &str) -> Vec<String> {
    let text: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let token = match text.as_str() {
        "contents" | "table of contents" | "toc" | "list of illustrations" => "toc",
        "cover" => "cover",
        "title page" | "titlepage" | "half title" => "titlepage",
        "copyright" | "copyright page" | "copyright notice" | "legal notice" => "copyright-page",
        "dedication" => "dedication",
        "epigraph" => "epigraph",
        "acknowledgements" | "acknowledgments" | "acknowledgement" | "acknowledgment" => {
            "acknowledgments"
        }
        "foreword" => "foreword",
        "preface" => "preface",
        "about the author" | "about the authors" | "also by" | "other books by" => "backmatter",
        "index" => "index",
        "notes" | "endnotes" | "footnotes" | "author s notes" => "endnotes",
        "glossary" => "glossary",
        "bibliography" => "bibliography",
        "appendix" => "appendix",
        "afterword" => "afterword",
        "colophon" => "colophon",
        t if t.contains("project gutenberg")
            && (t.contains("license") || t.contains("licence")) =>
        {
            "colophon"
        }
        _ => return Vec::new(),
    };
    vec![token.to_string()]
}

/// The section of each file, and which file the book itself starts with. `files` is each chapter file's words, in reading order:
/// from its markup, the landmarks and the guide, then its name's.
pub fn classify(files: &[Vec<String>]) -> (Vec<Section>, usize) {
    let said: Vec<Option<Say>> = files
        .iter()
        .map(|words| {
            // The strongest word wins: a file that is the cover and also front matter is the cover.
            let all: Vec<Say> = words.iter().filter_map(|word| say(word)).collect();
            all.into_iter().min_by_key(|found| match found {
                Say::Is(Section::Cover) => 0,
                Say::Is(Section::Contents) => 1,
                Say::Is(Section::Body) => 2,
                Say::Is(Section::Back) => 3,
                Say::Is(Section::Front) => 4,
                Say::Either => 5,
            })
        })
        .collect();
    let first = said
        .iter()
        .position(|found| matches!(found, Some(Say::Is(Section::Body))))
        .or_else(|| said.iter().position(|found| found.is_none()))
        .unwrap_or(0);
    // Front matter can come after an introduction (a dedication, an epigraph): the book starts after the last of it in the first files.
    let lead = said.iter().take(10).rposition(|found| {
        matches!(
            found,
            Some(Say::Is(Section::Cover | Section::Contents | Section::Front)) | Some(Say::Either)
        )
    });
    let start = match lead {
        Some(lead)
            if lead + 1 < said.len()
                && lead + 1 > first
                && !matches!(said[lead + 1], Some(Say::Is(Section::Back))) =>
        {
            lead + 1
        }
        _ => first,
    };
    let sections = said
        .iter()
        .enumerate()
        .map(|(index, found)| match found {
            Some(Say::Is(section)) => *section,
            Some(Say::Either) if index < start => Section::Front,
            Some(Say::Either) => Section::Back,
            None => Section::Body,
        })
        .collect();
    (sections, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&[&str]]) -> Vec<Vec<String>> {
        text.iter()
            .map(|words| words.iter().map(|word| word.to_string()).collect())
            .collect()
    }

    #[test]
    fn a_book_is_cut_into_front_body_and_back() {
        let files = words(&[
            &["cover"],
            &["titlepage"],
            &["copyright-page"],
            &["toc"],
            &["acknowledgments"],
            &["introduction"],
            &["dedication"],
            &["bodymatter", "chapter"],
            &[],
            &["epilogue"],
            &["endnotes"],
            &["colophon"],
        ]);
        let (sections, start) = classify(&files);
        use Section::*;
        assert_eq!(
            sections,
            [
                Cover, Front, Front, Contents, Front, Body, Front, Body, Body, Body, Back, Back
            ]
        );
        assert_eq!(start, 7);
    }

    #[test]
    fn without_words_the_first_unknown_file_is_where_the_book_starts() {
        let (sections, start) = classify(&words(&[
            &["cover"],
            &["titlepage"],
            &[],
            &[],
            &["acknowledgments"],
        ]));
        assert_eq!(start, 2);
        assert_eq!(sections[4], Section::Back);
        let (all, start) = classify(&words(&[&["cover"], &["colophon"]]));
        assert_eq!((all[0], all[1], start), (Section::Cover, Section::Back, 0));
    }

    #[test]
    fn a_title_that_is_only_a_section_name_says_so() {
        assert_eq!(words_of_title("CONTENTS"), ["toc"]);
        assert_eq!(words_of_title(" Table of Contents "), ["toc"]);
        assert_eq!(words_of_title("Acknowledgements"), ["acknowledgments"]);
        assert_eq!(
            words_of_title("The Full Project Gutenberg™ License"),
            ["colophon"]
        );
        assert!(words_of_title("Notes on a Whale").is_empty());
        assert!(words_of_title("Chapter 1").is_empty());
    }

    #[test]
    fn a_file_name_says_what_it_is_when_the_book_does_not() {
        assert_eq!(words_of_name("OEBPS/text/titlepage.xhtml"), ["titlepage"]);
        assert_eq!(words_of_name("OEBPS/text/Endnotes.xhtml"), ["endnotes"]);
        assert_eq!(words_of_name("x/ch-01.xhtml"), Vec::<String>::new());
        assert_eq!(words_of_name("x/toc.xhtml"), ["toc"]);
        assert_eq!(words_of_name("x/uncopyright.xhtml"), ["copyright-page"]);
    }
}
