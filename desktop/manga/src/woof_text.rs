//! Text for Woof: the model was taught with numbers, dates and times written the way people say them ("six fifteen", "the
//! third of May", "nineteen ninety nine"), so a digit it has not seen spelled out comes out wrong. This turns the text a
//! book has into that form. It is a port of `fallback()` in Underdog's `normalize.py`, and of the abbreviations from
//! `norm_pre()`. The rest of that file (phone numbers, codes, web addresses, units, Roman numerals) is not ported: a book
//! seldom has them, and they are read as plain numbers and words.

use regex::{Captures, Regex};
use std::sync::LazyLock;

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [(u64, &str); 4] = [
    (1_000_000_000_000, "trillion"),
    (1_000_000_000, "billion"),
    (1_000_000, "million"),
    (1_000, "thousand"),
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// 1999 -> one thousand nine hundred and ninety nine.
fn cardinal(n: u64) -> String {
    if n < 20 {
        return ONES[n as usize].to_string();
    }
    if n < 100 {
        let ones = if n % 10 == 0 {
            String::new()
        } else {
            format!(" {}", ONES[(n % 10) as usize])
        };
        return format!("{}{ones}", TENS[(n / 10) as usize]);
    }
    if n < 1000 {
        let rest = if n % 100 == 0 {
            String::new()
        } else {
            format!(" and {}", cardinal(n % 100))
        };
        return format!("{} hundred{rest}", ONES[(n / 100) as usize]);
    }
    for (value, name) in SCALES {
        if n >= value {
            let rest = n % value;
            let tail = match rest {
                0 => String::new(),
                1..=99 => format!(" and {}", cardinal(rest)),
                _ => format!(" {}", cardinal(rest)),
            };
            return format!("{} {name}{tail}", cardinal(n / value));
        }
    }
    n.to_string()
}

/// 21 -> twenty first.
fn ordinal(n: u64) -> String {
    let words = cardinal(n);
    let (head, last) = match words.rfind(' ') {
        Some(at) => (&words[..=at], &words[at + 1..]),
        None => ("", words.as_str()),
    };
    let ordinal = match last {
        "one" => "first".to_string(),
        "two" => "second".to_string(),
        "three" => "third".to_string(),
        "five" => "fifth".to_string(),
        "eight" => "eighth".to_string(),
        "nine" => "ninth".to_string(),
        "twelve" => "twelfth".to_string(),
        _ if last.ends_with('y') => format!("{}ieth", &last[..last.len() - 1]),
        _ => format!("{last}th"),
    };
    format!("{head}{ordinal}")
}

/// 1999 -> nineteen ninety nine, 1905 -> nineteen oh five, 2005 -> two thousand and five.
fn year(n: u64) -> String {
    if (2000..=2009).contains(&n) {
        return cardinal(n);
    }
    let (high, low) = (n / 100, n % 100);
    match low {
        0 => format!("{} hundred", cardinal(high)),
        1..=9 => format!("{} oh {}", cardinal(high), ONES[low as usize]),
        _ => format!("{} {}", cardinal(high), cardinal(low)),
    }
}

/// "1,249.5" -> one thousand two hundred and forty nine point five.
fn number(text: &str) -> String {
    let text = text.replace(',', "");
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text.as_str(), None),
    };
    let words = match whole.parse::<u64>() {
        Ok(n) => cardinal(n),
        Err(_) if whole.is_empty() => "zero".to_string(),
        Err(_) => spell_digits(whole),
    };
    match fraction {
        Some(fraction) => format!("{words} point {}", spell_digits(fraction)),
        None => words,
    }
}

fn spell_digits(digits: &str) -> String {
    digits
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| ONES[d as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replaces every match with what `rule` makes of it. `rule` also sees the whole text, to look at what comes before and
/// after (the regex crate has no look-around), and returns None to leave the match as it is.
fn replace(
    pattern: &Regex,
    text: &str,
    rule: impl Fn(&Captures, &str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in pattern.captures_iter(text) {
        let whole = caps.get(0).expect("a match has a whole");
        out.push_str(&text[last..whole.start()]);
        match rule(&caps, text) {
            Some(new) => out.push_str(&new),
            None => out.push_str(whole.as_str()),
        }
        last = whole.end();
    }
    out.push_str(&text[last..]);
    out
}

fn after<'a>(text: &'a str, caps: &Captures) -> &'a str {
    &text[caps.get(0).map_or(0, |m| m.end())..]
}

macro_rules! pattern {
    ($name:ident, $text:expr) => {
        static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($text).expect("a fixed pattern"));
    };
}

pattern!(MONEY, r"([$€£])(\d[\d,]*)(?:\.(\d+))?\b");
pattern!(CLOCK, r"\b(\d{1,2}):(\d{2})\s*([aApP]\.?[mM])?(\.)?");
pattern!(HOUR, r"\b(\d{1,2})\s*([aApP])\.?[mM](\.)?");
pattern!(SLASH_DATE, r"\b(\d{1,2})/(\d{1,2})/(\d{4}|\d{2})\b");
pattern!(
    MONTH_DAY,
    r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\.? (\d{1,2})\b"
);
pattern!(DEGREES_C, r"(\d)\s?°\s?C\b");
pattern!(DEGREES_F, r"(\d)\s?°\s?F\b");
pattern!(DEGREES, r"(\d)\s?°");
pattern!(SLASH, r"(\w)/(\w)");
pattern!(ORDINAL, r"\b(\d+)(st|nd|rd|th)\b");
pattern!(YEAR, r"\b(1[1-9]\d\d|20\d\d)\b");
pattern!(MINUS, r"(^|[^\w-])-(\d)");
pattern!(LETTER_DIGIT, r"([A-Za-z])(\d)");
pattern!(PERCENT, r"(\d+(?:\.\d+)?)\s?%");
pattern!(
    NUMBER,
    r"\b\d{1,3}(?:,\d{3})+(?:\.\d+)?\b|\b\d+(?:\.\d+)?\b"
);
pattern!(SPACE_BEFORE_MARK, r"\s+([.,!?;:])");
pattern!(SPACES, r"\s{2,}");

fn money(caps: &Captures, _: &str) -> Option<String> {
    let (one, many, cent_one, cent_many) = match &caps[1] {
        "$" => ("dollar", "dollars", "cent", "cents"),
        "€" => ("euro", "euros", "cent", "cents"),
        _ => ("pound", "pounds", "penny", "pence"),
    };
    let whole: u64 = caps[2].replace(',', "").parse().ok()?;
    let fraction = caps.get(3).map(|m| m.as_str());
    if let Some(fraction) = fraction.filter(|f| f.len() != 2) {
        // $1.5 -> one point five dollars
        return Some(format!(
            "{} {many}",
            number(&format!("{}.{fraction}", &caps[2]))
        ));
    }
    let cents: u64 = fraction.map_or(0, |f| f.parse().unwrap_or(0));
    if whole == 0 && cents > 0 {
        return Some(format!(
            "{} {}",
            cardinal(cents),
            if cents == 1 { cent_one } else { cent_many }
        ));
    }
    let mut out = format!(
        "{} {}",
        cardinal(whole),
        if whole == 1 { one } else { many }
    );
    if cents > 0 {
        out.push_str(&format!(
            " and {} {}",
            cardinal(cents),
            if cents == 1 { cent_one } else { cent_many }
        ));
    }
    Some(out)
}

/// What follows a time: a full stop that ends "a.m." is dropped when the sentence goes on in lower case.
fn tail(stop: bool, rest: &str) -> String {
    let goes_on = rest.trim_start().starts_with(|c: char| c.is_lowercase())
        && rest.starts_with(char::is_whitespace);
    if stop && !goes_on {
        ".".to_string()
    } else {
        String::new()
    }
}

fn clock(caps: &Captures, text: &str) -> Option<String> {
    let rest = after(text, caps);
    if rest.starts_with(is_word) {
        return None;
    }
    let (hour, minute): (u64, u64) = (caps[1].parse().ok()?, caps[2].parse().ok()?);
    let marker = caps.get(3).map(|m| m.as_str());
    let stop = caps.get(4).is_some();
    // A full stop that is not the end of "a.m." belongs to the sentence.
    let stop_belongs = marker.is_none() && stop;
    let end = if stop_belongs {
        ".".to_string()
    } else {
        tail(stop && marker.is_some(), rest)
    };
    let spoken = if minute == 0 && marker.is_none() && rest.trim_start().starts_with("noon")
        || minute == 0 && marker.is_none() && rest.trim_start().starts_with("midnight")
    {
        cardinal(hour)
    } else if minute == 0 && marker.is_none() && (hour > 12 || hour == 0) {
        format!(
            "{} hundred",
            if hour == 0 {
                "zero".to_string()
            } else {
                cardinal(hour)
            }
        )
    } else {
        let mut out = cardinal(hour);
        match minute {
            0 if marker.is_none() => out.push_str(" o'clock"),
            0 => {}
            1..=9 => out.push_str(&format!(" oh {}", ONES[minute as usize])),
            _ => out.push_str(&format!(" {}", cardinal(minute))),
        }
        if let Some(marker) = marker {
            out.push(' ');
            out.push_str(&marker.replace('.', "").to_uppercase());
        }
        out
    };
    Some(format!("{spoken}{end}"))
}

fn hour(caps: &Captures, text: &str) -> Option<String> {
    let rest = after(text, caps);
    if rest.starts_with(is_word) {
        return None;
    }
    let stop = caps.get(3).is_some();
    Some(format!(
        "{} {}M{}",
        cardinal(caps[1].parse().ok()?),
        caps[2].to_uppercase(),
        tail(stop, rest)
    ))
}

fn slash_date(caps: &Captures, _: &str) -> Option<String> {
    let (month, day): (u64, u64) = (caps[1].parse().ok()?, caps[2].parse().ok()?);
    let mut y: u64 = caps[3].parse().ok()?;
    if caps[3].len() == 2 {
        y += 2000;
    }
    if (13..=31).contains(&month) && (1..=12).contains(&day) {
        // 14/03/2027 is day first.
        return Some(format!(
            "the {} of {} {}",
            ordinal(month),
            MONTHS[day as usize - 1],
            year(y)
        ));
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!(
        "{} {} {}",
        MONTHS[month as usize - 1],
        ordinal(day),
        year(y)
    ))
}

/// The abbreviations a book is full of, said in full. Those marked `true` only count before a capital letter.
const ABBREVIATIONS: [(&str, &str, bool); 22] = [
    ("Mr.", "Mister", false),
    ("Mrs.", "Missus", false),
    ("Ms.", "Miz", false),
    ("Dr.", "Doctor", true),
    ("Prof.", "Professor", true),
    ("St.", "Saint", true),
    ("Sgt.", "Sergeant", false),
    ("Capt.", "Captain", false),
    ("Lt.", "Lieutenant", false),
    ("Col.", "Colonel", false),
    ("Gen.", "General", true),
    ("Gov.", "Governor", false),
    ("Sen.", "Senator", false),
    ("Rev.", "Reverend", false),
    ("Jr.", "Junior", false),
    ("Sr.", "Senior", false),
    ("Mt.", "Mount", false),
    ("vs.", "versus", false),
    ("etc.", "et cetera", false),
    ("e.g.", "for example", false),
    ("i.e.", "that is", false),
    ("approx.", "approximately", false),
];

fn abbreviations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'scan: while let Some(c) = rest.chars().next() {
        let at_word_start = out.chars().next_back().is_none_or(|p| !is_word(p));
        if at_word_start {
            for (short, full, needs_capital) in ABBREVIATIONS {
                if let Some(after) = rest.strip_prefix(short) {
                    let capital = after.trim_start().starts_with(char::is_uppercase)
                        && after.starts_with(char::is_whitespace);
                    if !needs_capital || capital {
                        out.push_str(full);
                        rest = after;
                        continue 'scan;
                    }
                }
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The text as it is said: the same words, with numbers, times, dates and money written out.
pub fn spoken(text: &str) -> String {
    let text = text
        .replace(['’', '‘'], "'")
        .replace(['“', '”'], "\"")
        .replace('−', "-");
    let text = abbreviations(&text);
    let t = replace(&MONEY, &text, money);
    let t = replace(&CLOCK, &t, clock);
    let t = replace(&HOUR, &t, hour);
    let t = replace(&SLASH_DATE, &t, slash_date);
    let t = replace(&MONTH_DAY, &t, |caps, text| {
        (!after(text, caps).starts_with(':'))
            .then(|| format!("{} {}", &caps[1], ordinal(caps[2].parse().unwrap_or(0))))
    });
    let t = DEGREES_C.replace_all(&t, "$1 degrees Celsius").into_owned();
    let t = DEGREES_F
        .replace_all(&t, "$1 degrees Fahrenheit")
        .into_owned();
    let t = DEGREES.replace_all(&t, "$1 degrees").into_owned();
    let mut t = t;
    for _ in 0..2 {
        t = SLASH.replace_all(&t, "$1 slash $2").into_owned();
    }
    let t = replace(&ORDINAL, &t, |caps, _| Some(ordinal(caps[1].parse().ok()?)));
    let t = replace(&YEAR, &t, |caps, text| {
        let rest = after(text, caps);
        let mut chars = rest.chars();
        let decimal = matches!(chars.next(), Some('.' | ','))
            && chars.next().is_some_and(|c| c.is_ascii_digit());
        (!decimal).then(|| year(caps[1].parse().unwrap_or(0)))
    });
    let t = MINUS.replace_all(&t, "${1}minus $2").into_owned();
    let t = LETTER_DIGIT.replace_all(&t, "$1 $2").into_owned();
    let t = replace(&PERCENT, &t, |caps, _| {
        Some(format!("{} percent", number(&caps[1])))
    });
    let t = replace(&NUMBER, &t, |caps, _| Some(number(&caps[0])));
    let t = t
        .replace('&', " and ")
        .replace('+', " plus ")
        .replace('=', " equals ")
        .replace('@', " at ")
        .replace('#', " number ")
        .replace('–', "-");
    let t = SPACE_BEFORE_MARK.replace_all(&t, "$1");
    SPACES.replace_all(&t, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_like_people_say_them() {
        assert_eq!(cardinal(1999), "one thousand nine hundred and ninety nine");
        assert_eq!(ordinal(21), "twenty first");
        assert_eq!(ordinal(40), "fortieth");
        assert_eq!(year(1999), "nineteen ninety nine");
        assert_eq!(year(1905), "nineteen oh five");
        assert_eq!(year(2005), "two thousand and five");
        assert_eq!(
            number("1,249.5"),
            "one thousand two hundred and forty nine point five"
        );
    }

    #[test]
    fn a_sentence_with_everything_in_it() {
        assert_eq!(spoken("It was 1999."), "It was nineteen ninety nine.");
        assert_eq!(
            spoken("The 3rd of May, at 5pm."),
            "The third of May, at five PM."
        );
        assert_eq!(
            spoken("Boards at 6:45 a.m. sharp."),
            "Boards at six forty five AM sharp."
        );
        assert_eq!(spoken("Back by 6:05."), "Back by six oh five.");
        assert_eq!(
            spoken("He paid $25 for 12 eggs."),
            "He paid twenty five dollars for twelve eggs."
        );
        assert_eq!(
            spoken("Mr. Darcy and Dr. Who met 3 times."),
            "Mister Darcy and Doctor Who met three times."
        );
        assert_eq!(
            spoken("Up 4.5% in March 3."),
            "Up four point five percent in March third."
        );
        assert_eq!(spoken("Come at 14:00."), "Come at fourteen hundred.");
        assert_eq!(
            spoken("Chapter 12: He walked 3.5 miles at -4 degrees."),
            "Chapter twelve: He walked three point five miles at minus four degrees."
        );
        assert_eq!(
            spoken("On 12/25/2026 and 14/03/2027."),
            "On December twenty fifth twenty twenty six and the fourteenth of March twenty twenty seven."
        );
    }
}
