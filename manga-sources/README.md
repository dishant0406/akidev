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

Three sources, each with fixtures recorded from the live site, and one theme:

| Source | Site | How it reads the site |
| --- | --- | --- |
| `asurascans` | api.asurascans.com | its own JSON API: `data`, `meta.has_more`, `series`, `data.chapter.pages.*.url` |
| `dynastyscans` | dynasty-scans.com | its own JSON API, for series and for one-shots; an id is `series/<p>` or `doujins/<p>` |
| `flamecomics` | flamecomics.xyz | a Next.js site: reads `script#__NEXT_DATA__` as JSON for detail, chapters and pages; search is its `/api/series` |

No shipped source extends `madara` any more. The theme stays: `examples/madara-example.json` and
`examples/madara-protected.json` are what keep it honest, and the family is still the cheapest place to add sources from.
Earlier ports of `manhuaplus`, `3asq` and `tcbscans` live in `git show 75a1a53`, and the findings they produced — the
mirror trap, the per-site `pages` override, the chapter list that must ride in the detail page — are why those sites are
cheap whenever they are wanted again.

These are the only sources here for the only reason that matters: a source is worth shipping
when the site is **reachable, gives its chapters in the answer, and gives its pages in the answer**. Most sites in the
ecosystem fail one of the three. The probe behind that, and why each family fails, is in
`pipeline/MANGA_SOURCES_PORT_PLAN.md` §3b.

A site that answers a `POST` (the chapter list) through a redirect loses the body, which silently returns the wrong page:
put the host that answers without redirecting in `base_url` and the other in `mirrors`. The recorded case was
`3asq.org`, which answers `301` to `3asq.online`: `.online` had to be `base_url` and `.org` the mirror, even though the
extension that led there named `.org`. Probe a mirror pair before writing either address, because the failure is silent.

Two examples under `examples/` test the engine (`madara-example` for Madara's markup, `madara-protected` for an encrypted
chapter payload). A source is added by writing `sources/<id>.json` (see Publishing) or pasted into the app on its own
(Sources tab, Add a source). Only sources that are legal to read belong here: a public API that welcomes third-party
readers, or work released under an open licence. Nothing that republishes copyrighted chapters without the publisher's
leave.

## Adding this repository to the app

Sources tab, Add a repository, paste the address: `github.com/<owner>/<repo>`, or
`github.com/<owner>/<repo>/tree/<branch>/<folder>` when the repository is in a folder. The app reads
`raw.githubusercontent.com/<owner>/<repo>/<branch>/index.json` (any other https folder with an `index.json`
works too; plain http is accepted only for this machine, for development). Adding installs nothing: the repository
page lists its sources, each with Install or Update. The app checks for newer versions at start.

## Adding one source without a repository

Sources tab, Add a source, paste the definition. It is checked before it is kept, and the app shows its name,
languages and version first. A definition that extends a theme has the theme merged into it at that moment and is
stored that way, so it keeps working on its own — the theme it borrowed does not have to stay installed, and a
source from a repository still follows its theme's updates. A definition pasted this way does not update itself:
raise its `version` and paste it again, or put it in a repository.

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
  "sources": [ { "id": "example", "name": "Example", "version": 1, "path": "sources/example.json",
                 "sha256": "...", "lang": ["en"], "nsfw": false, "min_app": "0.1.25", "requires": ["madara"] } ],
  "themes":  [ { "id": "madara", "version": 1, "path": "themes/madara.json", "sha256": "..." } ] }
```

Paths stay inside the repository (no `..`, no addresses); ids are lowercase letters, digits and dashes; every
`requires` names a listed theme (installed with the source); `min_app` is the oldest app that can run it.

## Checking a definition

```
cargo run -p akidev-manga --bin akidev-manga-cli -- check manga-sources/examples/madara-example.json manga-sources/fixtures/madara-example --themes manga-sources/themes
cargo run -p akidev-manga --bin akidev-manga-cli -- check ... --bless     # write what the definition returns as the expected answers
cargo run -p akidev-manga --bin akidev-manga-cli -- run manga-sources/sources/<id>.json popular   # live
```

`record` asks the live site for the calls you name and saves its answers as fixtures in one go:

```
cargo run -p akidev-manga --bin akidev-manga-cli -- record manga-sources/sources/<id>.json manga-sources/fixtures/<id> \
  popular details=<series> chapters=<series> pages=<series>,<chapter>        # also: latest, search=TERM
```

`--bless` writes whatever the definition returns: read the result before committing it. A definition is also
validated when it is loaded (selectors and patterns compile, every request starts with `{base}`, date formats
have a year, settings have valid defaults).

**Fixtures written by hand:** `madara-example` (to look like Madara's markup) and `madara-protected` (an encrypted chapter
payload). Hand-written fixtures prove the engine reads those shapes, not that a real site matches them; a source added later
should have fixtures recorded from the live site with `record`.

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
| `{"aes_cbc": {"passphrase": "{nonce}"}}` | decrypts a payload a site hands over encrypted (below) |

`"where"` values may hold holes (`"equals": "{id}"` picks the row for the id asked about). `{base}` is the source's address.
`{value}` in a template is the node being read; `{^.path}` is a value in the row the expression started from
(`"{^.id}"` in a cover address); `{name}` is a hole of the request, a setting or a var.

### Chains, vars, repeats, settings

- `vars`, on any section: `{"server": "baseUrl"}` reads values out of that section's answer and makes them holes in
  its expressions (the image server an answer names).
- `prepare`, on any section: requests made first, each with its own `vars`, which later requests use as holes (a page
  that holds a nonce, a lookup that says which server to ask).
- `chapters.repeat`: `{"size": 500, "total": "total"}` asks again with `{offset}` raised by `size` until `total` is
  reached (a source that pages its chapter list). `{page}` is also set, raised by one each time, for a site that
  numbers its pages (`?t={page}`) instead of counting rows — so put `{page}` in the request's url, or every request
  is the same one. `total` counts **chapters**; a site that only says how many **pages** it has (`"total_pages": 6`)
  must leave `total` out. Asking then stops when a page brings no chapter not already seen. Screenshots of the same
  list can repeat across pages, so that is the safer stop either way. One list may take at most **60** requests, so a
  source with more chapters than `60 * size` in all asks for a bigger `size`; running out is reported, never returned
  as a short list that looks complete.
- Lists: `page_size` (top level) and `{offset}` (`(page - 1) * page_size`); `total` says how many titles there are, so
  the app knows whether another page follows. `next` is an expression over the answer, and a page follows when it gives
  something (`"next": "a.nextpostslink"` reads the site's own next link). Leave both out and a page follows whenever
  the answer had **rows** — cards dropped for having no id do not end the list. Do not build `next` out of the
  request's own holes (`{"template": "{page}"}`): those are always set, so it would say every page follows for ever.
  `total` is compared with `page * page_size`, so it has to count what a page holds; a site handing back 25 rows a page
  cannot be described by `total` unless `page_size` is 25 too.
- `settings`: `{"id": "lang", "name": ..., "default": "en", "options": [{"value", "name"}]}`; the person's choice (the
  default until then) fills the hole `{lang}`. `{rating:raw}` is not percent-encoded, for an option that is a piece of
  query. The app shows each setting as a picker on the source's page.
- `user_agent` (top level) is sent instead of the browser one, for a source that asks clients to say who they are.
- `image_headers`: a header with an empty value is left out (`"referer": ""`).

Chapters come back newest first whatever order the site lists them in (`"order": "oldest-first"` if it
lists them oldest first). A chapter's number is read from its title when no `number` is given.
`pages.reading` is `longstrip`, `paged` or `paged_rtl`.

### A Next.js (or similar) site that ships its data as JSON in the page

Some sites render no list markup at all: the catalogue lives in a `<script>` blob the page's own JavaScript
reads. `flamecomics` (Next.js, `script#__NEXT_DATA__`) is the working pattern. The blob is HTML, so the section
keeps `format: html` and the expression is `[{ "css": "script#__NEXT_DATA__" }, "html", "json", { "path": … }]`.
Three things that bite, each proven against the live site:

- Once the expression has gone through `"json"`, the nodes are JSON, but the **section's format is still
  `html`**, so a short-form field (`"token"`) reads as CSS, not a path, and silently gives nothing. Write the
  path explicitly: `[{ "path": "token" }]` (a bare `{ "path": "token" }` is not a valid expression — a single
  step still needs its list).
- A JSON **object** iterated with `*` (a chapter's `images`, keyed `"0".."22"`) comes back in the object's key
  order, which is lexicographic: `…-09, -10, … -19, -02`. Add a `"sort"` step before the `template` that builds
  the address, or the pages come out of order.
- A `select` filter, the `next`/`total` rule and the rest behave exactly as on a plain HTML site — the JSON
  detour changes only how a value is reached, not how lists page or filters fill holes.

### Encrypted payloads

Some sites do not send a chapter's images at all: they send a blob their own JavaScript decrypts. `aes_cbc` undoes
it. Its input is the payload — an object, or the JSON text of one — and the payload usually carries the salt it was
made with, so a definition names only the passphrase it read from the same page:

```json
"images": [
  { "css": "script#chapter-protector-data" }, "html",
  { "replace": { "from": "\\\\/", "to": "/" } },
  { "regex": "(?s)var\\s+chapter_data\\s*=\\s*'(\\{.*?\\})';" },
  { "aes_cbc": { "passphrase": "{nonce}" } },
  "json", { "path": "*" }
],
"vars": { "nonce": [{ "css": "script#chapter-protector-data" }, "html", { "replace": { "from": "\\\\", "to": "" } }, { "regex": "var\\s+wpmangaprotectornonce\\s*=\\s*'([^']*)';" }] }
```

(The Madara family's "chapter protector", worked through in `examples/madara-protected.json`.) The key is
`MD5(previous ++ passphrase ++ salt)` repeated to 48 bytes — 32 for the key, 16 for the IV — which is what the
site's script does; a payload that carries its own `iv` uses that instead. A site with one fixed key states it
rather than deriving one: `{"aes_cbc": {"key": "…64 hex…", "iv": "…32 hex…"}}` (32-byte key for AES-256, 16 for
AES-128). `"hex": true` when the ciphertext itself is hex rather than base64. The salt must be hexadecimal.

Decryption is the only thing in the engine that touches cryptography: no definition can ask for an encryption, a
signature or a hash of its own, so this cannot be turned into an oracle for something else.

## Not here yet

Filters are parsed but only passed through to a request's holes (no filter screen yet); `xor` and tile descrambling
steps; a way past a check (there is no web view: a site that wants a check returns a `fetch::Challenge` error and
the app says it is not supported from here); scripting for sites a definition cannot express (decided against for
v1, see `pipeline/MANGA_PLAN.md`).

Tile descrambling is the one worth explaining, because it looks easy and is not: the image arrives with a
permutation saying which `tile_cols × tile_rows` cell belongs where, so it has to be undone on the image bytes,
which means a codec in the engine and a call the app makes instead of showing the address. The only source that has
it (`AsuraScans`) stopped sending the permutation — verified against its live API on 2026-10-22, every page comes
back as `{url, width, height}` — so there is nothing to descramble and nothing to test against. It gets built when
a source actually sends one; the algorithm is a paragraph, not a research project (see
`pipeline/MANGA_SOURCES_PORT_PLAN.md` §Phase 4).
