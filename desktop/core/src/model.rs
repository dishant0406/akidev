//! The shapes every provider produces and the UI renders.

use serde::{Deserialize, Serialize};

/// What a kind of title is like, which decides how its ids, episodes and playback work: an anime has
/// episodes only by number, a series has seasons, a film has no episodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Anime,
    Series,
    Movie,
}

struct KindInfo {
    id: &'static str,
    shape: Shape,
    /// What one title of it is called: "Film".
    singular: &'static str,
    /// Made known by a plugin, and so not to be taken for one that merely turned up in saved data.
    declared: bool,
}

/// The most kinds there can be: the built-in three, plugins' and any that saved data names.
const MAX_KINDS: usize = 32;

fn registry() -> &'static std::sync::RwLock<Vec<KindInfo>> {
    static KINDS: std::sync::OnceLock<std::sync::RwLock<Vec<KindInfo>>> =
        std::sync::OnceLock::new();
    KINDS.get_or_init(|| {
        let built_in = |id, shape, singular| KindInfo {
            id,
            shape,
            singular,
            declared: true,
        };
        std::sync::RwLock::new(vec![
            built_in("anime", Shape::Anime, "Anime"),
            built_in("series", Shape::Series, "Series"),
            built_in("movie", Shape::Movie, "Film"),
        ])
    })
}

/// A kind of title. The three built-in ones are constants; a plugin can bring more (`Kind::register`).
/// It is a small copyable id, which is what lets it be used as a key and a field everywhere.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Kind(&'static str);

#[allow(non_upper_case_globals)]
impl Kind {
    pub const Anime: Kind = Kind("anime");
    pub const Series: Kind = Kind("series");
    pub const Movie: Kind = Kind("movie");
    /// The kinds every build has.
    pub const BUILT_IN: [Kind; 3] = [Kind::Anime, Kind::Series, Kind::Movie];
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// The kind with this name, if it is one that is known.
    pub fn parse(text: &str) -> Option<Kind> {
        registry()
            .read()
            .unwrap()
            .iter()
            .find(|info| info.id == text)
            .map(|info| Kind(info.id))
    }

    pub fn shape(self) -> Shape {
        self.info(|info| info.shape).unwrap_or(Shape::Series)
    }

    /// What one title of this kind is called: "Film".
    pub fn singular(self) -> &'static str {
        self.info(|info| info.singular).unwrap_or(self.0)
    }

    fn info<T>(self, read: impl Fn(&KindInfo) -> T) -> Option<T> {
        registry()
            .read()
            .unwrap()
            .iter()
            .find(|info| info.id == self.0)
            .map(read)
    }

    /// Makes a kind known. The name is 1 to 16 lower case letters. A built-in kind keeps its shape; any
    /// other can be declared again, and the last declaration counts.
    pub fn register(id: &str, shape: Shape, singular: &str) -> anyhow::Result<Kind> {
        anyhow::ensure!(is_kind_name(id), "{id:?} is not a kind name");
        let mut kinds = registry().write().unwrap();
        if let Some(info) = kinds.iter_mut().find(|info| info.id == id) {
            if Kind::BUILT_IN.iter().any(|kind| kind.0 == id) {
                anyhow::ensure!(info.shape == shape, "{id} is a kind there already is");
                return Ok(Kind(info.id));
            }
            // Declared again, as a plugin that is updated does: the last word counts. Two plugins that
            // are on together cannot bring one kind, which the tabs refuse.
            info.shape = shape;
            if info.singular != singular {
                info.singular = leak(singular);
            }
            info.declared = true;
            return Ok(Kind(info.id));
        }
        anyhow::ensure!(kinds.len() < MAX_KINDS, "too many kinds");
        let info = KindInfo {
            id: leak(id),
            shape,
            singular: leak(singular),
            declared: true,
        };
        let kind = Kind(info.id);
        kinds.push(info);
        Ok(kind)
    }

    /// A kind that saved data names. If nothing has declared it (a plugin that was removed), it is kept
    /// so the data still reads, and behaves like a series.
    fn named_in_data(text: &str) -> Option<Kind> {
        if let Some(kind) = Kind::parse(text) {
            return Some(kind);
        }
        if !is_kind_name(text) {
            return None;
        }
        let mut kinds = registry().write().unwrap();
        if kinds.len() >= MAX_KINDS {
            return None;
        }
        let info = KindInfo {
            id: leak(text),
            shape: Shape::Series,
            singular: leak(text),
            declared: false,
        };
        let kind = Kind(info.id);
        kinds.push(info);
        Some(kind)
    }
}

fn is_kind_name(text: &str) -> bool {
    (1..=16).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// A name kept for as long as the program runs. There are at most `MAX_KINDS` of them.
fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

impl std::fmt::Debug for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for Kind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

thread_local! {
    static ONLY_KNOWN_KINDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While it lives, reading a kind accepts only one that is known, instead of keeping any name that
/// turns up. A plugin file is read this way, so a file that is refused leaves nothing behind.
pub(crate) struct OnlyKnownKinds;

impl OnlyKnownKinds {
    pub(crate) fn begin() -> OnlyKnownKinds {
        ONLY_KNOWN_KINDS.with(|only| only.set(true));
        OnlyKnownKinds
    }
}

impl Drop for OnlyKnownKinds {
    fn drop(&mut self) {
        ONLY_KNOWN_KINDS.with(|only| only.set(false));
    }
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Kind, D::Error> {
        let text = String::deserialize(deserializer)?;
        let found = if ONLY_KNOWN_KINDS.with(std::cell::Cell::get) {
            Kind::parse(&text)
        } else {
            Kind::named_in_data(&text)
        };
        found.ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not a kind")))
    }
}

/// How a browse row or a search is ordered. Providers map these onto what they have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    Trending,
    Popularity,
    Score,
    Recent,
    Title,
}

impl Sort {
    /// The name the definitions use for this sort.
    pub const ALL: [Sort; 5] = [
        Sort::Trending,
        Sort::Popularity,
        Sort::Score,
        Sort::Recent,
        Sort::Title,
    ];

    pub fn parse(text: &str) -> Option<Sort> {
        Sort::ALL.into_iter().find(|sort| sort.as_str() == text)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Sort::Trending => "trending",
            Sort::Popularity => "popularity",
            Sort::Score => "score",
            Sort::Recent => "recent",
            Sort::Title => "title",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub sort: Option<Sort>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub page: u32,
    pub per_page: u32,
}

/// One card: everything a poster grid, a row or the hero needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Media {
    /// The title's key, e.g. `series-tt0903747` or `anime-16498`. See `media_id`.
    pub id: String,
    pub kind: Kind,
    pub title: String,
    /// Other names the title is known by, used to match release names.
    pub title_variants: Vec<String>,
    pub poster: Option<String>,
    pub banner: Option<String>,
    pub description: Option<String>,
    pub format: Option<String>,
    pub year: Option<i32>,
    /// Total episodes, when the provider reports one.
    pub episodes: Option<u32>,
    pub runtime_minutes: Option<u32>,
    pub status: Option<String>,
    /// 0-100, so every provider's rating reads on one scale.
    pub score: Option<u32>,
    pub popularity: i64,
    pub genres: Vec<String>,
    pub is_adult: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Page {
    pub results: Vec<Media>,
    pub has_next: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    /// A full media id, e.g. `series-tt0903747-s1e5`.
    pub id: String,
    pub season: Option<u32>,
    pub number: u32,
    pub name: Option<String>,
    pub airdate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trailer {
    pub site: String,
    pub id: String,
}

/// A title in full, for the detail and watch pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detail {
    pub media: Media,
    pub studios: Vec<String>,
    pub trailer: Option<Trailer>,
    /// Empty for a film, and for an anime whose provider reports only a count.
    pub episodes: Vec<Episode>,
    pub cast: Vec<Person>,
    pub related: Vec<Media>,
    pub recommended: Vec<Media>,
    /// Streaming search keys the stream layer needs. Not rendered.
    pub imdb_id: Option<String>,
    /// TV database id, used to confirm a catalog match when IMDb is missing.
    #[serde(default)]
    pub tvdb_id: Option<String>,
    /// Country of origin, when the provider reports one.
    #[serde(default)]
    pub country: Option<String>,
    /// Original language, when the provider reports one.
    #[serde(default)]
    pub language: Option<String>,
    /// True when another catalog title has the same name and year.
    #[serde(default)]
    pub title_ambiguous: bool,
}

/// One torrent an indexer found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub title: String,
    /// 40 lowercase hex characters. See `magnet::to_hex_info_hash`.
    pub info_hash: String,
    /// Optional zero-based file index within the torrent, preserving an addon's selected file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_idx: Option<usize>,
    pub magnet: String,
    pub size_bytes: Option<u64>,
    /// Upstream-reported count; missing is not an observed dead swarm.
    #[serde(default)]
    pub seeders: Option<u32>,
    /// Which indexer, or which tracker an aggregating indexer got it from.
    pub source: String,
    /// Ordered indexer observations; old cached rows may only carry `source`.
    #[serde(default)]
    pub sources: Vec<String>,
}

impl Release {
    /// A hash identifies a swarm; an optional index keeps an addon's pack file distinct.
    pub fn selection_id(&self) -> String {
        match self.file_idx {
            Some(file_idx) => format!("{}:{file_idx}", self.info_hash),
            None => self.info_hash.clone(),
        }
    }

    /// Parses a legacy hash or a file-scoped `<hash>:<fileIdx>` selection.
    pub fn parse_selection_id(id: &str) -> Option<(String, Option<usize>)> {
        let (hash, file_idx) = match id.split_once(':') {
            Some((hash, index)) if !index.contains(':') => {
                if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                (hash, Some(index.parse().ok()?))
            }
            Some(_) => return None,
            None => (id, None),
        };
        if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        Some((hash.to_ascii_lowercase(), file_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_kinds_keep_their_names_and_shapes() {
        for (kind, name, shape, singular) in [
            (Kind::Anime, "anime", Shape::Anime, "Anime"),
            (Kind::Series, "series", Shape::Series, "Series"),
            (Kind::Movie, "movie", Shape::Movie, "Film"),
        ] {
            assert_eq!(kind.as_str(), name);
            assert_eq!(Kind::parse(name), Some(kind));
            assert_eq!((kind.shape(), kind.singular()), (shape, singular));
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<Kind>(&format!("\"{name}\"")).unwrap(),
                kind
            );
        }
    }

    #[test]
    fn a_kind_saved_by_a_plugin_that_is_gone_still_reads_and_behaves_like_a_series() {
        let kind: Kind = serde_json::from_str("\"goneplugin\"").unwrap();
        assert_eq!((kind.as_str(), kind.shape()), ("goneplugin", Shape::Series));
        // A plugin that comes back declares it, and its shape wins.
        let declared = Kind::register("goneplugin", Shape::Movie, "Gone").unwrap();
        assert_eq!(
            (declared, declared.shape(), declared.singular()),
            (kind, Shape::Movie, "Gone")
        );
        // Declared again, the last word counts (a plugin that is updated), but a built-in kind stays as it is.
        let again = Kind::register("goneplugin", Shape::Anime, "Gone for good").unwrap();
        assert_eq!(
            (again.shape(), again.singular()),
            (Shape::Anime, "Gone for good")
        );
        assert!(Kind::register("anime", Shape::Movie, "x").is_err());
        assert!(Kind::register("anime", Shape::Anime, "Anime").is_ok());
    }

    #[test]
    fn while_only_known_kinds_are_read_a_new_name_is_refused_and_not_kept() {
        let only = OnlyKnownKinds::begin();
        assert!(serde_json::from_str::<Kind>("\"neverseenbefore\"").is_err());
        assert!(serde_json::from_str::<Kind>("\"anime\"").is_ok());
        drop(only);
        assert!(Kind::parse("neverseenbefore").is_none());
    }

    #[test]
    fn a_name_that_is_not_a_kind_name_is_refused() {
        for bad in ["", "Anime", "a-b", "a1", "has space", "waytoolongakindname"] {
            assert!(
                serde_json::from_str::<Kind>(&format!("\"{bad}\"")).is_err(),
                "{bad}"
            );
            assert!(Kind::register(bad, Shape::Series, "x").is_err(), "{bad}");
        }
        assert!(Kind::parse("nosuchkindyet").is_none());
    }
}
