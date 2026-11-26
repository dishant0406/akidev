//! The "share card": a small file that says which series from which source a friend is reading, so their phone can add
//! the source and the series and read it from the site. No chapter images travel; only where to find them.

use super::clean_name;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A card is a few kilobytes; anything near this is not one.
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    /// The card's own version, for the day it changes.
    pub card: u32,
    pub title: String,
    pub cover: Option<String>,
    pub series: String,
    pub source: String,
    /// What the other phone is asked about before the source is added.
    pub source_name: String,
    pub nsfw: bool,
    pub languages: Vec<String>,
    /// Where the source came from, when it came from a repository.
    pub repo: Option<String>,
    /// The source itself, when it was added by hand and so has no repository to fetch it from.
    pub definition: Option<String>,
}

impl Card {
    /// The name the file is sent under.
    pub fn file_name(&self) -> String {
        format!("{}.akidev.json", clean_name(&self.title))
    }

    pub fn write(&self, path: &Path) -> anyhow::Result<()> {
        Ok(std::fs::write(path, serde_json::to_vec(self)?)?)
    }

    /// Reads a card that came from another phone, so every part of it is checked.
    pub fn read(path: &Path) -> anyhow::Result<Card> {
        let size = std::fs::metadata(path)
            .context("cannot open the card")?
            .len();
        anyhow::ensure!(size <= MAX_BYTES, "that is too large to be a share card");
        let mut card: Card =
            serde_json::from_slice(&std::fs::read(path)?).context("that is not a share card")?;
        anyhow::ensure!(
            card.card == 1,
            "this card is from a newer version of the app"
        );
        anyhow::ensure!(
            !card.series.is_empty() && !card.source.is_empty(),
            "the card names no series"
        );
        anyhow::ensure!(
            card.repo.is_some() || card.definition.is_some(),
            "the card does not say where its source is"
        );
        // A cover is a picture on a site. A file on the sender's phone means nothing here.
        card.cover = card
            .cover
            .filter(|cover| cover.starts_with("https://") || cover.starts_with("http://"));
        Ok(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            card: 1,
            title: "One / Piece".into(),
            cover: Some("https://example.com/c.jpg".into()),
            series: "one-piece".into(),
            source: "mangadex".into(),
            source_name: "MangaDex".into(),
            nsfw: false,
            languages: vec!["en".into()],
            repo: Some("https://example.com/repo".into()),
            definition: None,
        }
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("akidev-card-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_card_reads_back_as_written() {
        let path = folder("same").join("c.akidev.json");
        card().write(&path).unwrap();
        assert_eq!(Card::read(&path).unwrap(), card());
        assert_eq!(card().file_name(), "Piece.akidev.json");
    }

    #[test]
    fn a_cover_on_the_senders_phone_is_dropped() {
        let path = folder("cover").join("c.json");
        Card {
            cover: Some("file:///var/mobile/x.jpg".into()),
            ..card()
        }
        .write(&path)
        .unwrap();
        assert_eq!(Card::read(&path).unwrap().cover, None);
    }

    #[test]
    fn a_card_with_no_source_location_is_refused() {
        let path = folder("where").join("c.json");
        Card {
            repo: None,
            ..card()
        }
        .write(&path)
        .unwrap();
        assert!(Card::read(&path).is_err());
    }

    #[test]
    fn something_else_is_refused() {
        let dir = folder("else");
        std::fs::write(dir.join("a.json"), "[1,2]").unwrap();
        assert!(Card::read(&dir.join("a.json")).is_err());
        std::fs::write(dir.join("b.json"), vec![b' '; 300 * 1024]).unwrap();
        assert!(Card::read(&dir.join("b.json")).is_err());
    }
}
