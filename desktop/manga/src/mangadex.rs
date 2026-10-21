//! MangaDex, through its own API: the one source that is official and wants third-party readers.
//!
//! It follows MangaDex's rules: an honest `User-Agent` that names this app, no `Via` header, and at
//! most five requests a second per address (a minimum they guarantee, not a cap they enforce
//! exactly, so this asks for fewer). Images come from the server it hands out for each chapter and
//! are never proxied.
//!
//! Written from MangaDex's public API documentation as remembered; the live documentation could not
//! be reached when this was written (see pipeline/MANGA_PLAN.md), so the shapes below are checked
//! only against recorded answers until someone runs it against the real API.

use crate::fetch::{Fetcher, Req};
use crate::model::{Card, Chapter, Detail, Page, PageList, Reading, Status};
use crate::template::percent_encode;
use anyhow::Context;
use serde_json::Value;
use std::collections::BTreeMap;

pub const BASE: &str = "https://api.mangadex.org";
const COVERS: &str = "https://uploads.mangadex.org/covers";
const PAGE_SIZE: u32 = 20;

pub struct MangaDex {
    fetcher: Fetcher,
    base: String,
    languages: Vec<String>,
    /// `safe`, `suggestive`, `erotica`, `pornographic`: safe and suggestive unless the person opts in.
    ratings: Vec<String>,
}

fn user_agent() -> String {
    format!("akidev-manga/{}", env!("CARGO_PKG_VERSION"))
}

impl MangaDex {
    pub fn live() -> MangaDex {
        let fetcher = Fetcher::live(Some(4.0));
        fetcher.set_user_agent(&user_agent());
        MangaDex::with(fetcher, BASE)
    }

    pub fn with(fetcher: Fetcher, base: &str) -> MangaDex {
        MangaDex { fetcher, base: base.trim_end_matches('/').to_string(), languages: vec!["en".into()], ratings: vec!["safe".into(), "suggestive".into()] }
    }

    pub fn languages(mut self, languages: &[&str]) -> MangaDex {
        self.languages = languages.iter().map(|language| language.to_string()).collect();
        self
    }

    pub fn ratings(mut self, ratings: &[&str]) -> MangaDex {
        self.ratings = ratings.iter().map(|rating| rating.to_string()).collect();
        self
    }

    pub fn fetcher(&self) -> &Fetcher {
        &self.fetcher
    }

    async fn get(&self, path_and_query: &str) -> anyhow::Result<Value> {
        let req = Req { method: "GET".into(), url: format!("{}{path_and_query}", self.base), headers: vec![("accept".into(), "application/json".into())], form: vec![], json: None };
        let body = self.fetcher.send(&req).await?;
        let value: Value = serde_json::from_str(&body).context("MangaDex did not answer with JSON")?;
        if value["result"] == "error" {
            anyhow::bail!("MangaDex: {}", value["errors"][0]["detail"].as_str().unwrap_or("an error"));
        }
        Ok(value)
    }

    fn ratings_query(&self) -> String {
        self.ratings.iter().map(|rating| format!("&contentRating[]={rating}")).collect()
    }

    fn languages_query(&self) -> String {
        self.languages.iter().map(|language| format!("&availableTranslatedLanguage[]={language}")).collect()
    }

    async fn list(&self, order: &str, term: Option<&str>, page: u32) -> anyhow::Result<Page<Card>> {
        let offset = page.max(1).saturating_sub(1) * PAGE_SIZE;
        let title = term.map(|term| format!("&title={}", percent_encode(term))).unwrap_or_default();
        let query = format!("/manga?limit={PAGE_SIZE}&offset={offset}&includes[]=cover_art&{order}{title}{}{}", self.ratings_query(), self.languages_query());
        let answer = self.get(&query).await?;
        let items: Vec<Card> = answer["data"].as_array().map(Vec::as_slice).unwrap_or_default().iter().filter_map(card).collect();
        let total = answer["total"].as_u64().unwrap_or(0);
        Ok(Page { items, has_next: u64::from(offset + PAGE_SIZE) < total })
    }

    pub async fn popular(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list("order[followedCount]=desc", None, page).await
    }

    pub async fn latest(&self, page: u32) -> anyhow::Result<Page<Card>> {
        self.list("order[latestUploadedChapter]=desc", None, page).await
    }

    pub async fn search(&self, term: &str, page: u32) -> anyhow::Result<Page<Card>> {
        self.list("order[relevance]=desc", Some(term), page).await
    }

    pub async fn details(&self, id: &str) -> anyhow::Result<Detail> {
        let answer = self.get(&format!("/manga/{}?includes[]=cover_art&includes[]=author&includes[]=artist", percent_encode(id))).await?;
        let data = &answer["data"];
        let attributes = &data["attributes"];
        let title = localized(&attributes["title"]).context("the title has no name")?;
        let people: Vec<String> = relations(data, &["author", "artist"]).filter_map(|person| person["attributes"]["name"].as_str().map(str::to_string)).fold(Vec::new(), |mut all, name| {
            if !all.contains(&name) {
                all.push(name);
            }
            all
        });
        Ok(Detail {
            title,
            cover: cover_of(data),
            description: localized(&attributes["description"]),
            authors: people,
            tags: attributes["tags"].as_array().map(Vec::as_slice).unwrap_or_default().iter().filter_map(|tag| localized(&tag["attributes"]["name"])).collect(),
            status: Status::parse(attributes["status"].as_str().unwrap_or_default()),
        })
    }

    /// Every chapter in the person's languages, newest first. External chapters (a link to another
    /// site, nothing to read here) are left out.
    pub async fn chapters(&self, id: &str) -> anyhow::Result<Vec<Chapter>> {
        let languages: String = self.languages.iter().map(|language| format!("&translatedLanguage[]={language}")).collect();
        let mut all: Vec<Chapter> = Vec::new();
        let mut offset = 0u64;
        loop {
            let query = format!(
                "/manga/{}/feed?limit=500&offset={offset}&order[chapter]=desc&order[volume]=desc&includeExternalUrl=0&includes[]=scanlation_group{languages}{}",
                percent_encode(id),
                self.ratings_query()
            );
            let answer = self.get(&query).await?;
            let rows = answer["data"].as_array().map(Vec::as_slice).unwrap_or_default();
            for row in rows {
                let attributes = &row["attributes"];
                let (Some(chapter_id), false) = (row["id"].as_str(), attributes["externalUrl"].is_string()) else { continue };
                if attributes["pages"].as_u64() == Some(0) {
                    continue;
                }
                all.push(Chapter {
                    id: chapter_id.to_string(),
                    number: attributes["chapter"].as_str().and_then(|number| number.parse().ok()),
                    title: attributes["title"].as_str().filter(|title| !title.is_empty()).map(str::to_string),
                    date_ms: attributes["publishAt"].as_str().and_then(|at| crate::date::parse(at, &["iso".to_string()], 0)),
                    scanlator: relations(row, &["scanlation_group"]).find_map(|group| group["attributes"]["name"].as_str().map(str::to_string)),
                });
            }
            offset += rows.len() as u64;
            if rows.is_empty() || offset >= answer["total"].as_u64().unwrap_or(0) {
                break;
            }
        }
        Ok(all)
    }

    /// The pages of a chapter, from the image server MangaDex hands out for it.
    pub async fn pages(&self, chapter: &str) -> anyhow::Result<PageList> {
        let answer = self.get(&format!("/at-home/server/{}", percent_encode(chapter))).await?;
        let base = answer["baseUrl"].as_str().context("no image server")?;
        let hash = answer["chapter"]["hash"].as_str().context("no chapter hash")?;
        let files: Vec<&str> = answer["chapter"]["data"].as_array().map(Vec::as_slice).unwrap_or_default().iter().filter_map(Value::as_str).collect();
        anyhow::ensure!(!files.is_empty(), "the chapter has no pages");
        Ok(PageList { images: files.iter().map(|file| format!("{base}/data/{hash}/{file}")).collect(), reading: Reading::Unknown, headers: BTreeMap::new() })
    }
}

/// The text in a language map ({"en": "..."}): English, else the first there is.
fn localized(map: &Value) -> Option<String> {
    let map = map.as_object()?;
    map.get("en").or_else(|| map.values().next())?.as_str().filter(|text| !text.is_empty()).map(str::to_string)
}

fn relations<'a>(data: &'a Value, kinds: &'a [&str]) -> impl Iterator<Item = &'a Value> {
    data["relationships"].as_array().map(Vec::as_slice).unwrap_or_default().iter().filter(move |relation| relation["type"].as_str().is_some_and(|kind| kinds.contains(&kind)))
}

fn cover_of(data: &Value) -> Option<String> {
    let file = relations(data, &["cover_art"]).find_map(|cover| cover["attributes"]["fileName"].as_str())?;
    Some(format!("{COVERS}/{}/{file}.512.jpg", data["id"].as_str()?))
}

fn card(row: &Value) -> Option<Card> {
    Some(Card { id: row["id"].as_str()?.to_string(), title: localized(&row["attributes"]["title"])?, cover: cover_of(row) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn dex(answers: &[(&str, &str)]) -> MangaDex {
        let answers: HashMap<String, String> = answers.iter().map(|(url, body)| (format!("GET https://api.test{url}"), body.to_string())).collect();
        MangaDex::with(Fetcher::recorded(answers), "https://api.test")
    }

    const LIST: &str = r#"{"result":"ok","total":45,"data":[
        {"id":"m1","attributes":{"title":{"ja-ro":"Ore dake","en":"Solo Leveling"}},"relationships":[{"type":"cover_art","attributes":{"fileName":"c1.jpg"}}]},
        {"id":"m2","attributes":{"title":{"ko-ro":"Only Title"}},"relationships":[]}]}"#;

    #[tokio::test]
    async fn popular_reads_titles_in_english_first_and_covers_from_the_relationship() {
        let url = "/manga?limit=20&offset=0&includes[]=cover_art&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&availableTranslatedLanguage[]=en";
        let page = dex(&[(url, LIST)]).popular(1).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].title, "Solo Leveling");
        assert_eq!(page.items[0].cover.as_deref(), Some("https://uploads.mangadex.org/covers/m1/c1.jpg.512.jpg"));
        assert_eq!(page.items[1].title, "Only Title");
        assert_eq!(page.items[1].cover, None);
        assert!(page.has_next);
    }

    #[tokio::test]
    async fn chapters_skip_external_and_empty_ones_and_read_the_group() {
        let feed = r#"{"result":"ok","total":3,"data":[
          {"id":"c3","attributes":{"chapter":"3","title":"Three","publishAt":"2024-03-05T10:00:00+00:00","pages":20},"relationships":[{"type":"scanlation_group","attributes":{"name":"Team A"}}]},
          {"id":"c2","attributes":{"chapter":"2","externalUrl":"https://elsewhere.test/2","pages":0},"relationships":[]},
          {"id":"c1","attributes":{"chapter":"1.5","title":"","pages":0},"relationships":[]}]}"#;
        let url = "/manga/m1/feed?limit=500&offset=0&order[chapter]=desc&order[volume]=desc&includeExternalUrl=0&includes[]=scanlation_group&translatedLanguage[]=en&contentRating[]=safe&contentRating[]=suggestive";
        let chapters = dex(&[(url, feed)]).chapters("m1").await.unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].id, "c3");
        assert_eq!(chapters[0].number, Some(3.0));
        assert_eq!(chapters[0].scanlator.as_deref(), Some("Team A"));
        assert_eq!(chapters[0].date_ms, Some(1_709_596_800_000));
    }

    #[tokio::test]
    async fn pages_use_the_server_the_chapter_is_given() {
        let home = r#"{"result":"ok","baseUrl":"https://s1.test","chapter":{"hash":"abc","data":["1.png","2.png"],"dataSaver":["1.jpg"]}}"#;
        let list = dex(&[("/at-home/server/c3", home)]).pages("c3").await.unwrap();
        assert_eq!(list.images, vec!["https://s1.test/data/abc/1.png", "https://s1.test/data/abc/2.png"]);
    }

    #[tokio::test]
    async fn an_error_answer_is_reported_with_its_detail() {
        let error = dex(&[("/at-home/server/x", r#"{"result":"error","errors":[{"detail":"Not found"}]}"#)]).pages("x").await.unwrap_err();
        assert_eq!(error.to_string(), "MangaDex: Not found");
    }

    #[test]
    fn the_user_agent_names_this_app_and_is_never_a_browser() {
        assert!(user_agent().starts_with("akidev-manga/"));
    }
}
