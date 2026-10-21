# Manga source definitions

A definition says which requests to make to a site and how to read the answer. Nothing in it is code.
The engine is `akidev-manga` (`src/source.rs`); the reasoning is in `pipeline/MANGA_PLAN.md`.

- `themes/<name>.json`: a base definition for sites that run the same CMS (`"theme": true`).
- `<id>.json`: a site. `{"extends": "madara", "id": ..., "base_url": ...}` is the theme with those fields
  replaced (objects merge key by key; lists and texts are replaced).
- `fixtures/<id>/`: recorded pages and `fixtures.json`. `cargo test -p akidev-manga` reads every definition
  here against its fixtures, so a change to a theme cannot quietly break a site that extends it.

**The fixtures in `fixtures/madara-example/` are hand-written to look like Madara's markup. They are not
recorded from a live site** (no site was contacted when this was written). They prove the engine reads that
shape; they do not prove any real Madara site matches it. Replace them with recorded pages when there is a
site to record.

## Checking a definition

```
cargo run -p akidev-manga --bin akidev-manga-cli -- check sources/madara-example.json sources/fixtures/madara-example --themes sources/themes
cargo run -p akidev-manga --bin akidev-manga-cli -- check ... --bless     # write what the definition returns as the expected answers
cargo run -p akidev-manga --bin akidev-manga-cli -- run sources/madara-example.json popular --themes sources/themes   # live
```

`--bless` writes whatever the definition returns: read the result before committing it. A definition is
also validated when it is loaded (selectors and patterns compile, every request starts with `{base}`, date
formats have a year).

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

Chapters come back newest first whatever order the site lists them in (`"order": "oldest-first"` if it
lists them oldest first). A chapter's number is read from its title when no `number` is given.
`pages.reading` is `longstrip`, `paged` or `paged_rtl`.

## Not here yet

Filters are parsed but only passed through to a request's holes; the filter UI, ajax chapter lists that
need a nonce from the page, `aes`, `xor` and tile descrambling steps, the `challenge` web view hook (a site
that wants a check returns a `fetch::Challenge` error the app can act on), and signed source lists.
