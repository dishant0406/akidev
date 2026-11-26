//! Media ids.
//!
//! Every id names the kind, then the provider's own id, then the episode where
//! there is one:
//!
//! ```text
//! anime-16498-ep1     an anime, episode 1
//! series-169-s1e5     a series, season 1 episode 5
//! movie-tt32261958    a film, no episode
//! ```
//!
//! The kind is part of the id because ids come from different providers and would
//! collide: TVMaze's show 1 and YTS's film 1 are different titles. The provider id is
//! alphanumeric because Cinemeta uses IMDb ids ("tt32261958").
//!
//! A title from a plugin's provider says which provider it came from, because the built-in
//! chain could not tell its ids from AniList's: `movie-myprovider~abc_2d12` is the film `abc-12`
//! of the provider `myprovider`. The service's own id has every character that is not a letter or
//! a digit written as `_` and two hex digits, so it can hold a dash and still split cleanly.

use crate::model::{Kind, Shape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaId {
    pub kind: Kind,
    pub provider_id: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl MediaId {
    /// The title's own id, without the episode.
    pub fn key(&self) -> String {
        title_key(self.kind, &self.provider_id)
    }
}

pub fn title_key(kind: Kind, provider_id: &str) -> String {
    format!("{}-{}", kind.as_str(), provider_id)
}

/// A title's id, for a provider whose titles say where they come from.
pub fn qualified_id(provider: &str, raw: &str) -> String {
    format!("{provider}~{}", escape(raw))
}

fn escape(raw: &str) -> String {
    let mut escaped = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() {
            escaped.push(byte as char);
        } else {
            escaped.push_str(&format!("_{byte:02x}"));
        }
    }
    escaped
}

/// The provider and the service's own id of a title that says where it comes from. `None` for an
/// ordinary id (the built-in chain answers for those) and for one that is damaged.
pub fn split_qualified(provider_id: &str) -> Option<(&str, String)> {
    let (provider, escaped) = provider_id.split_once('~')?;
    let bytes = escaped.as_bytes();
    let mut raw = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'_' {
            let hex = escaped.get(at + 1..at + 3)?;
            raw.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            raw.push(bytes[at]);
            at += 1;
        }
    }
    let raw = String::from_utf8(raw).ok()?;
    // Only the one spelling `qualified_id` makes: `_4a` for a `J` would be a second key for one title.
    (!provider.is_empty() && !raw.is_empty() && escape(&raw) == escaped).then_some((provider, raw))
}

/// The id of an episode of a title whose episodes are numbered only (an anime shape).
pub fn anime_episode_id(kind: Kind, provider_id: &str, episode: u32) -> String {
    format!("{}-{provider_id}-ep{episode}", kind.as_str())
}

/// Parses a media id. The suffix must match the kind: a film has no episode and an
/// anime has no season, so a malformed id is rejected rather than reinterpreted.
pub fn parse(id: &str) -> Option<MediaId> {
    let (kind_text, rest) = id.split_once('-')?;
    let kind = Kind::parse(kind_text)?;
    let (provider_id, suffix) = match rest.split_once('-') {
        Some((provider_id, suffix)) => (provider_id, Some(suffix)),
        None => (rest, None),
    };
    let plain = provider_id.chars().all(|c| c.is_ascii_alphanumeric());
    if provider_id.is_empty() || !(plain || is_qualified(provider_id)) {
        return None;
    }

    let mut parsed = MediaId {
        kind,
        provider_id: provider_id.to_string(),
        season: None,
        episode: None,
    };
    let Some(suffix) = suffix else {
        return Some(parsed);
    };

    match kind.shape() {
        Shape::Movie => return None,
        Shape::Anime => parsed.episode = Some(digits(suffix.strip_prefix("ep")?)?),
        Shape::Series => {
            let (season, episode) = suffix.strip_prefix('s')?.split_once('e')?;
            parsed.season = Some(digits(season)?);
            parsed.episode = Some(digits(episode)?);
        }
    }
    Some(parsed)
}

/// Whether this is a well formed `provider~id`.
fn is_qualified(provider_id: &str) -> bool {
    provider_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '~' || c == '_')
        && provider_id.matches('~').count() == 1
        && split_qualified(provider_id).is_some()
}

fn digits(text: &str) -> Option<u32> {
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_that_says_where_it_comes_from_keeps_its_provider_and_its_id_whatever_is_in_it() {
        for raw in ["abc123", "abc-12", "3f2a-9b-ep4", "a_b", "a~b", "é"] {
            let id = format!("series-{}-s1e2", qualified_id("myprov", raw));
            let parsed = parse(&id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!((parsed.season, parsed.episode), (Some(1), Some(2)));
            let (provider, back) = split_qualified(&parsed.provider_id).unwrap();
            assert_eq!((provider, back.as_str()), ("myprov", raw), "{id}");
            assert_eq!(
                parsed.key(),
                format!("series-{}", qualified_id("myprov", raw))
            );
        }
        assert_eq!(qualified_id("p", "a-b"), "p~a_2db");
    }

    #[test]
    fn ids_that_only_look_qualified_are_rejected_and_plain_ones_have_no_provider() {
        for id in [
            "movie-~abc",
            "movie-p~",
            "movie-p~~a",
            "movie-p~a~b",
            "movie-p~a_zz",
            "movie-p~a_2",
            "movie-p_a",
            // Another spelling of an id that has one: upper case hex, a letter or digit written out.
            "movie-p~a_2D",
            "movie-p~_41",
            "movie-p~_4a",
        ] {
            assert!(parse(id).is_none(), "{id}");
        }
        assert!(split_qualified("tt32261958").is_none());
        assert!(parse("movie-tt32261958").is_some());
    }

    #[test]
    fn parses_each_kind() {
        let anime = parse("anime-16498-ep1").unwrap();
        assert_eq!(
            (anime.kind, anime.episode, anime.season),
            (Kind::Anime, Some(1), None)
        );
        let series = parse("series-169-s1e5").unwrap();
        assert_eq!((series.season, series.episode), (Some(1), Some(5)));
        let film = parse("movie-tt32261958").unwrap();
        assert_eq!(
            (film.provider_id.as_str(), film.episode),
            ("tt32261958", None)
        );
    }

    #[test]
    fn a_title_key_has_no_episode() {
        assert_eq!(parse("series-169").unwrap().episode, None);
        assert_eq!(parse("series-169-s2e3").unwrap().key(), "series-169");
    }

    #[test]
    fn rejects_a_suffix_that_does_not_match_the_kind() {
        assert!(parse("movie-7691-ep1").is_none());
        assert!(parse("anime-1-s1e1").is_none());
        assert!(parse("series-1-ep1").is_none());
        assert!(parse("series-1-s1").is_none());
        assert!(parse("book-1").is_none());
        assert!(parse("anime-").is_none());
        assert!(parse("anime-1-ep").is_none());
    }

    #[test]
    fn builds_ids_that_parse_back() {
        assert_eq!(
            parse(&anime_episode_id(Kind::Anime, "5", 3))
                .unwrap()
                .episode,
            Some(3)
        );
        let id = parse("series-tt1-s2e7").unwrap();
        assert_eq!((id.season, id.episode), (Some(2), Some(7)));
    }
}
