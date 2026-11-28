//! Dates, without a date library: the definitions only need to compare a release date or year
//! with today.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub const DAY_MS: i64 = 86_400_000;

/// Days since 1970-01-01 for a calendar date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

/// The (year, month, day) of a Unix time in milliseconds, in UTC.
pub fn civil_from_ms(ms: i64) -> (i64, i64, i64) {
    let days = ms.div_euclid(DAY_MS) + 719468;
    let era = days.div_euclid(146097);
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// Unix milliseconds of a leading "YYYY-MM-DD" date, or `None` when the text has none.
pub fn parse_date_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let well_formed = bytes.len() >= 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4]
            .iter()
            .chain(&bytes[5..7])
            .chain(&bytes[8..10])
            .all(u8::is_ascii_digit);
    if !well_formed {
        return None;
    }
    let year: i64 = text[..4].parse().ok()?;
    let month: i64 = text[5..7].parse().ok()?;
    let day: i64 = text[8..10].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * DAY_MS)
}

/// Whether a "YYYY-MM-DD" air date is a real date on a day after today (in UTC). A missing, odd or invalid date is not in the future.
pub fn is_future_date(text: &str, now_ms: i64) -> bool {
    let Some(date_ms) = parse_date_ms(text) else {
        return false;
    };
    // Reject only real Gregorian dates: `parse_date_ms` normalises invalid days.
    let (year, month, day) = civil_from_ms(date_ms);
    text.starts_with(&format!("{year:04}-{month:02}-{day:02}"))
        && date_ms.div_euclid(DAY_MS) > now_ms.div_euclid(DAY_MS)
}

/// Whether a release date ("YYYY-MM-DD", "YYYY-MM" or "YYYY") is after today (in UTC). A month or a
/// year is in the future only when all of it is. Any other text is not in the future.
pub fn is_upcoming(date: &str, now_ms: i64) -> bool {
    let (year, month, day) = civil_from_ms(now_ms);
    let today = format!("{year:04}-{month:02}-{day:02}");
    let shaped = matches!(date.len(), 4 | 7 | 10)
        && date
            .bytes()
            .zip(today.bytes())
            .all(|(have, want)| have.is_ascii_digit() == want.is_ascii_digit());
    shaped && date > &today[..date.len()]
}

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

/// A release date in words: "27 January 2027", "January 2027" or "2027". Other text comes back as it is.
pub fn long_date(date: &str) -> String {
    let mut parts = date.split('-').map(|part| part.parse::<usize>().ok());
    let (year, month, day) = (parts.next(), parts.next(), parts.next());
    let month = month
        .flatten()
        .and_then(|month| MONTHS.get(month.wrapping_sub(1)));
    match (year.flatten(), month, day.flatten()) {
        (Some(year), Some(month), Some(day)) => format!("{day} {month} {year}"),
        (Some(year), Some(month), None) => format!("{month} {year}"),
        _ => date.to_string(),
    }
}

/// The first run of four digits in a text, as a year: "2008", "2008–2013" and "(2019)" all give
/// their first year.
pub fn first_year(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(3))
        .find(|&at| bytes[at..at + 4].iter().all(u8::is_ascii_digit))
        .and_then(|at| text[at..at + 4].parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_through_civil_time() {
        let ms = parse_date_ms("2026-10-13").unwrap();
        assert_eq!(civil_from_ms(ms), (2026, 10, 13));
        assert_eq!(civil_from_ms(0), (1970, 1, 1));
        assert_eq!(parse_date_ms("2026-10-13T05:00:00.000Z"), Some(ms));
        assert_eq!(parse_date_ms("2026-13-01"), None);
        assert_eq!(parse_date_ms("soon"), None);
    }

    #[test]
    fn only_a_real_date_after_today_is_in_the_future() {
        let today = parse_date_ms("2026-10-13").unwrap() + 3_600_000;
        assert!(is_future_date("2026-10-14", today));
        assert!(!is_future_date("2026-10-13", today));
        assert!(!is_future_date("2025-01-01", today));
        assert!(!is_future_date("2026-02-31", today));
        assert!(!is_future_date("soon", today));
    }

    #[test]
    fn a_release_date_is_upcoming_only_when_all_of_it_is_after_today() {
        let today = parse_date_ms("2026-11-28").unwrap() + 3_600_000;
        assert!(is_upcoming("2026-11-29", today));
        assert!(!is_upcoming("2026-11-28", today));
        assert!(!is_upcoming("2026-11-20", today));
        assert!(is_upcoming("2026-12", today));
        assert!(!is_upcoming("2026-11", today));
        assert!(is_upcoming("2027", today));
        assert!(!is_upcoming("2026", today));
        assert!(!is_upcoming("soon", today));
        assert!(!is_upcoming("2027-01-27T00:00:00.000Z", today));
    }

    #[test]
    fn a_release_date_reads_in_words() {
        assert_eq!(long_date("2027-01-09"), "9 January 2027");
        assert_eq!(long_date("2027-10"), "October 2027");
        assert_eq!(long_date("2027"), "2027");
        assert_eq!(long_date("soon"), "soon");
    }

    #[test]
    fn a_range_is_read_as_its_first_year() {
        assert_eq!(first_year("2008\u{2013}2013"), Some(2008));
        assert_eq!(first_year("2026"), Some(2026));
        assert_eq!(first_year("2019-2021"), Some(2019));
        assert_eq!(first_year(""), None);
        assert_eq!(first_year("abc"), None);
    }
}
