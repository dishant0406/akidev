//! Every kind of source behind one type: a definition read by the engine, or MangaDex's own client.

use crate::fetch::Fetcher;
use crate::mangadex::MangaDex;
use crate::model::{Card, Chapter, Detail, Page, PageList};
use crate::source::Source;
use std::collections::BTreeMap;

pub enum Catalogue {
    Definition(Box<Source>),
    MangaDex(MangaDex),
}

/// What an app needs to list a source before it asks it anything.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub nsfw: bool,
    pub version: u64,
    pub can_popular: bool,
    pub can_latest: bool,
    pub can_search: bool,
}

pub const MANGADEX_ID: &str = "mangadex";

impl Catalogue {
    pub fn info(&self) -> Info {
        match self {
            Catalogue::Definition(source) => {
                let def = source.def();
                Info {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    languages: def.lang.clone(),
                    nsfw: def.nsfw,
                    version: def.version,
                    can_popular: def.popular.is_some(),
                    can_latest: def.latest.is_some(),
                    can_search: def.search.is_some(),
                }
            }
            Catalogue::MangaDex(_) => Info { id: MANGADEX_ID.into(), name: "MangaDex".into(), languages: vec!["en".into()], nsfw: false, version: 1, can_popular: true, can_latest: true, can_search: true },
        }
    }

    pub fn fetcher(&self) -> &Fetcher {
        match self {
            Catalogue::Definition(source) => source.fetcher(),
            Catalogue::MangaDex(dex) => dex.fetcher(),
        }
    }

    pub async fn popular(&self, page: u32) -> anyhow::Result<Page<Card>> {
        match self {
            Catalogue::Definition(source) => source.popular(page).await,
            Catalogue::MangaDex(dex) => dex.popular(page).await,
        }
    }

    pub async fn latest(&self, page: u32) -> anyhow::Result<Page<Card>> {
        match self {
            Catalogue::Definition(source) => source.latest(page).await,
            Catalogue::MangaDex(dex) => dex.latest(page).await,
        }
    }

    pub async fn search(&self, term: &str, page: u32, filters: &BTreeMap<String, String>) -> anyhow::Result<Page<Card>> {
        match self {
            Catalogue::Definition(source) => source.search(term, page, filters).await,
            Catalogue::MangaDex(dex) => dex.search(term, page).await,
        }
    }

    pub async fn details(&self, id: &str) -> anyhow::Result<Detail> {
        match self {
            Catalogue::Definition(source) => source.details(id).await,
            Catalogue::MangaDex(dex) => dex.details(id).await,
        }
    }

    pub async fn chapters(&self, id: &str) -> anyhow::Result<Vec<Chapter>> {
        match self {
            Catalogue::Definition(source) => source.chapters(id).await,
            Catalogue::MangaDex(dex) => dex.chapters(id).await,
        }
    }

    pub async fn pages(&self, series: &str, chapter: &str) -> anyhow::Result<PageList> {
        match self {
            Catalogue::Definition(source) => source.pages(series, chapter).await,
            Catalogue::MangaDex(dex) => dex.pages(chapter).await,
        }
    }
}
