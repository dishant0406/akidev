//! What the engine returns. Plain data, serialised for the golden tests and the Swift facade.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    /// How many chapters the list says the series has, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ongoing,
    Completed,
    Hiatus,
    Cancelled,
    Unknown,
}

impl Status {
    pub fn parse(text: &str) -> Status {
        let text = text.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| text.contains(word));
        if has(&["hiatus"]) {
            Status::Hiatus
        } else if has(&["cancel", "dropped", "discontinued"]) {
            Status::Cancelled
        } else if has(&["complete", "finished", "ended"]) {
            Status::Completed
        } else if has(&["ongoing", "publishing", "updating", "releasing"]) {
            Status::Ongoing
        } else {
            Status::Unknown
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detail {
    pub title: String,
    pub cover: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub number: Option<f64>,
    pub title: Option<String>,
    /// Milliseconds since the epoch, when the source says when it came out.
    pub date_ms: Option<i64>,
    pub scanlator: Option<String>,
}

/// How a chapter wants to be read: manhwa is a long strip, manga pages turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reading {
    LongStrip,
    Paged,
    PagedRtl,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageList {
    pub images: Vec<String>,
    pub reading: Reading,
    /// Headers every image request must carry (a referer, usually).
    pub headers: BTreeMap<String, String>,
}
