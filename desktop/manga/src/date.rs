//! Dates and chapter numbers as sources write them.
//!
//! Dates come as a pattern ("MMM d, yyyy"), as ISO ("iso"), or relative ("3 days ago", "relative").
//! A chapter number is pulled out of a title ("Ch. 12.5 - Name").

use regex::Regex;

const DAY_MS: i64 = 86_400_000;

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const MONTHS: [&str; 12] = [
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december",
];

fn month_number(name: &str) -> Option<i64> {
    let name = name.trim_end_matches('.').to_lowercase();
    if name.len() < 3 {
        return None;
    }
    MONTHS.iter().position(|month| month.starts_with(&name)).map(|index| index as i64 + 1)
}

/// What a pattern holds, in the order its parts appear.
#[derive(Clone, Copy)]
enum Part {
    Year,
    ShortYear,
    MonthName,
    MonthNumber,
    Day,
}

/// A regular expression and the parts its groups stand for, built from a pattern like "MMM d, yyyy".
fn compile(format: &str) -> Option<(Regex, Vec<Part>)> {
    let mut regex = String::new();
    let mut parts = Vec::new();
    let mut rest = format;
    while !rest.is_empty() {
        let (pattern, part, width) = if rest.starts_with("yyyy") {
            (r"(\d{4})", Some(Part::Year), 4)
        } else if rest.starts_with("yy") {
            (r"(\d{2})", Some(Part::ShortYear), 2)
        } else if rest.starts_with("MMMM") {
            (r"([A-Za-z]+\.?)", Some(Part::MonthName), 4)
        } else if rest.starts_with("MMM") {
            (r"([A-Za-z]{3,9}\.?)", Some(Part::MonthName), 3)
        } else if rest.starts_with("MM") {
            (r"(\d{1,2})", Some(Part::MonthNumber), 2)
        } else if rest.starts_with('M') {
            (r"(\d{1,2})", Some(Part::MonthNumber), 1)
        } else if rest.starts_with("dd") {
            (r"(\d{1,2})(?:st|nd|rd|th)?", Some(Part::Day), 2)
        } else if rest.starts_with('d') {
            (r"(\d{1,2})(?:st|nd|rd|th)?", Some(Part::Day), 1)
        } else {
            ("", None, rest.chars().next()?.len_utf8())
        };
        match part {
            Some(part) => {
                regex.push_str(pattern);
                parts.push(part);
            }
            None => regex.push_str(&regex::escape(&rest[..width])),
        }
        rest = &rest[width..];
    }
    parts.iter().any(|part| matches!(part, Part::Year | Part::ShortYear)).then_some(())?;
    Some((Regex::new(&format!("(?i){regex}")).ok()?, parts))
}

fn by_pattern(text: &str, format: &str) -> Option<i64> {
    let (regex, parts) = compile(format)?;
    let found = regex.captures(text)?;
    let (mut year, mut month, mut day) = (None, 1, 1);
    for (index, part) in parts.iter().enumerate() {
        let group = found.get(index + 1)?.as_str();
        match part {
            Part::Year => year = Some(group.parse::<i64>().ok()?),
            Part::ShortYear => year = Some(2000 + group.parse::<i64>().ok()?),
            Part::MonthName => month = month_number(group)?,
            Part::MonthNumber => month = group.parse().ok()?,
            Part::Day => day = group.parse().ok()?,
        }
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year?, month, day) * DAY_MS)
}

fn iso(text: &str) -> Option<i64> {
    let found = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").ok()?.captures(text)?;
    let number = |index: usize| found[index].parse::<i64>().ok();
    let (year, month, day) = (number(1)?, number(2)?, number(3)?);
    ((1..=12).contains(&month) && (1..=31).contains(&day)).then(|| days_from_civil(year, month, day) * DAY_MS)
}

/// "3 days ago", "an hour ago", "yesterday", "just now", relative to `now_ms`.
fn relative(text: &str, now_ms: i64) -> Option<i64> {
    let lower = text.to_lowercase();
    if lower.contains("yesterday") {
        return Some(now_ms - DAY_MS);
    }
    if lower.contains("today") || lower.contains("just now") || lower.contains("moments ago") {
        return Some(now_ms);
    }
    let found = Regex::new(r"(\d+|an?|one)\s*(second|sec|minute|min|hour|hr|day|week|month|year|yr)s?\b")
        .ok()?
        .captures(&lower)?;
    let amount: i64 = match &found[1] {
        "a" | "an" | "one" => 1,
        digits => digits.parse().ok()?,
    };
    let unit = match &found[2] {
        "second" | "sec" => 1_000,
        "minute" | "min" => 60_000,
        "hour" | "hr" => 3_600_000,
        "day" => DAY_MS,
        "week" => 7 * DAY_MS,
        "month" => 30 * DAY_MS,
        _ => 365 * DAY_MS,
    };
    Some(now_ms - amount * unit)
}

/// The first format that reads the text, as milliseconds since the epoch.
pub fn parse(text: &str, formats: &[String], now_ms: i64) -> Option<i64> {
    formats.iter().find_map(|format| match format.as_str() {
        "iso" => iso(text),
        "relative" => relative(text, now_ms),
        pattern => by_pattern(text, pattern),
    })
}

/// Rejects a format that can never read anything.
pub fn check(format: &str) -> bool {
    matches!(format, "iso" | "relative") || compile(format).is_some()
}

/// The chapter number in a title: a number after "chapter", "ch.", "episode", "ep" or "#" first,
/// otherwise the first number.
pub fn chapter_number(text: &str) -> Option<f64> {
    let marked = Regex::new(r"(?i)(?:chapter|ch\.?|episode|ep\.?|#)\s*(\d+(?:\.\d+)?)").ok()?;
    let any = Regex::new(r"(\d+(?:\.\d+)?)").ok()?;
    marked.captures(text).or_else(|| any.captures(text))?[1].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn formats(list: &[&str]) -> Vec<String> {
        list.iter().map(|format| format.to_string()).collect()
    }

    #[test]
    fn reads_patterns_with_month_names_and_ordinals() {
        let day = parse("March 5th, 2024", &formats(&["MMMM d, yyyy"]), NOW).unwrap();
        assert_eq!(day, days_from_civil(2024, 3, 5) * DAY_MS);
        assert_eq!(parse("Jan 07, 2023", &formats(&["MMM dd, yyyy"]), NOW), Some(days_from_civil(2023, 1, 7) * DAY_MS));
        assert_eq!(parse("07/01/23", &formats(&["dd/MM/yy"]), NOW), Some(days_from_civil(2023, 1, 7) * DAY_MS));
    }

    #[test]
    fn reads_relative_and_iso_dates_and_tries_formats_in_order() {
        assert_eq!(parse("3 days ago", &formats(&["relative"]), NOW), Some(NOW - 3 * DAY_MS));
        assert_eq!(parse("an hour ago", &formats(&["relative"]), NOW), Some(NOW - 3_600_000));
        assert_eq!(parse("2024-02-29T10:00:00+00:00", &formats(&["iso"]), NOW), Some(days_from_civil(2024, 2, 29) * DAY_MS));
        assert!(parse("2 weeks ago", &formats(&["MMM d, yyyy", "relative"]), NOW).is_some());
        assert!(parse("nonsense", &formats(&["relative", "iso"]), NOW).is_none());
    }

    #[test]
    fn pulls_chapter_numbers_out_of_titles() {
        assert_eq!(chapter_number("Chapter 12.5 - Name"), Some(12.5));
        assert_eq!(chapter_number("Solo Leveling 2 Ch. 40"), Some(40.0));
        assert_eq!(chapter_number("Episode 7"), Some(7.0));
        assert_eq!(chapter_number("Prologue"), None);
    }

    #[test]
    fn rejects_a_format_without_a_year() {
        assert!(check("MMM d, yyyy"));
        assert!(!check("MMM d"));
    }
}
