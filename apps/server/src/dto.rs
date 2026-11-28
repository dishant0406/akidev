//! The JSON the API sends. See API.md.
//!
//! These mirror the library's own types but leave out what only the server uses (the other names
//! a title is known by, the IMDb id), so the API stays small and the library stays free to change.

use akidev_core::model::{Detail, Episode, Kind, Media, Person, Trailer};
use akidev_core::screens::{HomeRow, RowStyle};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MediaOut {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub poster: Option<String>,
    pub banner: Option<String>,
    pub description: Option<String>,
    pub format: Option<String>,
    /// When the title first came out: "YYYY-MM-DD", "YYYY-MM" or "YYYY".
    pub release_date: Option<String>,
    /// For a title that is not out yet, the words to show: "Coming 27 January 2027".
    pub coming_soon: Option<String>,
    pub year: Option<i32>,
    pub episodes: Option<u32>,
    pub runtime_minutes: Option<u32>,
    pub status: Option<String>,
    pub score: Option<u32>,
    pub popularity: i64,
    pub genres: Vec<String>,
    pub is_adult: bool,
}

impl From<&Media> for MediaOut {
    fn from(media: &Media) -> MediaOut {
        MediaOut {
            id: media.id.clone(),
            kind: media.kind,
            title: media.title.clone(),
            poster: media.poster.clone(),
            banner: media.banner.clone(),
            description: media.description.clone(),
            format: media.format.clone(),
            release_date: media.release_date.clone(),
            coming_soon: media.coming_soon(akidev_core::calendar::now_ms()),
            year: media.year,
            episodes: media.episodes,
            runtime_minutes: media.runtime_minutes,
            status: media.status.clone(),
            score: media.score,
            popularity: media.popularity,
            genres: media.genres.clone(),
            is_adult: media.is_adult,
        }
    }
}

pub fn media_list(list: &[Media]) -> Vec<MediaOut> {
    list.iter().map(MediaOut::from).collect()
}

#[derive(Debug, Serialize)]
pub struct PageOut {
    pub results: Vec<MediaOut>,
    pub has_next: bool,
}

#[derive(Debug, Serialize)]
pub struct RowOut {
    pub title: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "RowStyle::is_poster")]
    pub style: RowStyle,
    pub items: Vec<MediaOut>,
}

impl RowOut {
    /// A row of the home page as the apps get it: a ranked row keeps its first ten titles.
    pub fn of(row: &HomeRow, mut titles: Vec<Media>) -> RowOut {
        row.style.trim(&mut titles);
        RowOut {
            title: row.title.clone(),
            kind: row.kind,
            style: row.style,
            items: media_list(&titles),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HomeOut {
    pub featured: Vec<MediaOut>,
    pub rows: Vec<RowOut>,
}

#[derive(Debug, Serialize)]
pub struct DetailOut {
    pub media: MediaOut,
    pub studios: Vec<String>,
    pub trailer: Option<Trailer>,
    pub episodes: Vec<Episode>,
    pub cast: Vec<Person>,
    pub related: Vec<MediaOut>,
    pub recommended: Vec<MediaOut>,
}

impl From<&Detail> for DetailOut {
    fn from(detail: &Detail) -> DetailOut {
        DetailOut {
            media: MediaOut::from(&detail.media),
            studios: detail.studios.clone(),
            trailer: detail.trailer.clone(),
            episodes: detail.episodes.clone(),
            cast: detail.cast.clone(),
            related: media_list(&detail.related),
            recommended: media_list(&detail.recommended),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Subtitle {
    pub label: String,
    pub lang: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ready {
    pub available: bool,
    pub url: String,
    pub stream_type: &'static str,
    pub release: String,
    /// Bare hash or file-scoped selection id: what a watch party names so every device plays
    /// the same file (`release` in the request plays exactly that one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<String>,
    pub source: &'static str,
    pub duration: Option<f64>,
    pub subtitles: Vec<Subtitle>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_title() -> Media {
        Media {
            id: "anime-1".into(),
            kind: Kind::Anime,
            title: "T".into(),
            title_variants: vec!["secret".into()],
            poster: None,
            banner: None,
            description: None,
            format: None,
            year: Some(2020),
            episodes: Some(12),
            runtime_minutes: None,
            status: None,
            score: Some(80),
            popularity: 5,
            genres: vec!["Action".into()],
            is_adult: false,
            next_episode: None,
            release_date: None,
        }
    }

    #[test]
    fn a_ranked_home_row_keeps_ten_titles_and_says_so_and_a_plain_row_keeps_all() {
        let row = |style| HomeRow {
            title: "Row".into(),
            kind: Kind::Anime,
            sort: akidev_core::model::Sort::Trending,
            genre: None,
            style,
            provider: None,
        };
        let twenty_four = || vec![a_title(); 24];
        let ranked =
            serde_json::to_value(RowOut::of(&row(RowStyle::Ranked), twenty_four())).unwrap();
        assert_eq!(ranked["items"].as_array().unwrap().len(), 10);
        assert_eq!(ranked["style"], "ranked");
        let plain =
            serde_json::to_value(RowOut::of(&row(RowStyle::Poster), twenty_four())).unwrap();
        assert_eq!(plain["items"].as_array().unwrap().len(), 24);
        assert!(plain.get("style").is_none());
    }

    #[test]
    fn the_media_json_has_the_fields_the_contract_names_and_no_more() {
        let media = a_title();
        let json = serde_json::to_value(MediaOut::from(&media)).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "banner",
                "coming_soon",
                "description",
                "episodes",
                "format",
                "genres",
                "id",
                "is_adult",
                "kind",
                "popularity",
                "poster",
                "release_date",
                "runtime_minutes",
                "score",
                "status",
                "title",
                "year"
            ]
        );
        assert_eq!(json["kind"], "anime");
    }
}
