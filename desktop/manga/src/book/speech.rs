//! A chapter cut into what a voice reads one at a time: sentences, each with the place it covers in the chapter, in the
//! same positions highlights and search use, so the reader can mark what is being read without any mapping.

use super::{Chapter, Kind};
use unicode_segmentation::UnicodeSegmentation;

/// One sentence, or one heading.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Where it starts and how long it is, in the chapter's UTF-16 positions.
    pub start: u32,
    pub length: u32,
    /// What is read: without footnote marks and soft hyphens, spaces made single.
    pub text: String,
    pub heading: bool,
}

/// Words that end in a full stop without ending the sentence.
const ABBREVIATIONS: [&str; 11] = [
    "mr", "mrs", "ms", "dr", "prof", "st", "jr", "sr", "mt", "capt", "vs",
];

pub fn units(chapter: &Chapter) -> Vec<Unit> {
    let mut units = Vec::new();
    for block in &chapter.blocks {
        let Kind::Text(runs) = &block.kind else {
            continue;
        };
        let mut shown = String::new();
        // The byte ranges of `shown` that are footnote marks: shown on the page, not read.
        let mut silent = Vec::new();
        for run in runs {
            let from = shown.len();
            shown.push_str(&run.text);
            if run.look.shift != 0 {
                silent.push(from..shown.len());
            }
        }
        // UTF-16 position of every character, to turn the sentence's byte range into the chapter's positions.
        let mut at = Vec::with_capacity(shown.len());
        let mut position = block.start;
        for (byte, c) in shown.char_indices() {
            at.push((byte, position));
            position += c.len_utf16() as u32;
        }
        let end_of = |byte: usize| match at.binary_search_by_key(&byte, |&(b, _)| b) {
            Ok(i) => at[i].1,
            Err(_) => position,
        };
        let spoken = |range: std::ops::Range<usize>| -> String {
            let text: String = shown[range.clone()]
                .char_indices()
                .filter(|(i, c)| {
                    *c != '\u{ad}' && !silent.iter().any(|s| s.contains(&(range.start + i)))
                })
                .map(|(_, c)| c)
                .collect();
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        };
        let sentences: Vec<(usize, &str)> = if block.heading > 0 {
            vec![(0, shown.as_str())]
        } else {
            shown.split_sentence_bound_indices().collect()
        };
        let mut pending: Option<(usize, usize)> = None;
        for (from, sentence) in sentences {
            let (from, to) = match pending.take() {
                Some((first, _)) => (first, from + sentence.len()),
                None => (from, from + sentence.len()),
            };
            let text = spoken(from..to);
            if ends_in_abbreviation(&text) && to < shown.len() {
                pending = Some((from, to));
                continue;
            }
            if !text.chars().any(char::is_alphanumeric) {
                continue;
            }
            let to = from + shown[from..to].trim_end().len();
            let start = end_of(from);
            units.push(Unit {
                start,
                length: end_of(to) - start,
                text,
                heading: block.heading > 0,
            });
        }
    }
    units
}

fn ends_in_abbreviation(text: &str) -> bool {
    let Some(word) = text.trim_end().strip_suffix('.') else {
        return false;
    };
    let word = word.rsplit(char::is_whitespace).next().unwrap_or("");
    ABBREVIATIONS.contains(&word.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Align, Block, Look, Run};

    fn look(shift: i8) -> Look {
        Look {
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            small_caps: false,
            mono: false,
            shift,
            size: 1.0,
            family: None,
            link: None,
        }
    }

    fn block(start: u32, heading: u8, runs: Vec<(&str, i8)>) -> Block {
        Block {
            start,
            kind: Kind::Text(
                runs.into_iter()
                    .map(|(text, shift)| Run {
                        text: text.into(),
                        look: look(shift),
                    })
                    .collect(),
            ),
            heading,
            align: Align::Start,
            left: 0.0,
            first_line: 0.0,
            space_before: 0.0,
            page_break_before: false,
        }
    }

    fn chapter(blocks: Vec<Block>) -> Chapter {
        Chapter {
            blocks,
            ..Chapter::default()
        }
    }

    #[test]
    fn sentences_carry_their_place_and_a_heading_is_one_unit() {
        let c = chapter(vec![
            block(0, 1, vec![("Chapter One", 0)]),
            block(
                12,
                0,
                vec![
                    ("It was late. She said “Come in!” Then ", 0),
                    ("2", 1),
                    ("left.", 0),
                ],
            ),
        ]);
        let found = units(&c);
        let texts: Vec<&str> = found.iter().map(|u| u.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Chapter One",
                "It was late.",
                "She said “Come in!”",
                "Then left."
            ]
        );
        assert!(found[0].heading && !found[1].heading);
        assert_eq!((found[0].start, found[0].length), (0, 11));
        assert_eq!((found[1].start, found[1].length), (12, 12));
        // The last one runs over the footnote mark that is not read.
        assert_eq!(found[3].start + found[3].length, 12 + 38 + 1 + 5);
    }

    #[test]
    fn positions_count_utf16_units() {
        let c = chapter(vec![block(5, 0, vec![("🙂 Hi. Yes.", 0)])]);
        let found = units(&c);
        assert_eq!(found.len(), 2);
        // The emoji is two units, then a space and "Hi.".
        assert_eq!((found[0].start, found[0].length), (5, 6));
        assert_eq!(found[1].start, 5 + 7);
    }

    #[test]
    fn titles_and_soft_hyphens_do_not_split_or_sound() {
        let c = chapter(vec![block(
            0,
            0,
            vec![("Mr. Darcy was a gentle\u{ad}man. Dr. Who? No.", 0)],
        )]);
        let texts: Vec<String> = units(&c).into_iter().map(|u| u.text).collect();
        assert_eq!(texts, ["Mr. Darcy was a gentleman.", "Dr. Who?", "No."]);
        let c = chapter(vec![block(
            0,
            0,
            vec![("No. Please stay. Fruit, etc. She left.", 0)],
        )]);
        let texts: Vec<String> = units(&c).into_iter().map(|u| u.text).collect();
        assert_eq!(texts, ["No.", "Please stay.", "Fruit, etc.", "She left."]);
    }

    #[test]
    fn a_block_with_nothing_to_say_has_no_unit() {
        let c = chapter(vec![
            block(0, 0, vec![("* * *", 0)]),
            block(6, 0, vec![("  ", 0)]),
        ]);
        assert!(units(&c).is_empty());
    }
}
