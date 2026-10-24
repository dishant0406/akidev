//! Book catalogues: an OPDS feed is an Atom page listing books to get and shelves to open, which any program can read. This
//! reads one and fetches a book from it; it has no catalogue of its own.

use super::epub::parse;
use anyhow::Context;
use scraper::{ElementRef, Selector};
use url::Url;

/// A page of a catalogue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Feed {
    pub title: String,
    pub items: Vec<Item>,
    /// The next page of the same list.
    pub next: Option<String>,
    /// How to search: an address with `{searchTerms}` in it, or the address of a description that has one.
    pub search: Option<String>,
}

/// A book to get, or a shelf to open.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub cover: Option<String>,
    /// Another page of the catalogue this opens, for a shelf.
    pub open: Option<String>,
    /// The EPUB file to get, for a book.
    pub get: Option<String>,
}

const MAX_FEED_BYTES: usize = 8 * 1024 * 1024;
const MAX_BOOK_BYTES: usize = 150 * 1024 * 1024;

/// The address as a secure one: a catalogue that still lists `http` addresses is read over `https`, which the phone requires anyway.
/// A server on the same machine or network (a catalogue being tried) is left alone.
fn secure(mut address: Url) -> Url {
    let local = address.host_str().is_none_or(|host| {
        host == "localhost" || host.ends_with(".local") || host.parse::<std::net::IpAddr>().is_ok()
    });
    if address.scheme() == "http" && !local {
        let _ = address.set_scheme("https");
    }
    address
}

fn selector(text: &str) -> Selector {
    Selector::parse(text).expect("a fixed selector")
}

fn text_of(element: &ElementRef, name: &str) -> Option<String> {
    let found = element.select(&selector(name)).next()?;
    let text = found.text().collect::<String>();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(text).filter(|text| !text.is_empty())
}

/// Reads a feed fetched from `at`; every address in it is made whole against that.
pub fn read(xml: &[u8], at: &Url) -> Feed {
    let document = parse(xml);
    let whole = |href: &str| at.join(href.trim()).ok().map(String::from);
    let mut feed = Feed::default();
    // The feed's own links come before its first entry.
    let first_entry = document
        .select(&selector("entry"))
        .next()
        .map(|entry| entry.id());
    for link in document.select(&selector("link")) {
        if first_entry.is_some_and(|entry| link.ancestors().any(|ancestor| ancestor.id() == entry))
        {
            continue;
        }
        let (rel, kind) = (
            link.value().attr("rel").unwrap_or(""),
            link.value().attr("type").unwrap_or(""),
        );
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        match rel {
            "next" => feed.next = whole(href),
            "search" if kind.contains("atom") || href.contains("{searchTerms}") => {
                // Joining percent-encodes the braces of a template: they are put back.
                feed.search = at.join(href).ok().map(|whole| {
                    whole
                        .to_string()
                        .replace("%7BsearchTerms%7D", "{searchTerms}")
                });
            }
            "search" if kind.contains("opensearch") && feed.search.is_none() => {
                feed.search = whole(href)
            }
            _ => {}
        }
    }
    feed.title = document
        .select(&selector("feed > title"))
        .next()
        .map(|title| title.text().collect::<String>().trim().to_string())
        .or_else(|| text_of(&document.root_element(), "title"))
        .unwrap_or_default();
    for entry in document.select(&selector("entry")) {
        let mut item = Item {
            title: text_of(&entry, "title").unwrap_or_default(),
            author: text_of(&entry, "author name"),
            summary: text_of(&entry, "summary").or_else(|| text_of(&entry, "content")),
            ..Item::default()
        };
        let mut thumbnail = None;
        for link in entry.select(&selector("link")) {
            let (rel, kind) = (
                link.value().attr("rel").unwrap_or(""),
                link.value().attr("type").unwrap_or(""),
            );
            let Some(href) = link.value().attr("href").and_then(whole) else {
                continue;
            };
            if rel.contains("opds-spec.org/acquisition") {
                if kind.contains("epub") && item.get.is_none() {
                    item.get = Some(href);
                }
            } else if rel == "http://opds-spec.org/image" {
                item.cover = Some(href);
            } else if rel == "http://opds-spec.org/image/thumbnail" {
                thumbnail = Some(href);
            } else if kind.contains("atom+xml")
                && matches!(rel, "" | "subsection" | "alternate" | "related")
                || rel.contains("opds-spec.org/sort")
                || rel.contains("opds-spec.org/featured")
            {
                // An entry's "alternate" link to a page of details is not a shelf: only a feed is.
                if kind.contains("atom") && item.open.is_none() {
                    item.open = Some(href);
                }
            }
        }
        item.cover = item.cover.or(thumbnail);
        if item.get.is_some() {
            item.open = None;
        }
        if !item.title.is_empty() && (item.get.is_some() || item.open.is_some()) {
            feed.items.push(item);
        }
    }
    feed
}

/// Fetches and reads one page of a catalogue.
pub async fn fetch(address: &str) -> anyhow::Result<Feed> {
    let at = Url::parse(address).context("that is not a web address")?;
    anyhow::ensure!(
        matches!(at.scheme(), "http" | "https"),
        "a catalogue is a web address"
    );
    let response = crate::http::client()
        .get(at.clone())
        .header("Accept", "application/atom+xml, application/xml, text/xml")
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "the catalogue answered {}",
        response.status()
    );
    let at = response.url().clone();
    let body = crate::http::read_capped(response, MAX_FEED_BYTES).await?;
    let feed = read(&body, &at);
    anyhow::ensure!(
        !feed.items.is_empty() || feed.search.is_some(),
        "that address is not a book catalogue"
    );
    Ok(feed)
}

/// The address to ask for a search of `term`, from what a feed gave as `search`: the template, or a description to read first.
pub async fn search_address(search: &str, term: &str) -> anyhow::Result<String> {
    let encoded: String = url::form_urlencoded::byte_serialize(term.as_bytes()).collect();
    if search.contains("{searchTerms}") {
        return Ok(search.replace("{searchTerms}", &encoded));
    }
    let at = secure(Url::parse(search).context("that is not a web address")?);
    let response = crate::http::client().get(at.clone()).send().await?;
    let body = crate::http::read_capped(response, MAX_FEED_BYTES).await?;
    let description = parse(&body);
    let template = description
        .select(&selector("url"))
        .filter(|url| {
            url.value()
                .attr("type")
                .is_some_and(|kind| kind.contains("atom"))
        })
        .find_map(|url| url.value().attr("template"))
        .context("the catalogue does not say how to search it")?;
    let template = template.replace("{searchTerms}", &encoded);
    // Optional parts of a template ({startPage?}) are left out.
    let mut address = template;
    while let (Some(open), Some(close)) = (address.find('{'), address.find('}')) {
        if close < open {
            break;
        }
        address.replace_range(open..=close, "");
    }
    Ok(at.join(&address)?.to_string())
}

/// Downloads an EPUB into `folder` (which the caller makes, and removes), named after its address, and returns the path.
pub async fn download(
    address: &str,
    folder: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let at = Url::parse(address).context("that is not a web address")?;
    anyhow::ensure!(
        matches!(at.scheme(), "http" | "https"),
        "a book is fetched from a web address"
    );
    let response = crate::http::client()
        .get(at.clone())
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "the catalogue answered {}",
        response.status()
    );
    let body = crate::http::read_capped(response, MAX_BOOK_BYTES).await?;
    std::fs::create_dir_all(folder)?;
    let last = at
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .unwrap_or("");
    let name = if last.to_lowercase().ends_with(".epub") {
        last.to_string()
    } else {
        "book.epub".to_string()
    };
    let path = folder.join(name);
    std::fs::write(&path, body)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>Example Library</title>
  <link rel="search" type="application/opensearchdescription+xml" href="/search.xml"/>
  <link rel="next" type="application/atom+xml;profile=opds-catalog" href="/books?page=2"/>
  <entry>
    <title>Popular</title>
    <link rel="subsection" type="application/atom+xml;profile=opds-catalog;kind=acquisition" href="/popular"/>
  </entry>
  <entry>
    <title>A &amp; B</title>
    <author><name>Jane Doe</name></author>
    <summary>Short   text.</summary>
    <link rel="http://opds-spec.org/image" type="image/jpeg" href="/covers/1.jpg"/>
    <link rel="http://opds-spec.org/image/thumbnail" type="image/jpeg" href="/covers/1t.jpg"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="text/html" href="/read/1"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="/files/1.epub"/>
    <link rel="alternate" type="text/html" href="/page/1"/>
  </entry>
  <entry>
    <title>Not a book</title>
    <link rel="alternate" type="text/html" href="/page/2"/>
  </entry>
</feed>"#;

    #[test]
    fn a_feed_gives_shelves_books_and_whole_addresses() {
        let feed = read(
            ROOT.as_bytes(),
            &Url::parse("https://lib.example/catalog/").unwrap(),
        );
        assert_eq!(feed.title, "Example Library");
        assert_eq!(
            feed.next.as_deref(),
            Some("https://lib.example/books?page=2")
        );
        assert_eq!(
            feed.search.as_deref(),
            Some("https://lib.example/search.xml")
        );
        assert_eq!(feed.items.len(), 2);
        assert_eq!(
            feed.items[0].open.as_deref(),
            Some("https://lib.example/popular")
        );
        let book = &feed.items[1];
        assert_eq!(
            (
                book.title.as_str(),
                book.author.as_deref(),
                book.summary.as_deref()
            ),
            ("A & B", Some("Jane Doe"), Some("Short text."))
        );
        assert_eq!(
            book.get.as_deref(),
            Some("https://lib.example/files/1.epub")
        );
        assert_eq!(
            book.cover.as_deref(),
            Some("https://lib.example/covers/1.jpg")
        );
        assert_eq!(book.open, None);
    }

    #[test]
    fn a_search_template_is_filled_and_a_description_is_pointed_at() {
        let feed = read(
            br#"<feed><title>T</title><link rel="search" type="application/atom+xml" href="/s?q={searchTerms}"/></feed>"#,
            &Url::parse("https://lib.example/").unwrap(),
        );
        assert_eq!(
            feed.search.as_deref(),
            Some("https://lib.example/s?q={searchTerms}")
        );
        let address = block_on(search_address(
            feed.search.as_deref().unwrap(),
            "war & peace",
        ));
        assert_eq!(address.unwrap(), "https://lib.example/s?q=war+%26+peace");
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }
}
