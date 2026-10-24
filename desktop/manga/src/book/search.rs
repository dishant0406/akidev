//! Looking for words in a whole book: the exact words first, and if they are nowhere, the nearest words (a typo, a different spelling).
//!
//! Every chapter is read on its own thread with its own copy of the book open, so a search never makes the page being read wait.

use super::{Chapter, Kind, open};
use neo_frizbee::{Config, Matcher};
use std::path::Path;

/// One place a search found: where it is, how long it is, and the words around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub chapter: u32,
    pub offset: u32,
    /// How long the found words are, in the units positions are counted in.
    pub length: u32,
    pub snippet: String,
    /// The words found are near the ones asked for, not the same.
    pub similar: bool,
}

/// Every place in the book at `path` where `query` is written, in reading order, up to `limit`. When it is written nowhere (and is
/// long enough for a typo to be told from another word) the nearest words are given instead, marked `similar`.
pub fn search(path: &Path, query: &str, limit: usize) -> anyhow::Result<Vec<Hit>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let chapters = open(path)?.chapters();
    let exact = across(path, chapters, query, limit, false)?;
    if !exact.is_empty() || query.chars().count() < 4 {
        return Ok(exact);
    }
    across(path, chapters, query, limit, true)
}

/// What a thread found: its chapters that had finds, each with them.
type Found = Vec<(usize, Vec<Hit>)>;

/// The chapters shared out over the threads, each with its own open book; the finds come back in reading order.
fn across(
    path: &Path,
    chapters: usize,
    query: &str,
    limit: usize,
    similar: bool,
) -> anyhow::Result<Vec<Hit>> {
    let workers = std::thread::available_parallelism()
        .map_or(2, |count| count.get())
        .clamp(1, 8)
        .min(chapters.max(1));
    let found: Vec<anyhow::Result<Found>> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..workers)
            .map(|worker| {
                scope.spawn(move || -> anyhow::Result<Found> {
                    let mut book = open(path)?;
                    let mut mine = Vec::new();
                    let mut count = 0;
                    for index in (worker..chapters).step_by(workers) {
                        // A chapter that cannot be read is passed over, not the end of the search.
                        let Ok(chapter) = book.chapter(index) else {
                            continue;
                        };
                        let hits = in_chapter(&chapter, index, query, similar);
                        count += hits.len();
                        if !hits.is_empty() {
                            mine.push((index, hits));
                        }
                        if count >= limit {
                            break;
                        }
                    }
                    Ok(mine)
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| {
                thread
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("a search stopped")))
            })
            .collect()
    });
    let mut all: Found = Vec::new();
    for part in found {
        all.extend(part?);
    }
    all.sort_by_key(|(index, _)| *index);
    // A word found everywhere stops each thread at the limit, so memory stays small: the list is then the first finds of each thread.
    let mut hits: Vec<Hit> = all.into_iter().flat_map(|(_, hits)| hits).collect();
    hits.truncate(limit);
    Ok(hits)
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The finds in one chapter.
pub fn in_chapter(chapter: &Chapter, index: usize, query: &str, similar: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    let needle: Vec<char> = query.trim().chars().map(fold).collect();
    if needle.is_empty() {
        return hits;
    }
    for block in &chapter.blocks {
        let Kind::Text(runs) = &block.kind else {
            continue;
        };
        let chars: Vec<char> = runs.iter().flat_map(|run| run.text.chars()).collect();
        let folded: Vec<char> = chars.iter().copied().map(fold).collect();
        let places = if similar {
            near(&chars, &needle)
        } else {
            exact(&folded, &needle)
        };
        for (at, length) in places {
            let before: usize = chars[..at].iter().map(|c| c.len_utf16()).sum();
            let units: usize = chars[at..at + length].iter().map(|c| c.len_utf16()).sum();
            hits.push(Hit {
                chapter: index as u32,
                offset: block.start + before as u32,
                length: units as u32,
                snippet: snippet(&chars, at, length),
                similar,
            });
        }
    }
    hits
}

/// Where `needle` is written in `folded`, as (start, length) in characters.
fn exact(folded: &[char], needle: &[char]) -> Vec<(usize, usize)> {
    let mut places = Vec::new();
    let mut at = 0;
    while at + needle.len() <= folded.len() {
        if folded[at..at + needle.len()] == *needle {
            places.push((at, needle.len()));
            at += needle.len();
        } else {
            at += 1;
        }
    }
    places
}

/// The words of `chars` (or runs of as many words as the query has) that are nearly the query, as (start, length) in characters.
/// "Nearly" is a typo or two, by the fuzzy matcher, and a length near the query's: a word that merely contains its letters in order
/// is not near.
fn near(chars: &[char], needle: &[char]) -> Vec<(usize, usize)> {
    let words = needle
        .split(|c| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .count()
        .max(1);
    let spans = word_spans(chars);
    if spans.len() < words {
        return Vec::new();
    }
    let candidates: Vec<(usize, usize)> = (0..=spans.len() - words)
        .map(|first| (spans[first].0, spans[first + words - 1].1 - spans[first].0))
        .collect();
    let texts: Vec<String> = candidates
        .iter()
        .map(|&(at, length)| chars[at..at + length].iter().collect())
        .collect();
    let wanted: String = needle.iter().collect();
    let typos = (needle.len() / 4).clamp(1, 2);
    let mut matcher = Matcher::new(
        wanted.as_str(),
        &Config::default().max_typos(Some(typos as u16)),
    );
    // The length may differ by no more than the typos allowed.
    let slack = typos;
    matcher
        .match_list(&texts)
        .into_iter()
        .map(|found| found.index as usize)
        .filter(|&index| candidates[index].1.abs_diff(needle.len()) <= slack)
        .map(|index| candidates[index])
        .collect()
}

/// Where each word begins and ends in `chars`, as (start, end).
fn word_spans(chars: &[char]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (at, c) in chars.iter().enumerate() {
        let inside = c.is_alphanumeric() || (*c == '\'' || *c == '’') && start.is_some();
        match (inside, start) {
            (true, None) => start = Some(at),
            (false, Some(from)) => {
                spans.push((from, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        spans.push((from, chars.len()));
    }
    spans
}

/// The words around a find, about a line's worth, with … where it is cut.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Align, Block, Look, Run};

    fn chapter(texts: &[&str]) -> Chapter {
        let mut start = 0;
        let blocks = texts
            .iter()
            .map(|text| {
                let block = Block {
                    start,
                    kind: Kind::Text(vec![Run {
                        text: text.to_string(),
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
                    }]),
                    heading: 0,
                    align: Align::Start,
                    left: 0.0,
                    first_line: 0.0,
                    space_before: 0.0,
                    page_break_before: false,
                };
                start += text.encode_utf16().count() as u32 + 1;
                block
            })
            .collect();
        Chapter {
            blocks,
            anchors: Vec::new(),
            fonts: Vec::new(),
            vertical: false,
        }
    }

    #[test]
    fn exact_finds_report_where_and_how_long_in_utf16_units() {
        let found = in_chapter(
            &chapter(&["😀 Hello World", "say hello again"]),
            3,
            "HELLO",
            false,
        );
        assert_eq!(found.len(), 2);
        // The emoji is two units, then a space.
        assert_eq!(
            (found[0].chapter, found[0].offset, found[0].length),
            (3, 3, 5)
        );
        assert_eq!(found[1].offset, 15 + 4);
        assert!(!found[0].similar);
    }

    #[test]
    fn near_finds_a_misspelt_word_but_not_a_word_that_merely_holds_its_letters() {
        let page = chapter(&[
            "He walked to Pemberley at last, and the tailor stayed in Taxonomical Hall.",
        ]);
        let found = in_chapter(&page, 0, "Pemberly", true);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].offset, found[0].length), (13, 9));
        assert!(found[0].similar);
        // "tail" is inside "tailor" and in order inside "Taxonomical": neither is near it.
        assert!(in_chapter(&page, 0, "tail", true).is_empty());
        // Two words are matched as two words.
        let found = in_chapter(
            &chapter(&["it was a dark and stormy night"]),
            0,
            "dark and stomy",
            true,
        );
        assert_eq!((found.len(), found[0].offset, found[0].length), (1, 9, 15));
    }
}
