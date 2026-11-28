//! Metadata providers.
//!
//! A provider answers "what titles exist" and "what is this one". Each kind has an ordered chain:
//! the next provider answers when the first returns nothing or fails, so an outage degrades a row
//! instead of emptying it.
//!
//! Which providers there are, where they are and how their answers are read is data
//! (`definition::ProviderDef`); this module is the one engine that follows those definitions.

mod card;
mod detail;
mod disk;
mod fetch;
mod list;
#[cfg(test)]
mod read_tests;

use crate::definition::ProviderDef;
use crate::media_id::{qualified_id, split_qualified};
use crate::model::{Detail, Filters, Kind, Media, Page, Shape};
use crate::sources::{self, Sources};

/// Keeps catalogue answers in `dir` between runs, so a start shows what the last one saw while
/// the new answers come in.
pub fn keep_answers_in(dir: std::path::PathBuf) {
    disk::start(dir);
}

/// The providers that answer for a kind, in the order the definitions give.
fn chain(sources: &Sources, kind: Kind) -> Vec<&ProviderDef> {
    sources
        .chain(kind)
        .iter()
        .filter_map(|id| sources.provider(id))
        .collect()
}

/// Whether every request of a definition can be filled in with just what that request has: a
/// misspelt hole, or one that is empty when it matters (the id of a list, the genre nobody asked
/// for), would otherwise only look like a provider that never answers.
pub fn check(def: &ProviderDef) -> anyhow::Result<()> {
    for &kind in &def.kinds {
        let ensure_fills = |what: &str, asking: &fetch::Asking, request| -> anyhow::Result<()> {
            anyhow::ensure!(
                asking.prepare(request).is_some(),
                "the {what} request cannot be filled in for {}",
                kind.as_str()
            );
            Ok(())
        };
        let mut about = fetch::Asking::new(def, kind);
        about.id = Some("1");
        ensure_fills("detail", &about, &def.detail.request)?;
        for list in [&def.detail.related, &def.detail.recommended]
            .into_iter()
            .flatten()
        {
            if let Some(request) = &list.request {
                ensure_fills("related", &about, request)?;
            }
        }
        if let Some(list) = &def.list {
            let mut page = fetch::Asking::new(def, kind);
            page.per_page = 24;
            ensure_fills("list", &page, &list.request)?;
            page.skip = 48;
            ensure_fills("list", &page, &list.request)?;
        }
        if let Some(search) = &def.search {
            let mut asking = fetch::Asking::new(def, kind);
            asking.term = Some("sample");
            asking.per_page = search.per_page.unwrap_or(0);
            ensure_fills("search", &asking, &search.request)?;
        }
    }
    Ok(())
}

/// Walks a chain: the first non-empty page wins. `Ok` with an empty page means at least one
/// provider answered and had nothing; `Err` means every provider failed, which the caller must
/// tell apart from "nothing matched" (an offline app is not an empty catalogue).
async fn first_page<P, F, Fut>(what: &str, chain: &[P], mut ask: F) -> anyhow::Result<Page>
where
    P: Copy + std::fmt::Display,
    F: FnMut(P) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Page>>,
{
    let mut answered = false;
    let mut last_error = None;
    for &provider in chain {
        match ask(provider).await {
            Ok(page) if !page.results.is_empty() => return Ok(page),
            Ok(_) => answered = true,
            Err(error) => {
                tracing::warn!("[providers] {provider}.{what} failed: {error:#}");
                last_error = Some(error);
            }
        }
    }
    match last_error {
        Some(error) if !answered => Err(error),
        _ => Ok(Page::default()),
    }
}

/// A provider as a link of a chain, named by its id in the log.
#[derive(Clone, Copy)]
struct Link<'a>(&'a ProviderDef);

impl std::fmt::Display for Link<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.id)
    }
}

fn links(sources: &Sources, kind: Kind) -> Vec<Link<'_>> {
    chain(sources, kind).into_iter().map(Link).collect()
}

/// A title of a provider that says where its titles come from gets that in its id.
fn qualify(def: &ProviderDef, media: &mut Media) {
    let own = format!("{}-", media.kind.as_str());
    if let Some(raw) = media.id.strip_prefix(&own) {
        media.id = format!("{own}{}", qualified_id(&def.id, raw));
    }
}

fn mark_page(def: &ProviderDef, mut page: Page) -> Page {
    if def.qualified {
        page.results
            .iter_mut()
            .for_each(|media| qualify(def, media));
    }
    page
}

fn mark_detail(def: &ProviderDef, mut detail: Detail) -> Detail {
    if !def.qualified {
        return detail;
    }
    qualify(def, &mut detail.media);
    for media in detail.related.iter_mut().chain(&mut detail.recommended) {
        qualify(def, media);
    }
    // The provider's own episode ids were written for its own raw ids, so they are made again from the
    // title's id, which says where it comes from.
    let title = detail.media.id.clone();
    for episode in &mut detail.episodes {
        episode.id = match (detail.media.kind.shape(), episode.season) {
            (Shape::Series, Some(season)) => format!("{title}-s{season}e{}", episode.number),
            (Shape::Anime, _) => format!("{title}-ep{}", episode.number),
            _ => continue,
        };
    }
    detail
}

/// The providers a plugin brought that serve this kind.
fn extras(sources: &Sources, kind: Kind) -> Vec<&ProviderDef> {
    sources
        .providers
        .iter()
        .filter(|def| def.qualified && def.kinds.contains(&kind))
        .collect()
}

/// One browse row. `Err` only when every provider in the kind's chain failed.
pub async fn list(kind: Kind, filters: &Filters) -> anyhow::Result<Page> {
    list_row(kind, None, filters).await
}

/// A row of the home page: from the named provider alone, or from the kind's chain.
pub async fn list_row(
    kind: Kind,
    provider: Option<&str>,
    filters: &Filters,
) -> anyhow::Result<Page> {
    let sources = sources::current();
    if let Some(id) = provider {
        let def = sources
            .provider(id)
            .filter(|def| def.kinds.contains(&kind))
            .ok_or_else(|| anyhow::anyhow!("there is no provider {id} for {}", kind.as_str()))?;
        return list::list(def, kind, filters)
            .await
            .map(|page| mark_page(def, page));
    }
    let chain = links(&sources, kind);
    if chain.is_empty() {
        // A kind a plugin brought has no built-in chain: its own catalogues answer.
        return from_extras(&sources, kind, filters).await;
    }
    first_page("list", &chain, |link| list::list(link.0, kind, filters)).await
}

/// How long a plugin's catalogue has to answer before it is left out, so a slow one does not hold up the
/// rest.
const EXTRA_TIME: std::time::Duration = std::time::Duration::from_secs(6);

/// Asks a plugin's catalogue, giving up after `EXTRA_TIME`.
async fn within_time<T>(
    asking: impl std::future::Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    tokio::time::timeout(EXTRA_TIME, asking)
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("it did not answer in time")))
}

/// A page made of what the plugins' catalogues for this kind have, asked together. `Err` only when every
/// one of them failed.
async fn from_extras(sources: &Sources, kind: Kind, filters: &Filters) -> anyhow::Result<Page> {
    let mut page = Page::default();
    let mut answered = false;
    let mut last_error = None;
    let defs = extras(sources, kind);
    let answers = futures::future::join_all(
        defs.iter()
            .map(|def| within_time(list::list(def, kind, filters))),
    )
    .await;
    for (def, answer) in defs.into_iter().zip(answers) {
        match answer {
            Ok(found) => {
                answered = true;
                let found = mark_page(def, found);
                page.has_next |= found.has_next;
                page.results.extend(found.results);
            }
            Err(error) => {
                tracing::warn!("[providers] {}.list failed: {error:#}", def.id);
                last_error = Some(error);
            }
        }
    }
    match last_error {
        Some(error) if !answered => Err(error),
        _ => Ok(page),
    }
}

/// A search within one kind: the chain answers with the fallback rules of `list`, and the providers a
/// plugin brought add what they find, after it.
pub async fn search(kind: Kind, term: &str) -> anyhow::Result<Page> {
    let sources = sources::current();
    let defs = extras(&sources, kind);
    // The chain and the plugins' catalogues are asked together, each plugin with a limit of its own.
    let (chained, answers) = futures::future::join(
        first_page("search", &links(&sources, kind), |link| {
            list::search(link.0, kind, term)
        }),
        futures::future::join_all(
            defs.iter()
                .map(|def| within_time(list::search(def, kind, term))),
        ),
    )
    .await;
    let mut more: Vec<Media> = Vec::new();
    for (def, answer) in defs.into_iter().zip(answers) {
        match answer {
            Ok(found) => more.extend(mark_page(def, found).results),
            Err(error) => tracing::warn!("[providers] {}.search failed: {error:#}", def.id),
        }
    }
    if more.is_empty() {
        return chained;
    }
    // A plugin answered, so the search did, even if the whole chain failed.
    let mut page = chained.unwrap_or_default();
    for media in more {
        if !page.results.iter().any(|shown| shown.id == media.id) {
            page.results.push(media);
        }
    }
    Ok(page)
}

/// A search within one named provider. Used to enrich a title without changing the chain order.
pub async fn search_provider(id: &str, kind: Kind, term: &str) -> anyhow::Result<Page> {
    let sources = sources::current();
    let def = sources
        .provider(id)
        .filter(|def| def.kinds.contains(&kind))
        .ok_or_else(|| anyhow::anyhow!("there is no provider {id} for {}", kind.as_str()))?;
    list::search(def, kind, term)
        .await
        .map(|page| mark_page(def, page))
}

/// A title in full. `id` is a title key such as `series-tt0903747`; an episode id is accepted too
/// and reduced to its title.
pub async fn detail(id: &str) -> Option<Detail> {
    let parsed = crate::media_id::parse(id)?;
    let sources = sources::current();
    if let Some((provider, raw)) = split_qualified(&parsed.provider_id) {
        let def = sources
            .provider(provider)
            .filter(|def| def.qualified && def.kinds.contains(&parsed.kind))?;
        return match detail::detail(def, parsed.kind, &raw).await {
            Ok(Some(found)) if !found.media.id.is_empty() => Some(mark_detail(def, found)),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!("[providers] {}.detail failed: {error:#}", def.id);
                None
            }
        };
    }
    for provider in links(&sources, parsed.kind) {
        match detail::detail(provider.0, parsed.kind, &parsed.provider_id).await {
            Ok(Some(detail)) if !detail.media.id.is_empty() => return Some(detail),
            Ok(_) => {}
            Err(error) => tracing::warn!("[providers] {provider}.detail failed: {error:#}"),
        }
    }
    None
}

/// The id of a title's episode in another service, found through the mapper `mapper_id`: metadata, not
/// a torrent source. `provider_id` is our own (AniList's) id. The mapper's answer must name an episode
/// of the same number, in the shape `<mapper>:<title>:<episode>` (`<mapper>:<title>` for a film),
/// or there is no mapping rather than another title.
pub async fn mapped_episode(
    mapper_id: &str,
    kind: Kind,
    provider_id: &str,
    episode: u32,
) -> Option<crate::indexers::Mapped> {
    if episode == 0
        || provider_id.is_empty()
        || !provider_id.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let sources = sources::current();
    let mapper = sources.mapper(mapper_id)?;
    if !mapper.kinds.contains(&kind) {
        return None;
    }
    let detail = detail::detail(&mapper.provider, kind, provider_id)
        .await
        .ok()??;
    // `provider_id` is ours; the response identifies the distinct title in the other service.
    let title = detail
        .media
        .id
        .strip_prefix(&format!("{}-", kind.as_str()))?;
    let movie = detail.media.format.as_deref() == Some("movie");
    let selected = detail
        .episodes
        .into_iter()
        .find(|entry| entry.number == episode)?
        .id;
    let compatible = if movie {
        episode == 1 && is_actual_compatible_movie(mapper_id, &selected, title)
    } else {
        is_actual_compatible_episode(mapper_id, &selected, title, episode)
    };
    compatible.then(|| crate::indexers::Mapped {
        mapper: mapper_id.into(),
        kind,
        id: selected,
        movie,
    })
}

/// Reject malformed or changed episode coordinates rather than finding another title.
fn is_actual_compatible_episode(mapper: &str, id: &str, title: &str, episode: u32) -> bool {
    let mut pieces = id.split(':');
    pieces.next() == Some(mapper)
        && pieces.next() == Some(title)
        && !title.is_empty()
        && title.bytes().all(|byte| byte.is_ascii_digit())
        && pieces.next() == Some(&episode.to_string())
        && pieces.next().is_none()
}

fn is_actual_compatible_movie(mapper: &str, id: &str, title: &str) -> bool {
    let mut pieces = id.split(':');
    pieces.next() == Some(mapper)
        && pieces.next() == Some(title)
        && !title.is_empty()
        && title.bytes().all(|byte| byte.is_ascii_digit())
        && pieces.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Media;

    fn page(count: usize) -> Page {
        let media = Media {
            id: "movie-1".into(),
            kind: Kind::Movie,
            title: String::new(),
            title_variants: vec![],
            poster: None,
            banner: None,
            description: None,
            format: None,
            year: None,
            episodes: None,
            runtime_minutes: None,
            status: None,
            score: None,
            popularity: 0,
            genres: vec![],
            is_adult: false,
            next_episode: None,
            release_date: None,
        };
        Page {
            results: vec![media; count],
            ..Page::default()
        }
    }

    /// Answers each provider from a script, in chain order.
    async fn walk(script: Vec<anyhow::Result<Page>>) -> anyhow::Result<Page> {
        let names = ["first", "second"];
        let mut script = script.into_iter();
        first_page("test", &names[..script.len()], |_| {
            std::future::ready(script.next().unwrap())
        })
        .await
    }

    #[test]
    fn anime_kitsu_episode_ids_must_match_the_mapped_title_and_episode() {
        for (id, title, episode) in [("kitsu:7442:1", "7442", 1), ("kitsu:1:12", "1", 12)] {
            assert!(is_actual_compatible_episode("kitsu", id, title, episode));
        }
        for id in [
            "",
            "kitsu:130003:1",
            "kitsu:7442:2",
            "kitsu:7442:01",
            "kitsu:7442:0",
            "kitsu:7442:-1",
            "kitsu:7442",
            "kitsu:7442:1:extra",
            "Kitsu:7442:1",
            "anilist:7442:1",
            "kitsu:nope:1",
        ] {
            assert!(
                !is_actual_compatible_episode("kitsu", id, "7442", 1),
                "{id}"
            );
        }
    }

    #[test]
    fn anime_kitsu_movies_require_the_exact_unsuffixed_title_id() {
        assert!(is_actual_compatible_movie("kitsu", "kitsu:50192", "50192"));
        for id in [
            "",
            "kitsu:50192:1",
            "kitsu:50192:1:2",
            "kitsu:50193",
            "kitsu:050192",
            "kitsu:0",
            "Kitsu:50192",
            "anilist:50192",
            "https://localhost",
        ] {
            assert!(!is_actual_compatible_movie("kitsu", id, "50192"), "{id}");
        }
    }

    #[tokio::test]
    async fn the_first_non_empty_page_wins() {
        let found = walk(vec![Err(anyhow::anyhow!("down")), Ok(page(2))]).await;
        assert_eq!(found.unwrap().results.len(), 2);
    }

    #[tokio::test]
    async fn an_empty_answer_is_ok_even_when_another_provider_failed() {
        let found = walk(vec![Err(anyhow::anyhow!("down")), Ok(page(0))]).await;
        assert!(found.unwrap().results.is_empty());
    }

    #[tokio::test]
    async fn every_provider_failing_is_an_error() {
        let found = walk(vec![Err(anyhow::anyhow!("a")), Err(anyhow::anyhow!("b"))]).await;
        assert!(found.is_err());
    }
}
