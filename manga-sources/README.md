# Akidev sources

This folder is a **repository** of source plugins for Akidev, laid out the way any repository must be, and
the place the format is documented. Akidev is a renderer: it has no source of its own, and reads whatever a
definition says. A definition says which requests to make to a site and how to read the answer. Nothing in it
is code. The engine is `desktop/manga` (`src/source.rs`); the reasoning is in `pipeline/MANGA_PLAN.md`.

```
repo.json              {"name": "..."}: the repository's name
index.json             the index (generated: see below), what an app reads first
index.json.sig         optional, a signature over index.json
themes/<id>.json       a base definition for sites that run the same CMS ("theme": true, with a "version")
sources/<id>.json      a source (the file is named after its id)
examples/<id>.json     sources used only to test the engine: not in the index
fixtures/<id>/         recorded pages and fixtures.json for a source or an example
```

## What is in it

Only sources that are legal to read: a public API that welcomes third-party readers, or work released under an open
licence. Nothing that republishes copyrighted chapters without the publisher's leave.

| Source | What it reads | Licence / terms |
| --- | --- | --- |
| `mangadex` | MangaDex's public API: titles, covers, chapters in a chosen language, pages from its image servers | Their API terms (an honest user agent, 5 requests a second at most) |
| `xkcd` | xkcd's JSON endpoints and its archive page | CC BY-NC 2.5 |
| `pepper-and-carrot` | Pepper&Carrot's published episode list and images, in 7 languages | CC BY 4.0 |
| `internet-archive-comics` | The Internet Archive's search and book-reader endpoints, restricted to items carrying a public-domain licence mark | Whatever the item says: the mark is the uploader's claim, not the Archive's |

## Adding this repository to the app

Sources tab, Add a repository, paste the address: `github.com/<owner>/<repo>`, or
`github.com/<owner>/<repo>/tree/<branch>/<folder>` when the repository is in a folder. The app reads
`raw.githubusercontent.com/<owner>/<repo>/<branch>/index.json` (any other https folder with an `index.json`
works too; plain http is accepted only for this machine, for development). Adding installs nothing: the repository
page lists its sources, each with Install or Update. The app checks for newer versions at start.

## Publishing

1. Edit or add `sources/<id>.json` (raise its `version`) and `themes/<id>.json`.
2. `cargo test -p akidev-manga` runs every definition against its fixtures (and fails when `index.json` is stale).
3. `cargo run -p akidev-manga --bin akidev-manga-cli -- index manga-sources` rewrites `index.json` with every file's
   version and SHA-256. Commit it with the rest. An app refuses a file whose hash is not the index's, so a stale
   index breaks installs, which is why a test guards it.
4. Optional signing: `akidev-manga-cli keygen secret.key` (keep it out of the repository), then
   `akidev-manga-cli index manga-sources --key secret.key` writes `index.json` with the public key and
   `index.json.sig`. The app remembers the key the first time a signed repository is added and refuses the
   repository later if it names a different one (the person removes and re-adds it to trust a new key).

### The index

```json
{ "schema": 1, "name": "Akidev sources", "key": "<public key hex, only if signed>",
  "sources": [ { "id": "mangadex", "name": "MangaDex", "version": 1, "path": "sources/mangadex.json",
                 "sha256": "...", "lang": ["en"], "nsfw": false, "min_app": "0.1.25", "requires": ["madara"] } ],
  "themes":  [ { "id": "madara", "version": 1, "path": "themes/madara.json", "sha256": "..." } ] }
```

Paths stay inside the repository (no `..`, no addresses); ids are lowercase letters, digits and dashes; every
`requires` names a listed theme (installed with the source); `min_app` is the oldest app that can run it.

## Checking a definition

```
cargo run -p akidev-manga --bin akidev-manga-cli -- check manga-sources/sources/mangadex.json manga-sources/fixtures/mangadex --themes manga-sources/themes
cargo run -p akidev-manga --bin akidev-manga-cli -- check ... --bless     # write what the definition returns as the expected answers
cargo run -p akidev-manga --bin akidev-manga-cli -- run manga-sources/sources/mangadex.json popular   # live
```

`record` asks the live site for the calls you name and saves its answers as fixtures in one go:

```
cargo run -p akidev-manga --bin akidev-manga-cli -- record manga-sources/sources/xkcd.json manga-sources/fixtures/xkcd \
  popular details=xkcd chapters=xkcd pages=xkcd,614        # also: latest, search=TERM, pages=SERIES,CHAPTER
```

`--bless` writes whatever the definition returns: read the result before committing it. A definition is also
validated when it is loaded (selectors and patterns compile, every request starts with `{base}`, date formats
have a year, settings have valid defaults).

**Fixtures recorded from the live sites (2026-10-21, with `record`):** `xkcd` (its archive page trimmed to the newest 60
comics), `pepper-and-carrot` and `internet-archive-comics`. **Fixtures written by hand:** `madara-example` (to look like
Madara's markup) and `mangadex` (to look like MangaDex's API answers; the site was not reachable when they were written).
Hand-written fixtures prove the engine reads those shapes, not that a real site matches them.

## Format (schema 1)

A source has `popular`, `latest` and `search` (at least one), then `detail`, `chapters` and `pages`.
Each has a `request` and a `format` (`html`, the default, or `json`).

A request's `url` must start with `{base}`: a source can only ask its own address (and its `mirrors`,
tried in turn when one cannot be reached). Holes: `{page}`, `{page0}`, `{term}`, `{id}`, `{series}` and a
filter's id, each percent-encoded; `{id:raw}` is not (for an id that is a path). `{? ... ?}` vanishes when a
hole inside is empty. A request can be `POST` with `form` or `json`, and has `headers`.

A value is read by an expression. Short forms: HTML `"h3"` (the text of the match), `"img@data-src|src"`
(an attribute, the first that is not empty), `"@href"` (an attribute of the row); JSON `"a.b.c"` (a path,
`*` for every value). For anything else, a list of steps:

| step | does |
| --- | --- |
| `{"css": sel}` | elements below each node (also in a text that is HTML, for an ajax answer) |
| `{"attr": "a\|b"}`, `"text"`, `"html"` | an attribute, the text (blocks on their own lines), inner HTML |
| `{"path": "a.*.b"}`, `"json"` | into JSON; text parsed as JSON (a `<script>` blob) |
| `{"regex": p}`, `{"replace": {"from": p, "to": t}}` | first capture group (a node that does not match is dropped); replace all |
| `{"template": "{base}/x/{value}"}` | text with the node and the request holes |
| `"abs"` | a link made absolute against the source |
| `{"date": ["MMMM d, yyyy", "iso", "relative"]}` | milliseconds; formats tried in order; `relative` reads "3 days ago" |
| `"number"` | the chapter number in a text ("Ch. 12.5 - Name") |
| `"first"`, `"last"`, `{"nth": n}`, `{"join": ", "}`, `{"split": ","}` | pick, join or split |
| `{"const": t}`, `{"default": t}`, `"non_empty"`, `"exists"`, `{"map": [["ongoing","ongoing"]]}` | constants and tests |
| `{"coalesce": [ex, ex]}` | the first expression that gives something |
| `{"where": {"path": p, "equals"/"not_equals"/"in"/"absent": ...}}` | keeps the JSON nodes whose value at `path` passes (a related item of one type, a row that is not external) |
| `"unique"`, `"sort"` | drops texts already seen; orders texts the way file names read (`p2` before `p10`) |
| `"plain_text"` | HTML inside a text (a JSON field that holds markup) as plain text |

`"where"` values may hold holes (`"equals": "{id}"` picks the row for the id asked about). `{base}` is the source's address.
`{value}` in a template is the node being read; `{^.path}` is a value in the row the expression started from
(`"{^.id}"` in a cover address); `{name}` is a hole of the request, a setting or a var.

### Chains, vars, repeats, settings

- `vars`, on any section: `{"server": "baseUrl"}` reads values out of that section's answer and makes them holes in
  its expressions (the image server an answer names).
- `prepare`, on any section: requests made first, each with its own `vars`, which later requests use as holes (a page
  that holds a nonce, a lookup that says which server to ask).
- `chapters.repeat`: `{"size": 500, "total": "total"}` asks again with `{offset}` raised by `size` until `total` is
  reached (a source that pages its chapter list).
- Lists: `page_size` (top level) and `{offset}` (`(page - 1) * page_size`); `total` says how many titles there are, so
  the app knows whether another page follows.
- `settings`: `{"id": "lang", "name": ..., "default": "en", "options": [{"value", "name"}]}`; the person's choice (the
  default until then) fills the hole `{lang}`. `{rating:raw}` is not percent-encoded, for an option that is a piece of
  query. The app shows each setting as a picker on the source's page.
- `user_agent` (top level) is sent instead of the browser one, for a source that asks clients to say who they are.
- `image_headers`: a header with an empty value is left out (`"referer": ""`).

Chapters come back newest first whatever order the site lists them in (`"order": "oldest-first"` if it
lists them oldest first). A chapter's number is read from its title when no `number` is given.
`pages.reading` is `longstrip`, `paged` or `paged_rtl`.

## Not here yet

Filters are parsed but only passed through to a request's holes (no filter screen yet); `aes`, `xor` and tile
descrambling steps; a way past a check (there is no web view: a site that wants a check returns a
`fetch::Challenge` error and the app says it is not supported from here); scripting for sites a definition
cannot express (decided against for v1, see `pipeline/MANGA_PLAN.md`).
