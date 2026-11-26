# Skeleton plan: apps render, JSON decides

Goal: the desktop app and the website only draw screens. The server and the core engine read JSON
(sources, catalogues, plugins) and do the work. A developer adds a source, a catalogue or a torrent list
with a JSON file and no code change.

Rule for every step below: **no current feature changes.** Each step adds data and tests first, then
removes the old code. Each step ships alone and can be rolled back alone.

This is a plan. Nothing here is built yet. Line numbers come from a code review on 2026-11-21; I checked the
main ones by hand (marked "checked"), the rest come from the review and need a second look when the step starts.

## Status (2026-11-21, updated)

Built and tested, not committed or published:

- **Step 0.** Count pins relaxed; the old-app gate (`desktop/scripts/check-old-apps.sh`, run by
  `publish-sources.sh`). `/v1/screens` has an end-to-end test; the screens contract is pinned in
  `core/src/sources.rs` tests and `akidev-frontend/test/screens.test.ts`.
- **Step 1.** YTS `empty_when`, `hinted` removed, image hosts from providers, `request.headers`. Dropped: 1.3
  (the three bonus-word lists mean different things) and 1.5 (credits are a legal statement: the owner's call).
- **Step 2.** `screens` in the document, `GET /v1/screens`, read by core, the desktop app and the website.
- **Step 4.** Id mappers (`mappers` in the document, `map` on an indexer). The hole is found in the path, a query
  value or the body. One document works for old and new apps, so no second feed was needed.
- **Step 5.** `Kind` is a copyable id with a shape; the three built-in kinds are constants; plugins can bring
  kinds. Provider-qualified keys (`kind-provider~id`) for plugin catalogues.
- **Plugins.** Indexers, subtitle sources, catalogues (providers and home rows) and kinds, desktop only, from a
  link or a file in the plugins folder, with stricter rules than the signed document.

- **Step 3.** `StreamSource` trait in `server/src/play.rs`, with the three sources, and 9 tests that run the
  orchestration with fake sources.
- **Step 3b.** A second feed: schema 2 under `/v2`, tried first, with the root feed as the fallback. The publish script
  and the daily tracker job publish both with one version number. Today the two documents are the same apart from
  the number; `sources/sources-v2.json` is where a value only newer apps can read would go.

Built for the phones (debug profile, with `RUSTFLAGS="-C target-feature=+fp16"`, which a speech dependency needs for
the iPhone device and the Android targets with this toolchain, also on the last commit): the engine for the iOS
simulator and device and for Android (arm64, NDK 28), the iOS app (arm64 simulator) and the Android debug app.
Not launched: the iOS simulator would not finish its first boot here, and the Android app was not run on a device.

Still hardcoded: the slugs in `next.config.mjs` and `scripts/cloudflare-cache.sh` (edge cache only), the small
`KIND_LABEL` in `site.ts`, and the website's three kinds (the server never loads plugins).

Release order that matters: publish the new `sources.json` (it carries `empty_when`, `headers`, `image_hosts`,
`screens`, `mappers`, `map`) **before** the server or app that needs them.

## How to publish this work (recipe, checked 2026-11-21)

*Since 2026-11-21 `sources.json` lives in the akidev-sources repo, so `sources/sources.json` below means that repo's `sources.json`, and `publish-sources.sh` is run with `SOURCES_DIR=<checkout of it>`.*

The repo copy of `sources.json` also holds the unreleased Kitsu change, which the old-app gate refuses for
every release up to v0.1.24 (check v0.1.25). So there are two ways:

1. **Release first.** Ship the app with the Kitsu change, run `OLD_REF=<that tag> ./scripts/publish-sources.sh`.
2. **Publish the new fields now.** Build the document from the live one. Run this in `desktop/`, with the live
   document saved as `live.json` (`curl -fsS https://sources.akidev.live/sources.json -o live.json`):

   ```bash
   jq --slurpfile repo sources/sources.json '
     .version += 1
     | (.indexers[] | select(.id == "yts") | .response.empty_when) = ($repo[0].indexers[] | select(.id == "yts") | .response.empty_when)
     | (.indexers[] | select(.id == "prowlarr") | .request.headers) = {"x-akidev-fallback": "1"}
     | .providers |= map(. as $p | .image_hosts = ($repo[0].providers[] | select(.id == $p.id) | .image_hosts))
     | .screens = $repo[0].screens' live.json > candidate.json
   ./scripts/check-old-apps.sh candidate.json
   cargo run -q -p akidev-core --bin akidev-sources -- check candidate.json
   ```

   Both checks passed for the candidate made from live v37 (v0.1.17 and the current validator). To publish it,
   put it at `sources/sources.json` in a scratch checkout (`git worktree add`), not in this tree, so the Kitsu
   change here is not lost, then run `./scripts/publish-sources.sh` there.

If the server or app ships **before** the document is published: YTS "no results" answers count as a failed
request (not cached as empty, the other indexers carry on), and the Prowlarr fallback answers 400 (it only runs
when the others found nothing). Posters and screens are safe, because the built-in copy fills in.

## 1. Where we are

*This section and section 3 are the plan as first written, from a review of the code before any of it was built.
The Status section above says what was built and where it differs: Step 4 turned out to need no second feed, the
second feed was built anyway, and `sources.rs` no longer pins the counts of indexers and providers.*


Good news first. You already have most of the idea:

- `desktop/sources/sources.json` is signed JSON. It holds 16 indexers, 4 metadata providers, 1 subtitle
  source and the chains (which provider answers for each kind, in order). Apps fetch newer copies every 6 hours.
- The provider engine (`core/src/providers/`) has no provider names in it. The subtitle engine is clean too.
- Ranking never looks at *which* source a release came from.

So the problem is not "we have no plugin system". The problem is that code still decides things that JSON
should decide. I found five groups.

**A. A few named-provider branches in the engine (small, easy)**
- `core/src/indexers/extract.rs:126` has `if def.id == "yts"` (checked). It exists because YTS answers an empty
  search without a `movies` list, and the generic code calls that an error.
- `core/src/releases.rs:570` and `:1496` accept the word `hinted` (checked). That word comes from a test file name
  and leaked into the real ranker.
- The "bonus / sample / extras" word list exists three times (`indexers/mod.rs:782`, `releases.rs:320`, and
  `torrentio.dedupe.avoid` in `sources.json`).

**B. The Kitsu bridge is a hardcoded integration, not data**
- `anime_kitsu: true` on an indexer, `{kitsu}` and `{kitsu_kind}` holes, and Kitsu id regexes live in
  `indexers/mod.rs`, `stream.rs` and `providers/mod.rs`.
- Its definition is a second file, `sources/anime_kitsu.json`, built into the app. It is **not** in the signed
  feed, so a change needs a new build.
- It only fires when `{kitsu}` is in the URL path (`stream.rs:738-750`, checked). Put `{kitsu}` in the query
  or body and the request is silently skipped.
- It assumes the anime provider is AniList.

**C. The set of kinds is closed**
- `core/src/model.rs:6` has `enum Kind { Anime, Series, Movie }` (checked). It reaches about 60 places: id
  parsing, chains, tabs, episode logic, search, the website regex.
- A `"manga"` string in the signed document makes old apps **reject the whole document** (serde error).
- The key format is `kind-providerid` with letters and digits only. Ids with `-` `_` `.` (UUIDs, slugs) are refused.

**D. Screens are hardcoded, in four copies**
- Home rows: `desktop/app/src/home.rs:28`, `desktop/server/src/api.rs:62` (`ROWS`, checked), and the loading
  placeholders in `akidev-frontend/src/components/home-view.tsx:11`.
- Tabs, sorts, genres: `desktop/app/src/browse.rs:23-81`, `akidev-frontend/src/constants/site.ts`,
  `app.rs:274`, `topbar.tsx`.
- Per-kind behaviour (episodes as a list or a count, "movie plays directly"): `episodes.rs`, `title.rs`,
  `watch.rs`, `tile.rs`, `hero.rs` and their TypeScript twins.
- Credits and the privacy page name AniList, Cinemeta, TVMaze, YTS in plain text, and already drift from
  `sources.json` (Kitsu is missing).

**E. The server's stream sources are hand-wired**
- `server/src/play.rs` has one `match` with three arms: `debrid`, `torrent`, `archive-org`.
- `debrid.rs` is TorBox only. `archive.rs` is archive.org only. `fallback.rs` is Prowlarr only (and `/health`,
  `deploy.sh`, `prowlarr-sync.js` name them).
- The image proxy allow-list is an env var (`server/src/config.rs:135`, checked). A new catalogue's poster host
  shows a broken image on the website until someone edits the env var and redeploys.
- Apps have **no way** to load a source that the owner did not sign: one `FEED_URL`, one `PUBLIC_KEY`.

## 2. The target

Three layers, each with one job.

1. **Definitions (JSON).** Say what exists: kinds, catalogues, home rows, tabs, sorts, genres, providers,
   indexers, subtitle sources, stream connectors, image hosts, credits.
2. **Engine (Rust: core and server).** Reads definitions and does the work. Names no provider.
3. **Skins (desktop app, website).** Ask the engine "what tabs, rows and kinds exist?" and draw them.
   The website asks the server over HTTP. The desktop calls core in the same process.

The three ways you described map onto this:

- **Desktop:** built-in list, then the signed feed, then the user's local plugins.
- **Website:** the server uses only your published list (no user plugins on the shared server).
- **Plugin:** one JSON file a developer shares. Desktop users install it.

## 3. The steps

Order is by risk: safety net, small wins, then the big changes. Steps 1 to 4 can ship without breaking any
installed app. Step 5 needs a feed decision.

### Step 0. Safety net (no behaviour change)

Today nothing guards what the user sees on home and browse, and nothing tests the order in which the server
tries stream sources. Fix that first. Done means these tests exist and pass on today's code:

- A test that calls `Player::resolve()` with fake sources and checks: order, equal time share, "archive is
  skipped when a release is pinned", and the "Unknown stream source" arm.
- A test for `GET /v1/home` in `server/src/e2e.rs` (not covered today).
- A "screen contract" test: rows, tabs, sorts and genres, written as a fixture file that today's hardcoded
  lists must match. Later steps change where the data comes from, and this fixture must still pass.
- A parse test per indexer already exists (`extract.rs`). Keep them. Relax the count pins
  (`sources.rs:396-397` asserts exactly 16 indexers and 4 providers, checked) so adding a source does not
  break the build.
- **An old-app gate.** Before any document is published, run `akidev-sources check` built from the oldest
  release we still support. Reason (checked in `sources.rs` and `definition.rs`): an installed app ignores
  unknown *fields*, but it **refuses the whole document** for an unknown *value*: a new hole name, a new `when`
  token, a kind it does not know, or any `schema` other than 1. A refused document also means the app stops
  getting tracker updates and `min_version`. So "additive" only means new fields, never new values in old fields.
- **Always start from the live document.** The daily tracker job publishes from the live document, so
  `desktop/sources/sources.json` in the repo can lag behind it. Download the live copy first, then edit it.
  `scripts/refresh-trackers.sh` only rewrites `.trackers` and `.version` with `jq`, so it keeps new sections
  such as `screens`. It will have to refresh the second feed (Step 1) too.

### Step 1. Remove the small hardcodings (low risk)

Each one is its own small change.

1. YTS special case: add `response.empty_when` (reuse the `Condition` type at `definition.rs:140`) and put
   the YTS rule in `sources.json`. Delete the `def.id == "yts"` branch.
2. Rename the `hinted` test fixtures. Remove the word from the two ranker regexes.
3. One shared "bonus words" list instead of three.
4. Image hosts from definitions: allow the hosts of each provider's `base_url`, plus an optional
   `image_hosts` list, plus the existing `IMAGE_HOSTS` env var. Copy the way `subtitles.rs:528` already does
   `file_hosts`. Only hosts from a signed or validated document count. Keep the https-only and redirect rules
   and the test `only_listed_hosts_over_https_are_fetched`.
   Careful: four of today's eight hosts (`kitsu.app`, `metahub.space`, `media-amazon.com`, `tmdb.org`) are
   not the `base_url` of any provider. So the built-in document must list **all eight** (`anilist.co`,
   `kitsu.app`, `tvmaze.com`, `metahub.space`, `media-amazon.com`, `tmdb.org`, `yts.gg`, `yts.mx`) in
   `image_hosts`, and a test must prove the allow-list is the same before and after.
5. Credits from data: add `name` and `attribution` to each provider. The footer and the privacy page read
   the list. This also fixes the missing Kitsu line.
6. `request.headers`: a generic header map on a request. Keep sending `x-akidev-fallback` for fallback
   indexers for one more release, then put it in the Prowlarr entry. Old apps ignore unknown fields
   (`fields_a_newer_format_adds_are_ignored`), so nothing breaks.

### Step 2. A screen manifest (home, tabs, sorts, genres)

Add one new section to `sources.json`. It is a new field, so `schema` stays 1 and old apps ignore it. Its
values (kind names, sort names) are only ever the three kinds and five sorts that exist today.

```json
"screens": {
  "tabs":  [{ "id": "anime", "label": "Anime", "kind": "anime" }, ...],
  "home":  [{ "title": "Trending Anime", "kind": "anime", "sort": "trending", "genre": null }, ...],
  "sorts": [{ "id": "popularity", "label": "Popular" }, ...],
  "genres": { "anime": ["Action", ...], "series": [...], "movie": [...] }
}
```

- The built-in copy holds **exactly today's values**. The Step 0 contract test proves it.
- Server: new `GET /v1/config` returns it. `/v1/home` builds rows from it. `RowOut.title` becomes a `String`.
- Desktop: reads the same structure in the same process. Delete `ROWS`, `home.rs rows()`, `browse.rs`
  genre and sort lists, `site.ts` lists, `LOADING_ROWS`.
- Website: fetches `/v1/config` on the server side (pages are server-rendered with `serverGet`), so pages still
  render with no extra flash. Keep what `web-performance.md` says: skip the data on router moves
  (`isClientNavigation()`), add no `loading.tsx`, never cache `_rsc` requests, keep the default-sort edge cache.
  Keep the config in server memory so it adds no extra trip to the origin.
- **Not checked yet, check at the start of this step:** the review says `app/[kind]/page.tsx` uses
  `generateStaticParams` with `dynamicParams = false`, and `next.config.mjs:31` has a route
  `/:kind(anime|series|films)`. If that is true, the tab list is needed at build time. Then either the build
  reads `/v1/config` once, or the route accepts any slug and the page asks the config (a miss shows the normal
  "not found" state card).
- This step still uses the three fixed kinds. It only moves the lists into data.

### Step 3. Server connectors (stream sources)

Server only. Users see nothing change.

1. Write the Step 0 `resolve()` test first.
2. Make a `StreamSource` trait with one method that answers "give me bytes for this title by this deadline".
   Move the three `match` arms into three impls. Turn `origin.rs` `Origin` into a trait with the same four methods
   it has now. `play.rs` loops over a list instead of a `match`.
3. Replace the magic `STREAM_SOURCES` strings with a list of connector entries (`id`, `type`, settings). Secrets
   stay in env vars, referenced by name.
4. Prowlarr becomes one config entry of a generic "search proxy" type. `/health` shows a generic `connectors`
   map. `prowlarr-sync.js` reads its site list from a file.
5. Debrid stays Rust code (each vendor has its own flow). TorBox is the first adapter. A second vendor is a new adapter.
6. Archive keeps its Rust identity checks (title, year, runtime, `ffprobe`). Later, its endpoints, field paths and
   format ranking can move to JSON as an "HTTP library" definition. This is the best future plugin type, but it is
   optional.

Careful: archive is the only source that skips the delivery check and goes straight to the browser, and it skips
pinned releases. Keep both behaviours. Give the trait a `capabilities()` for them.

### Step 4. Generalise the Kitsu bridge

**Needs the second feed first** (see "Step 3b" below). A new hole name is a new *value*, and an installed app
refuses a document with a hole it does not know. So the new shape cannot sit in the schema 1 document.

Rename the idea to **id mapping**: "to search this indexer, first ask provider X to map our id to its id".

- Move `anime_kitsu.json` into the signed document.
- Replace `anime_kitsu`, `{kitsu}` and `{kitsu_kind}` with a named mapping in JSON: its request, the id pattern,
  and the movie/series route. Detect the hole in path, query and body (fixes the silent skip).
- Keep every exact-episode safety rule. The tests at `indexers/mod.rs:1945-2033` and the Kitsu tests in
  `providers/` must pass without edits to what they assert.
- The schema 1 document keeps the old shape for old apps until `min_version` retires them.

### Step 3b. Two feeds (before Step 4)

Today every installed app reads one URL (`FEED_URL`, `core/src/sources.rs:24`) and refuses any `schema`
other than 1. Anything that changes a value (Step 4, Step 5) needs a place to live that old apps never read.

- Publish `schema 2` at a second URL. Apps that know schema 2 read it first; the rest keep reading schema 1.
- Keep schema 1 alive and in step with schema 2 for what old apps need: trackers, `min_version`, and every
  source that still works for them. The daily tracker job must refresh both documents.
- Run the old-app gate from Step 0 on every schema 1 publish.
- The signing key and `publish-sources.sh` stay the same.

### Step 5. Open kinds (the big one)

Replace `enum Kind` with a string id that the manifest defines.

```json
"kinds": [{ "id": "anime", "label": "Anime", "episodes": "count",
            "play_directly": false, "id_suffix": "ep" }, ...]
```

- The descriptor tells the skins how to behave (`episodes`: `list`, `count` or `none`; `play_directly`; labels).
  This removes the `match Kind` branches in `episodes.rs`, `title.rs`, `watch.rs`, `tile.rs`, `hero.rs` and the
  TypeScript copies. The server should also send a real episode list for anime so the skins stop inventing `1..N`.
- Keep the key format for `anime`, `series`, `movie` exactly (`anime-123`). Saved library, history, downloads and
  party links use it.
- Widen the id rule (letters, digits, `.`, `_`, `~`) only for new kinds.
- An unknown kind breaks old apps, so this lives in the schema 2 feed from Step 3b. Raise `min_version`
  later to retire old apps.
- Music and books stay on their separate path until this step is done. They are not `Media`.

### Step 6. Plugins (desktop first)

A plugin is one JSON file: its own `id`, `version`, `requires_schema`, and any of: indexers, providers,
subtitle sources, home rows, kinds, image hosts, credits.

- Load order: built-in, then signed feed, then local plugin folder. A plugin can add; it cannot silently replace
  a built-in id (a clash is refused and shown).
- Same validation as the signed document (hosts must be https, regexes compile, requests fillable), and a
  clear "unverified" label in Settings with an on/off switch per plugin.
- A user can add a plugin from a local file or from a link (to one plugin file or to a list of plugins). The
  app downloads it, validates it, and keeps a copy. A link is re-checked on the same 6-hour schedule as the feed.
- The website server loads **only** your list, never user plugins. A shared server must not call arbitrary
  hosts a stranger chose. A community plugin reaches the website only after you review it and add it to
  your signed list.
- Relax the caps in `sources.rs:89-95` (64 each) to apply to the merged list.
- Needs a trust decision from you (see section 5).

### Step 7. Smaller clean-ups (any time after Step 2)

- Fix mismatches found in the review: web `Episode.season` is a number but Rust allows none; status text differs
  ("Airing" on web, "Releasing" on desktop); the web party code has no `music` item.
- Let the server compute `first_playable`, next episode and release badges, so the TypeScript copies can go.
- `is_film` compares the text `"movie"` (`stream.rs:699`). Normalise in the provider JSON instead.
- The `identity_tail` language word list (`releases.rs:581`) silently drops titles like "Show Latino 1080p".
  Make it data, or widen it.

## 4. What I would not touch

- `manga-app/`, `manga-android/`, `desktop/manga*`: separate product, they do not use this code path.
- Ranking heuristics (Japanese romanisation folds, health tiers). They are tuning, not source handling.
- The torrent engine, HLS, party signalling.

## 5. Decisions I need from you

1. **DECIDED 2026-11-21: both.** Your signed list is the default and the only thing the website server uses.
   Desktop users can also add a plugin by link or local file, shown as "unverified", with an on/off switch.
   Options that were considered, for the record: **How do plugins reach people?** You said "I will just add the link", and that plugins install in the
   desktop app and on the website. The plan so far assumes a local plugin folder on the desktop and no
   plugins on the website server. Three options:
   - **Curated:** only you sign and publish. Community sends you a JSON file, you review and add it to the
     list. Safest. Works for desktop and website. Slowest for developers.
   - **Links:** a user pastes a link to a plugin file or a plugin list (an unsigned feed they trust). Works on
     desktop. On the website, a shared server must not call hosts a stranger chose, so website plugins would
     need to be reviewed and added to your list, or run per user.
   - **Both (my suggestion):** your signed list is the default and the only thing the website server uses.
     Desktop users can also add a link or local file, shown as "unverified", with an on/off switch.
2. **Where does the desktop get its data?** You described the server as the part that talks to JSON, plugins
   and connectors. Today the desktop calls the core engine in the same process and does not call the server
   (only watch parties use the server). I kept that, because the desktop works offline and has no extra hop.
   The skins would still be thin either way. Tell me if you want the desktop to call the server instead.
3. **Old apps:** second feed URL for `schema 2` (my suggestion), or force everyone to update with
   `min_version` first. The second feed is gentler, but it means two documents to keep in step.
4. **Step order:** 0, 1, 2, then 3, then 3b and 4, then 5, then 6. Step 6 is what the community sees, but it
   is only safe after 2 and 5, so the plugin format does not change twice.

## 6. How we keep features working

- Step 0 tests land first and stay green through every step.
- The built-in document always reproduces today's behaviour. A step is done when the old code is deleted and the
  same tests pass.
- Steps 1 and 2 add only new fields, which old apps ignore. Steps 4 and 5 change values, so they go in the
  schema 2 feed (Step 3b). The old-app gate checks this before every publish.
- Rust changes ship to desktop and library through a release; the website and server ship through the existing
  deploy. A signed document alone cannot fix installed code, so plan each step's release.
- Rollback for data steps is publishing a corrected document with a higher `version`.
- Each step runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, and `npm test`,
  type check, lint and build for the website.
- Before and after each step, take the same smoke path in both apps: home, browse, search, open a title, play an
  episode, play a film, subtitles, a watch party.
