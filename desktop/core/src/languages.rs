//! The languages the engine knows, as data (`languages` in the sources document).
//!
//! Three things read this list: the subtitle sources (which code and name to show), the release
//! ranking (which words in a release name name a language) and the same-name filter (which words
//! prove a release is from a country). Adding a language is a publish, not a build.

use crate::sources::{self, Sources};
use anyhow::ensure;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Language {
    /// The code shown to the viewer: `en`, `pt-BR`.
    pub code: String,
    /// The name shown to the viewer: "English".
    pub name: String,
    /// The codes the sources use for it, in lower case: `en`, `eng`.
    pub codes: Vec<String>,
    /// Words that name it in a release title: `korean`.
    #[serde(default)]
    pub words: Vec<String>,
    /// Short tags a release can carry for it: `kor`. Too short to count as a language word on their
    /// own, so they only help to prove a country.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Parts of a country name (or a language name) that mean this language: `korea`, `korean`.
    #[serde(default)]
    pub countries: Vec<String>,
}

impl Language {
    pub(crate) fn check(&self) -> anyhow::Result<()> {
        ensure!(
            !self.code.is_empty() && self.code.len() <= 16,
            "the code is empty or too long"
        );
        ensure!(
            !self.name.is_empty() && self.name.len() <= 64,
            "the name is empty or too long"
        );
        ensure!(
            !self.codes.is_empty() && self.codes.len() <= 8,
            "it needs 1 to 8 codes"
        );
        let spelling = |text: &String| {
            (1..=32).contains(&text.len())
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        };
        ensure!(
            self.codes.iter().all(spelling),
            "codes are lower case letters, digits and dashes"
        );
        ensure!(
            self.words.len() <= 16 && self.tags.len() <= 16,
            "too many words"
        );
        let word = |text: &String| {
            (1..=32).contains(&text.len())
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        };
        ensure!(
            self.words.iter().chain(&self.tags).all(word),
            "words and tags are lower case letters and digits"
        );
        ensure!(
            self.countries.len() <= 16
                && self
                    .countries
                    .iter()
                    .all(|part| (1..=40).contains(&part.len()) && *part == part.to_lowercase()),
            "countries are 1 to 16 lower case parts"
        );
        Ok(())
    }
}

/// The code and the name to show for a code a source used, or `None` for a language that is not
/// known (the subtitle is then left out: a menu entry nobody can read is no help).
pub fn find(code: &str) -> Option<(String, String)> {
    let wanted = code.trim().to_lowercase().replace('_', "-");
    sources::current()
        .languages_or_built_in()
        .iter()
        .find(|language| language.codes.contains(&wanted))
        .map(|language| (language.code.clone(), language.name.clone()))
}

/// Every word that names a language in a release title.
pub fn words(sources: &Sources) -> Vec<String> {
    sources
        .languages_or_built_in()
        .iter()
        .flat_map(|language| language.words.iter().cloned())
        .collect()
}

/// The words and tags that prove a release is from this country (or in this language), or `None`
/// when no language claims it, so nothing can be asked of the release.
pub fn markers_for(country: &str) -> Option<Vec<String>> {
    let country = country.to_lowercase();
    let markers: Vec<String> = sources::current()
        .languages_or_built_in()
        .iter()
        .filter(|language| language.countries.iter().any(|part| country.contains(part)))
        .flat_map(|language| language.words.iter().chain(&language.tags).cloned())
        .collect();
    (!markers.is_empty()).then_some(markers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_finds_its_language_whatever_the_spelling() {
        assert_eq!(find("eng"), Some(("en".into(), "English".into())));
        assert_eq!(
            find(" PT_br "),
            Some(("pt-BR".into(), "Portuguese (Brazil)".into()))
        );
        assert_eq!(find("zht").unwrap().0, "zh-TW");
        assert_eq!(find("EN"), Some(("en".into(), "English".into())));
        assert_eq!(find("fre"), find("fra"));
        assert_eq!(find("zh_TW").unwrap().0, "zh-TW");
        assert_eq!(find("xx"), None);
        assert_eq!(find(""), None);
    }

    #[test]
    fn a_country_or_a_language_name_gives_the_words_that_prove_it() {
        let korea = markers_for("South Korea").unwrap();
        assert!(
            korea.iter().any(|word| word == "korean") && korea.iter().any(|word| word == "kor")
        );
        assert_eq!(markers_for("Korean"), Some(korea));
        let china = markers_for("Hong Kong").unwrap();
        assert!(
            china.iter().any(|word| word == "cantonese") && china.iter().any(|word| word == "hk")
        );
        assert_eq!(markers_for("United States"), None);
    }

    #[test]
    fn the_embedded_languages_are_valid_and_no_code_is_claimed_twice() {
        let sources = sources::current();
        let all = sources.languages_or_built_in();
        for language in all {
            language.check().unwrap();
        }
        let mut spellings: Vec<&String> = all.iter().flat_map(|language| &language.codes).collect();
        spellings.sort();
        let before = spellings.len();
        spellings.dedup();
        assert_eq!(before, spellings.len());
    }
}
