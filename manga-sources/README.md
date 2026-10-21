# Aki Manga sources

This folder is a **repository** of source plugins for Aki Manga, laid out the way any repository must be, and
the place the format is documented. Aki Manga is a renderer: it has no source of its own, and reads whatever a
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

`--bless` writes whatever the definition returns: read the result before committing it. A definition is also
validated when it is loaded (selectors and patterns compile, every request starts with `{base}`, date formats
have a year, settings have valid defaults).

**The fixtures for `madara-example` are hand-written to look like Madara's markup, and the ones for `mangadex`
are hand-written to look like MangaDex's API answers. Neither was recorded from the live site** (none was
reachable when they were written). They prove the engine reads those shapes; they do not prove a real site
matches them. Replace them with recorded answers when there is a network that reaches the site.

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
| `"unique"` | drops texts already seen |

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
