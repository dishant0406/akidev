# Source definitions

`sources.json` says where Akidev looks for titles and for releases: the indexers, the
metadata providers, their addresses, and the public trackers. The app has one copy built in
and fetches a newer one in the background, so an indexer can be added, removed or moved
without a new build. Nothing in it is code: a definition only says which requests to make and
which fields to read.

## Publishing a change

1. Edit `sources.json` and raise `version` (every publish must be higher than the live one).
2. `mkdir -p .secrets` once. `cargo test -p akidev-core` checks the built-in copy parses and that each indexer reads a
   sample response correctly. `cargo run -p akidev-core --bin akidev-sources -- check sources/sources.json`
   validates a copy on its own.
3. `./scripts/publish-sources.sh` signs it and uploads it. Running apps pick it up within six
   hours, and anyone who opens the app after that gets it at start.

   It publishes **two feeds with one version number**: the document as it is, at the root of
   `sources.akidev.live` (schema 1, which every app reads), and the same document with `"schema": 2` under `/v2`
   (which only builds that know schema 2 read, and prefer: they fall back to the root feed when `/v2` has
   nothing, or what it has is not genuine or not readable). An app that knows only schema 1 refuses a schema 2
   document, so a value only schema 2 apps can read (a new kind of title in the document itself, a new hole name) goes
   in `sources/sources-v2.json`, which is published as `/v2` when it exists. Its `version` must equal
   `sources.json`'s. Until then the two are the same document. The daily tracker job refreshes the trackers and the version of both, and keeps what the live `/v2` document holds that the root one does not.

The app uses a downloaded copy only if its signature is valid, its `version` is higher than
the one in use and it passes the same validation as the built-in one (limits, addresses, regular
expressions, requests that can be filled in, chains that name providers serving the kind).
Otherwise it keeps what it has. Validation cannot know whether a mirror is alive or a GraphQL
selection is right, so test a change against the live services before publishing it; a mistake
that gets through is undone by publishing a corrected copy with a higher version.

### Trackers

`.github/workflows/refresh-trackers.yml` runs daily. It downloads the document that is live at
`sources.akidev.live`, runs `scripts/refresh-trackers.sh` on it (the first 32 udp/http(s) trackers of
[ngosang/trackerslist](https://github.com/ngosang/trackerslist), `trackers_all.txt`, best first, into
`trackers`, and `version` raised), then signs and publishes it. It never touches the repository, so the
live document is the one that counts and `sources/sources.json` is the copy built into the app. A hand
edit of `trackers` is overwritten by the next run. It needs the repository secrets `SOURCES_SIGNING_KEY`
(the contents of `.secrets/sources.key`), `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID` and `R2_SECRET_ACCESS_KEY`.

### One-time setup

- Create an R2 bucket and give it the custom domain `sources.akidev.live` (done with the cf CLI:
  `cf r2 buckets create-by-name akidev-sources`, then `cf r2 buckets domains custom create
  akidev-sources --domain sources.akidev.live --zone-id <zone> --min-tls 1.2 --enabled`).
- `cargo run -p akidev-core --bin akidev-sources -- keygen .secrets/sources.key` makes the
  signing key and prints its public half. The public half is `PUBLIC_KEY` in
  `core/src/sources.rs`. The private file is git-ignored: keep a copy in a password manager.
  Losing it means shipping a build with a new public key.

## Format

`schema` is the version of this format (an app ignores a document with a higher one),
`version` is the version of the content. `trackers` are added to every magnet. `subtitles` are the subtitle sources. `chains` list
the providers that answer for each kind, in order: the next answers when the first has
nothing or fails.

### Forcing an update

Two optional fields at the top of the document lock out apps that are too old:

```json
{ "min_version": "0.1.17", "min_version_message": "This version has a bug that breaks parties. Please update." }
```

An app whose version is lower than `min_version` shows an "Update required" screen (with the message, an
Update button and the download page) and nothing else, from the moment it reads the document: from its
saved copy at start, and again as soon as the background check picks a newer one up. Apps that are not
older, and apps from before this field existed (they ignore it), run as normal. A `min_version` that is
not `major.minor.patch` makes the whole document invalid, so a typo can never lock anyone out (the
previous document stays in use); and a document that cannot be fetched or verified says nothing.
Publish the release the version points to first: the Update button finds it from the release feed, and
with nothing there it can only open the download page. To lift
it again, publish a document without the field (or with a lower one) and a higher `version`.

### Indexers

```json
{ "id": "knaben", "name": "Knaben", "kinds": ["anime", "series", "movie"],
  "fanout": "per-term", "hosts": ["https://api.knaben.org/v1"], "cooldown_secs": 60,
  "request":  { "method": "POST", "path": "", "query": {}, "json": {} },
  "response": { "format": "json", "rows": "hits", "fields": { "title": "title", "hash": "hash" } } }
```

- `fanout`: `per-term` asks once per search term, `per-title` once per distinct title (for an
  index that answers with every episode, or has no episode syntax), `once` a single time (the
  term plays no part, as with an IMDb id).
- `hosts` are mirrors of one service, tried in order. A host that fails rests for
  `cooldown_secs`.
- `fallback: true` makes an indexer a last resort: it is asked, with longer to answer (the
  `fallback_timeout_ms` setting), only when the others found nothing worth opening. For a slow service
  that should not be paid for on every search, such as the server's Prowlarr search
  ([pipeline/PROWLARR_FALLBACK.md](../../pipeline/PROWLARR_FALLBACK.md)). That entry sets the request header
  `x-akidev-fallback` (see `headers` below). An app that predates the flag asks it like any other indexer,
  without the header, and the server's route turns that away at once.
- `request.headers` is a map of headers sent with every request, for a service that wants a key or a marker
  in a header.
- In `path`, `query` and `json`, `{term}`, `{imdb}` (`tt0903747`), `{imdb_digits}` (`0903747`), `{imdb_or_term}` (the id when the search has
  one, otherwise the term),
  `{season}` and `{episode}` are filled in. A request that needs a value the search lacks (an
  IMDb id) is not made. `{"$kind": {"anime": ..., "movie": ...}}` is replaced by the entry for
  the kind being searched.
- `format` is `json` or `rss`. In an RSS feed a tag's text is read under the tag's name and its
  attributes under `tag@attribute`; a Torznab attribute (`<torznab:attr name="seeders" value="12"/>`)
  is `torznab:attr@seeders`, so any Torznab feed can be read. `rows` is a dotted path to the rows (`*` means every value of an
  object; a list found there is read element by element; empty means the body itself). `children`
  reads rows nested in each row, whose fields reach the outer row with `^.`.
- `fields` are `title` and `hash` (required), `magnet`, `size` (bytes, or text like `1.3 GB`),
  `file_idx` and `seeders`. Each is a dotted path, or one of: `{"template": "{^.title}[ ({^.year})]"}`
  (`{? ... ?}` vanishes when a hole inside is empty), `{"regex": {"from": path, "pattern": ...}}`
  (first capture group), `{"line": {"from": path, "skip": pattern, "index": n}}`,
  `{"coalesce": [...]}`, `{"join": [...]}`, `{"magnet_hash": path}`, `{"magnet_name": path}`,
  `{"magnet_size": path}`. Without a `magnet` field one is built from the hash and the trackers.
  Stremio-shaped addons, including Torrentio, may supply their description in either `title` or
  `description`; `behaviorHints.filename` identifies the selected file and `behaviorHints.videoSize`
  its exact size. `file_idx` reads the addon's zero-based `fileIdx`, preserving distinct files within
  one torrent as distinct choices without bypassing title/episode checks. Only a validated torrent
  `infoHash` is used, never an addon's credential-bearing direct-play URL.
- `map` names a mapper (see Mappers below) that finds the id this indexer wants. For the mapper `kitsu` its
  request fills the holes `{kitsu}` and `{kitsu_kind}`, wherever they are: in the path, a query value or the
  body. The mapper accepts only an exact same-number episode (`kitsu:7442:1`); a film instead uses its exact
  unsuffixed title id (`kitsu:50192`) and the `movie` route. A missing or mismatched mapping skips that indexer
  instead of falling back to IMDb or playing another episode. `anime_kitsu: true` is the old way to say
  `"map": "kitsu"`: it is still read, and the signed document still carries it for the apps that use it.
- `seeders` is optional observation data: missing, malformed, negative, fractional or out-of-range
  values stay unknown (`null` in API output); an explicit valid `0` remains observed zero. Do not
  define a default zero for providers that omit health. Counts are upstream reports, not live
  measurements made by Akidev.
- `empty_when` is a list of conditions (the same shape as `drop`) that read the whole body: when every one
  holds, the answer is a successful answer with no rows. For a service that leaves the list out when nothing
  was found, which would otherwise read as a bad answer. YTS says it like this:
  `[{"path": "status", "equals": "ok"}, {"path": "data.movie_count", "equals": "0"}, {"path": "data.movies", "exists": false}]`.
- `drop` removes rows (`{"path": ..., "equals": ...}` or `"starts_with"`); `must_contain_term`
  keeps only rows whose title holds the whole searched title; `dedupe` keeps one row per
  `(hash, file_idx)`. Across requests, duplicates of that pair merge before ranking: unique `sources`
  provenance follows planned source/query order, not arrival order. The first `source` label stays
  compatible; known seeder observations use the maximum, never a sum, and safe trackers are limited
  to the signed configured list (at most 32). Corroboration is not proof of title/episode identity.

### Search execution and failure semantics

The engine has process-wide admission for at most **96 new request producers** and **24 HTTP
exchanges**, including hedged mirrors. Each search's planned request list is also capped at 96.
A cache hit or a caller joining the same in-flight request does not start another producer.
Permit waits count against host timeouts; local congestion is not evidence that a mirror is down,
and a mirror that was never contacted is not put on cooldown for losing a hedge.

Successfully parsed per-request observations are cached for ten minutes when non-empty, sixty
seconds when empty. Definition version, endpoints and filled request context participate in the
key. HTTP errors, malformed/blocked responses, timeouts and admission failures are not cached as
healthy empty answers. Late bounded producers may fill their own successful cache entries, while
failed/pending variants remain visible in the internal source report. A partial aggregate is not
promoted into a new complete per-provider observation.

The shared core resolves catalogue identity and applies aliases, episode/season and known film-year
gates before ranking and again to actual torrent/debrid filenames. Startup needs the whole bounded
torrent head range. The server additionally probes media and budgets preparation by stream source;
these code controls cannot be added or changed by publishing definitions alone. A definition check
or fixture parse does **not** prove current reachability, correct episode content or sustained
playback. See [SOURCE_RELIABILITY_PLAN.md](../../pipeline/SOURCE_RELIABILITY_PLAN.md) for implemented
scope, remaining hypotheses and verification limits.

### Providers

```json
{ "id": "tvmaze", "kinds": ["series"], "base_url": "https://api.tvmaze.com",
  "types": {}, "sorts": {},
  "card":   { "id": "id", "title": "name", "year": { "year": "premiered" } },
  "list":   { "request": { "url": "{base}/schedule/full" }, "rows": "_embedded.show",
              "paging": { "style": "pool" } },
  "search": { "request": { "url": "{base}/search/shows?q={term}" }, "rows": "show" },
  "detail": { "request": { "url": "{base}/shows/{id}" }, "root": "" } }
```

- `image_hosts` are the hosts the provider's pictures are kept on (an entry matches that host and its
  subdomains). The website's image proxy fetches a picture only from these hosts, from the
  hosts other providers name, and from the ones the operator adds with the `IMAGE_HOSTS` setting.
- `card` says how a title is read, once, wherever the service gives one (a page, a search, a title
  in full, the titles related to it). Its `id` is the service's own id; the title's key is made from
  the kind and that.
- A request's `url` is the whole address. `{base}`, `{type}` (what `types` calls the kind), `{sort}`
  (what `sorts` calls the sort, `default` otherwise), `{id}`, `{term}`, `{page}`, `{per_page}`,
  `{skip}`, `{genre}`, `{genre_lower}` and `{year}` are filled in, each percent-encoded. A request's
  `url` starts with `{base}/`, so what is searched for can only ever go after the service's own
  address. A hole with
  nothing to fill it empties the address; `{? ... ?}` is left out when a hole inside is empty. A
  request can have `method`, `query` and a `json` body, where `{"$text": "..."}` is a text left as
  it is (a GraphQL document), `{"$number": "{page}"}` a number, `{"$sort": {"recent": ...}}` picks
  by sort, and a key ending in `?` is left out when its value is empty.
- `list.paging.style` is how the service pages: `page` (it takes `{page}` and `{per_page}`; `next`
  says whether more follows, by a `flag` or a `total`; `widen` asks for a wider page and filters here
  when the service cannot), `skip` (it takes `{skip}`; `window` reads a run of pages from the start
  and pages here when it cannot filter or order) or `pool` (it has no paging: one answer, paged
  here).
- What the service cannot do is done here: `dedupe`, `drop` (rows to leave out, with `after_today`
  for what is not out yet), `genre` and `year` (how a row names them, to filter on) and `order` (a
  list of keys, per sort, that a row is ordered by).
- `detail` says where the title is (`root`), which ids the service has (`id_pattern`), what a
  placeholder for a missing title looks like (`missing`), and where the rest is: `description`,
  `imdb_id`, `studios`, `trailer`, `episodes`, `cast`, `related` and `recommended` (the last two can
  have a `request` of their own). `check` says how an answer reports that it failed.
- Expressions are the same as for indexers, plus `const`, `by_kind`, `year`, `minutes`, `scale`,
  `add`, `positive`, `map` and `plain_text`. A template reads the row with `{path}`, the row around
  it with `{^.path}` and the kind with `{@kind}`.

### Mappers

A mapper finds the id a title has in another service, so an indexer that knows titles by that id can be asked
for it. `mappers` is a list of them, and an indexer names one with `map`.

```json
{ "id": "kitsu", "kinds": ["anime"],
  "provider": { "id": "anime-kitsu", "kinds": ["anime"], "base_url": "https://anime-kitsu.strem.fun",
                "card": { ... },
                "detail": { "request": { "url": "{base}/meta/anime/anilist:{id}.json" },
                            "id_pattern": "^[0-9]+$", "root": "meta",
                            "episodes": { "rows": "videos", "id": "id", "season": "season", "number": "episode" } } } }
```

- `id` is lower case letters. It names the holes of an indexer's request (`{kitsu}`, `{kitsu_kind}`) and the
  front of the ids the mapper's answer must hold: `kitsu:<title>:<episode>`, or `kitsu:<title>` for a film.
- `provider` is asked for the detail of the title, by our own id (AniList's, digits). Its `episodes` give the
  other service's id of each episode. An answer that does not hold exactly the requested episode, in exactly
  that shape, is no mapping.
- A document from before `mappers` gets the built-in ones.

### Subtitles

```json
{ "id": "opensubtitles", "name": "OpenSubtitles", "kinds": ["series", "movie", "anime"],
  "hosts": ["https://opensubtitles-v3.strem.io"], "file_hosts": ["strem.io"],
  "lookup": { "kinds": ["anime"], "url": "https://v3-cinemeta.strem.io/catalog/series/top/search={term}.json",
              "rows": "metas", "value": "imdb_id", "max_episodes": 30, "skip_title": "(?i)season\\s*\\d" },
  "request":  { "path": { "$kind": { "movie": "/subtitles/movie/{imdb}.json",
                                      "series": "/subtitles/series/{imdb}:{season}:{episode}.json" } } },
  "response": { "rows": "subtitles", "fields": { "id": "id", "url": "url", "lang": "lang",
                "label": "subtitleFileName", "format": { "const": "srt" } } } }
```

- `hosts`, `cooldown_secs`, `request` and the expressions in `response.fields` work as for
  indexers. `fields` are `url` and `lang` (required), `id`, `label`, `format` (`srt`, `vtt` or
  `ass`) and `hearing_impaired`.
- `lookup` finds an IMDb id for a title that has none (anime): it asks `url` with `{term}`, reads
  `rows`, takes `value` from the first. It is skipped for titles with more than `max_episodes`
  episodes or whose name matches `skip_title`, since such a title's numbering would not match.
- `file_hosts` are the only hosts a subtitle file is fetched from, whatever a reply names:
  a host is allowed when it is one of them or a subdomain of one.
- A subtitle is converted to WebVTT, which both the web player and mpv read.

### Screens

`screens` says what the apps draw, so the desktop app and the website show the same things from one
place. Absent in a document from before there were any: the copy built into the app is used.

```json
{ "tabs":   [{ "kind": "anime", "slug": "anime", "label": "Anime" }],
  "sorts":  [{ "sort": "popularity", "label": "Popular" }],
  "genres": { "anime": ["Action"], "series": [], "movie": [], "all": [] },
  "home":   [{ "title": "Trending Anime", "kind": "anime", "sort": "trending", "genre": "Action" }] }
```

- `tabs` are the catalogues, in the order of the top bar. `slug` is the address on the website (`/films`).
- `sorts` are the ways a catalogue can be ordered. The first is the one it opens on.
- `genres` are offered for one catalogue, by kind name, in the providers' own spellings. `all` is for the page
  that shows every catalogue.
- `home` are the rows of the home page, in order. `genre` is optional. A row's `kind` must have a tab.
- The server sends it at `GET /v1/screens`.

### Plugins

A plugin is one JSON file that brings sources of its own. Nobody signs it, so it is held to stricter rules than
the document above, and only the desktop app loads it: the server uses the signed document alone.

```json
{ "plugin": 1, "id": "my-torrents", "name": "My torrents", "version": 1,
  "indexers": [ { "id": "my-torrents-api", "name": "My torrents", "kinds": ["series"],
                  "hosts": ["https://torrents.example.com/api"],
                  "request":  { "path": "/search", "query": { "q": "{term}" } },
                  "response": { "format": "json", "rows": "items", "fields": { "title": "title", "hash": "hash" } } } ],
  "subtitles": [] }
```

- `indexers`, `subtitles` and `providers` have the same shape as in the document. A plugin has at least one of
  them, at most 16 indexers, 4 subtitle sources and 4 catalogues (providers).
- `id` is 1 to 40 lower case letters, digits or dashes, and it is the name of the file the plugin is kept in.
- Every address must be `https`, on a public name, with no login and no port. No IP address, no `localhost`,
  no `.local` or `.internal` name. A public name that points at a private address is not caught.
- A plugin adds its sources after the built-in ones. A source whose id is already taken is refused, so a plugin
  can add but never replace.
- The app keeps plugins in `<data folder>/plugins`. `name.json` is on and `name.json.off` is off. Settings lists
  them, switches them, removes them, and adds one from a link (https, followed once, never redirected, 256 KB at
  most). A link is fetched once: to update a plugin, add its link again.
- A catalogue (a provider) of a plugin has an id of 1 to 24 lower case letters or digits, and every address in it
  (its `base_url`, its `image_hosts`) follows the rule above. Its titles say which provider they came from:
  `series-myprovider~abc_2d12` is the title `abc-12` of `myprovider` (every character of the service's own id that
  is not a letter or a digit is written as `_` and two hex digits). So they never collide with the built-in
  catalogues, and the episode ids are made by the app, not by the provider's `episodes.id`.
- `kinds` brings kinds of title of its own, each with a tab: `{ "id": "documentary", "label": "Documentaries",
  "singular": "Documentary", "shape": "series" }`. The `id` is 1 to 16 lower case letters and not one there is. The
  `shape` says how its titles behave: `series` (seasons and episodes, keys like `documentary-…-s1e2`), `anime`
  (episodes by number, `-ep3`) or `movie` (no episodes). A plugin's indexers, subtitle sources and catalogues can
  serve a kind it brings, and nothing else it does not. A kind has no built-in catalogue, so its tab lists from
  the plugin's own, and its titles show in search with the others. A kind that a plugin brought stays known after
  the plugin is removed, so saved titles still read.
- `home` rows are drawn from one catalogue of the plugin alone: `{ "title": "Community picks", "kind": "series",
  "sort": "popularity", "provider": "myprovider" }`. They come after the built-in rows. The catalogue's titles are
  also added to every search, after the built-in results. A catalogue is not part of a kind's chain and has no tab.

A service that needs something these cannot say needs a change to the engine in
`core/src/providers/`, and so a new build.
