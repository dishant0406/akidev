//! What the screens show: the tabs, the sorts, the genres and the rows of the home page.
//!
//! It is part of the sources document, so the desktop app and the website draw the same screens from
//! data, and a change is a publish rather than a build.

use crate::model::{Kind, Sort};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Pages of the website that a tab must not take the address of.
const RESERVED_SLUGS: [&str; 7] = [
    "search", "title", "watch", "download", "privacy", "manga", "info",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screens {
    /// The catalogues, in the order of the top bar.
    pub tabs: Vec<Tab>,
    /// The ways a catalogue can be ordered. The first is the one it opens on.
    pub sorts: Vec<SortOption>,
    /// The genres worth offering, by kind name, in the providers' own spellings. `all` is for the
    /// page that shows every catalogue at once.
    pub genres: BTreeMap<String, Vec<String>>,
    /// The rows of the home page, in order.
    pub home: Vec<HomeRow>,
    /// The credits the website shows.
    pub about: About,
}

/// What the website says about where its data comes from, so adding or dropping a source is a
/// change to the document and not to the site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct About {
    /// The line in the footer: the credit the catalogues require, and that nothing is stored.
    pub attribution: String,
    /// The catalogues, by name, as the privacy page lists them.
    pub catalogues: Vec<String>,
    /// The subtitle sources, by name, as the privacy page lists them.
    pub subtitles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub kind: Kind,
    /// The address segment of the website: `/films`.
    pub slug: String,
    /// The tab in the top bar and the heading of the page.
    pub label: String,
    /// The small label over a title of this kind: "FILM".
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortOption {
    pub sort: Sort,
    pub label: String,
}

/// How many titles a ranked row shows.
pub const RANKED_LEN: usize = 10;

/// How a home row is drawn. An app that does not know a style leaves out the key and draws a plain row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowStyle {
    #[default]
    Poster,
    /// A "Top 10": a big number beside each of the first ten titles.
    Ranked,
}

impl RowStyle {
    pub fn is_poster(&self) -> bool {
        *self == RowStyle::Poster
    }

    /// Cuts the titles of a ranked row to the ones it shows.
    pub fn trim<T>(&self, titles: &mut Vec<T>) {
        if *self == RowStyle::Ranked {
            titles.truncate(RANKED_LEN);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeRow {
    pub title: String,
    pub kind: Kind,
    pub sort: Sort,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "RowStyle::is_poster")]
    pub style: RowStyle,
    /// The provider the row draws from alone. Absent: the kind's chain answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl Screens {
    /// The kinds that have a tab, in the order of the tabs.
    pub fn kinds(&self) -> Vec<Kind> {
        self.tabs.iter().map(|tab| tab.kind).collect()
    }

    /// The genres to offer for one catalogue, or for all of them when `kind` is `None`.
    pub fn genres(&self, kind: Option<Kind>) -> &[String] {
        self.genres
            .get(kind.map_or("all", Kind::as_str))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=16).contains(&self.tabs.len()),
            "the tabs are out of range"
        );
        for (index, tab) in self.tabs.iter().enumerate() {
            ensure!(
                !tab.slug.is_empty()
                    && tab
                        .slug
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
                "the tab slug {:?} is not lower case letters",
                tab.slug
            );
            ensure!(
                !RESERVED_SLUGS.contains(&tab.slug.as_str()),
                "the tab slug {:?} is already a page of the site",
                tab.slug
            );
            ensure!(
                !tab.label.is_empty() && !tab.tag.is_empty(),
                "a tab has no label or tag"
            );
            ensure!(
                self.tabs[..index].iter().all(|other| other.slug != tab.slug
                    && other.kind != tab.kind
                    && other.label != tab.label),
                "the tab {} repeats a slug, a kind or a label",
                tab.slug
            );
        }
        ensure!(
            (1..=16).contains(&self.sorts.len()),
            "the sorts are out of range"
        );
        ensure!(
            self.sorts.iter().all(|option| !option.label.is_empty()),
            "a sort has no label"
        );
        for (name, genres) in &self.genres {
            ensure!(
                name == "all" || Kind::parse(name).is_some(),
                "genres for {name:?}, which is not a kind"
            );
            ensure!(genres.len() <= 64, "too many genres for {name}");
        }
        let about = &self.about;
        ensure!(
            !about.attribution.is_empty() && about.attribution.len() <= 1000,
            "the attribution is empty or too long"
        );
        ensure!(
            about.catalogues.len() <= 32
                && about.subtitles.len() <= 32
                && about
                    .catalogues
                    .iter()
                    .chain(&about.subtitles)
                    .all(|name| !name.is_empty() && name.len() <= 64),
            "the credits are out of range"
        );
        ensure!(
            (1..=32).contains(&self.home.len()),
            "the home rows are out of range"
        );
        for row in &self.home {
            ensure!(!row.title.is_empty(), "a home row has no title");
            ensure!(
                self.tabs.iter().any(|tab| tab.kind == row.kind),
                "the row {:?} is for a kind with no tab",
                row.title
            );
        }
        Ok(())
    }
}
