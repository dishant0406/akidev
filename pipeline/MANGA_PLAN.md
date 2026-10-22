# Manga and manhwa reader (Swift, Liquid Glass, Rust core): plan

Status: plan only, nothing built. Written 2026-10-21 from web research and an audit of our own crates.
Decisions needed are in section 10. Facts marked **(unverified)** came from memory or a thin source and
must be checked in the spike phase.

## 1. The answer

A native Swift app (SwiftUI, Liquid Glass) for manga, manhwa and manhua, with a Rust engine inside it and our
own source-definition system. It is separate from the streaming app: own app, own bundle id, own repository
folder. It shares what already exists in `desktop/`: the HTTP client, the signed-definition pipeline, the
expression engine, the SQLite and key-store code.

The one decision that shapes everything: **a source is data first, code second.** Most sites can be described
with a JSON definition (which request to make, which CSS selectors or JSON paths to read). Only the hard
sites need code, and that code runs in a sandbox. Paperback and Mihon go the other way (every source is a
program); that is why their extensions break often and need a developer to fix them. Ours should be fixable
by editing and re-publishing a JSON file, which we already know how to do.

## 2. What the others do (research)

| App | Source model | What to take | What to avoid |
| --- | --- | --- | --- |
| **Paperback** (iOS, MIT, TypeScript/JS extensions) | A source is a JS module with three required functions, `getMangaDetails(mangaId)`, `getChapters(mangaId)`, `getChapterDetails(mangaId, chapterId)`, built with `createManga/createChapter/createChapterDetails`, plus optional search and home-page sections. Users add a *repository* URL and install sources at runtime. Its own wiki says the system is "practically the same" as Tachiyomi's. Version 0.9 sources do not run on 0.8. It has no content servers of its own. | Repository URL + runtime install; the small required surface (details, chapters, pages); a `longstrip` flag on chapter details (a manhwa signal); ships with no sources. | JS sandbox with no direct HTTP (sources go through a request manager), breaking changes between app versions that strand every source. |
| **Aidoku** (iOS/macOS, Swift, GPL) | Sources are WebAssembly modules (`.aix` packages), written in Rust with the `aidoku-rs` SDK (crates `aidoku`, `aidoku-cli`, `aidoku-test`, test runner; `aidoku init` scaffolds one). Source *lists* are added in Settings and installed from Browse. Built-in clients for Komga, Kavita and Suwayomi, local CBZ, downloads, trackers (AniList, MAL), OCR dictionary. | WASM for the hard cases; test tooling for source authors; built-in self-hosted-server clients; local CBZ. | Rust-to-WASM means every source author needs a toolchain, and the host ABI is a permanent contract. (Package layout and host imports: **unverified**, read the aidoku-rs book chapters before copying anything.) |
| **Mihon / Tachiyomi** (Android, Kotlin APK extensions) | Each extension is an APK with `HttpSource` subclasses: popular, latest, search, details, chapter list, page list, each with request and parse halves; `ParsedHttpSource` adds CSS selectors; multi-site "multisrc" themes share code; repos are an `index.min.json` (**exact file name unverified**, one guide says `index.pb`). | The method set (the de-facto standard that most site code is written against); theme reuse (many sites run the same CMS); the popular/latest/search split. | Needs the JVM and Android; unusable on iOS directly. Suwayomi runs these extensions on a server, which is a legal and operational burden we do not want. |
| **Suwayomi-Server** (MPL-2.0) | Runs Mihon extensions headless, exposes OPDS 1.2 at `/api/opds/v1.2` (with OPDS-PSE page streaming), supports FlareSolverr for Cloudflare. | An adapter target for users who already run it. | Do not embed or ship it. |
| **Komga** | OPDS 1.2 and 2.0 plus a REST API. | Second adapter. | |
| **Kavita** | Server for mixed libraries incl. light novels. | Third adapter (low priority). | |
| **MangaDex** | Official public REST API, 5 requests/s per IP (a minimum, not an exact cap), a mandatory honest `User-Agent` (spoofing is forbidden), no `Via` header (no non-transparent proxies), image delivery through `/at-home/server/{chapterId}` (a per-path limit near 40/min was reported by third-party wrappers, **unverified**). | The one source that is legitimate by design: it is what lets the app ship *with* a working catalogue. Respect the limits, send our own User-Agent, never route images through a proxy. | |

Interpretation: all of them converge on **repository URL → list of installable sources → six operations**
(popular, latest, search, details, chapters, pages) **+ filters + settings**. Our definition format should
speak exactly these six operations, so that porting an existing Mihon or Aidoku source's logic is mechanical.

## 3. What we already have (audit of `desktop/`)

- `core/src/definition.rs` (1,017 lines): a JSON definition format with an `Expression` type (dotted path,
  list, and operations: `template`, `const`, `regex`, `coalesce`, `join`, `map`, `plain_text`, `scale`, ...),
  `schema` and `version` fields, validation that rejects bad patterns and unfillable requests.
- `core/src/expression.rs`, `template.rs`: the evaluators. `{path}`, `{^.parent}`, `{? ... ?}` optional groups.
- `core/src/providers/` (`list`, `detail`, `card`, `fetch`, `disk`): a generic engine that turns a definition
  into list/search/detail calls with paging styles (`page`, `skip`, `pool`) and a `read_tests.rs` that checks a
  definition against a recorded response.
- `core/src/sources.rs` + `sources/README.md` + `scripts/publish-sources.sh`: ed25519-signed documents,
  built-in copy plus a background-fetched newer one, version must rise, `min_version` lock-out, R2 bucket at
  `sources.akidev.live`. **This is the distribution system Paperback and Aidoku lack: signed, versioned,
  validated before use, with a safe rollback (publish a higher version).**
- `core/src/http.rs`: one client, timeouts, size cap (`read_capped`). `core/src/cache.rs`.
- `library/` crate: SQLite store (`rusqlite`), encrypted container, key store with a file fallback on iOS.
- `server/src/img.rs`: image pipeline (decode, webp) on the server.
- `pipeline/MOBILE_PLAN.md` already concluded: Rust crates are portable behind uniffi; switch `reqwest` from
  native-tls to **rustls** for phones; native SwiftUI over uniffi is the best fit (option B).

What is missing for manga: an HTML engine (the current engine reads JSON and RSS only), an image-page
engine (referers, cookies, unscrambling), a reading model (chapters, progress, long strip), and a client for
self-hosted servers.

## 4. Architecture

```
┌──────────────────────── Swift app (SwiftUI, Liquid Glass) ───────────────────────┐
│ Library · Browse · Search · Updates · History · Reader · Settings · Sources       │
└───────────────▲──────────────────────────────────────────────────────────────────┘
                │ uniffi (async, typed Swift API)         nothing else: no web view
┌───────────────┴────────── akidev-manga-ffi (thin facade) ──────── native only ─────┐
│                          akidev-manga (new Rust crate)                              │
│  catalogue  ·  library db  ·  reader prefetch  ·  downloads  ·  updates  ·  sync    │
│  ┌─────────── source runtime ───────────┐  ┌──────── adapters ────────┐             │
│  │ 1. declarative engine (JSON+HTML)    │  │ (none: no built-in     │             │
│  │ 2. WASM sandbox (hard sites)         │  │ Komga / Kavita / OPDS    │             │
│  └──────────────────────────────────────┘  │ Suwayomi (OPDS-PSE)      │             │
│  shared with desktop/core: http, expression, template, definition, signing, cache    │
└────────────────────────────────────────────────────────────────────────────────────┘
```

**Rule: the engine runs on the phone, not on our server.** Reasons: (a) our own history says it, the server's
IP is blocked by Cloudflare where a user's home IP is not (we already tried and removed Byparr for exactly
this, see the Prowlarr fallback memory), (b) the user's device is the one fetching the user's chosen sites,
which also keeps us out of the data path, (c) no server cost per reader. The server's only manga jobs are
optional: host the signed definition feed (R2, already there) and, later, account sync.

### Crates and folders

- `desktop/manga/` → new workspace member `akidev-manga`: no UI, no GPUI, depends on `akidev-core`. Split
  `akidev-core` features so the manga crate does not pull torrents: put `librqbit`, `axum`, and `serve` behind a
  `streaming` feature (default on for desktop and server), so a phone build compiles without them. This is the
  first real refactor and should be spiked first (section 9, spike 1).
- `desktop/manga-ffi/` → `akidev-manga-ffi`, a `cdylib/staticlib` exposing the facade with uniffi.
- `manga-app/` (repo root) → the Xcode project and Swift package `AkiManga` (the generated bindings plus the
  xcframework as a binary target, per the usual uniffi pipeline: build three iOS targets, lipo the simulator
  slices, `xcodebuild -create-xcframework`).
- Keep the Rust facade async and make every long call cancellable: uniffi does not cancel async work by
  itself (**reported by third parties**), so each page-list or search call takes a handle the Swift side can
  drop.

## 5. The source definition system ("Aki definitions")

Same philosophy as `sources.json`: nothing in a definition is code, it says which requests to make and which
fields to read. One file per source, grouped in a **source list** (the repository).

### 5.1 Layers (pick the lowest that works)

1. **Declarative** (about 80% of sites by my estimate, **unverified**: count the top 60 sites in Mihon's repo
   in spike 3). JSON requests, HTML via CSS selectors, JSON via paths.
2. **Theme + overrides.** Many sites are the same CMS (Madara, MangaThemesia, Mangabox, Foolslide, ...).
   A *theme* is a base definition; a site is `{"extends": "madara", "base_url": ..., "overrides": {...}}`.
   Fixing the theme fixes every site on it, and adding a site is five lines. (This is Mihon's multisrc idea
   done as data.)
3. **WASM module** for the rest (custom auth, signed URLs computed with JS-like logic, image descrambling that
   cannot be a fixed op). Written in Rust or AssemblyScript, runs in `wasmtime` or `wasmi` (**decide in spike 4**:
   `wasmi` is a pure interpreter and App Store-friendly, since iOS forbids JIT in third-party apps; wasmtime
   needs its non-JIT configuration). Host imports are tiny and capability-based: `http`, `html`, `json`,
   `log`, `settings`. No filesystem, no clock beyond what the host gives, memory and time limits per call.
   Design the host ABI as a versioned contract from day one (`abi: 1`), since Aidoku's experience is that this
   surface is permanent.

A JavaScript runtime (JavaScriptCore, which iOS ships) is a possible fourth layer, to run existing Paperback
extensions. **Not in v1.** It is the largest compatibility prize and the largest security and maintenance cost;
revisit when the definitions are stable.

### 5.2 Format (sketch)

```json
{
  "schema": 1, "id": "example", "name": "Example", "version": 7,
  "lang": ["en"], "nsfw": false, "kind": ["manhwa", "manga"],
  "base_url": "https://example.org", "mirrors": ["https://example.net"],
  "rate": { "per_second": 2 },
  "headers": { "referer": "{base}/" },
  "popular": { "request": { "url": "{base}/series?page={page}&order=popular" },
               "rows": { "css": "div.series-card" },
               "card": { "id": { "attr": ["a", "href", "regex:/series/([^/]+)"] },
                         "title": { "text": "h3" }, "cover": { "attr": ["img", "data-src"] } },
               "paging": { "style": "page", "next": { "exists": "a.next" } } },
  "latest":  { "...": "same shape" },
  "search":  { "request": { "url": "{base}/search?q={term}&page={page}{? &genre={genre}?}" }, "...": "..." },
  "filters": [ { "id": "genre", "type": "select", "options": "static or from a request" },
               { "id": "status", "type": "multi", "options": ["ongoing", "completed"] } ],
  "detail":   { "request": { "url": "{base}/series/{id}" }, "title": {...}, "description": {...},
                "status": { "map": ["ongoing->ongoing", "completed->completed"] }, "tags": {...} },
  "chapters": { "request": {...}, "rows": {...},
                "chapter": { "id": {...}, "number": { "regex": "..." }, "title": {...},
                             "date": { "date": ["text", "MMM d, yyyy", "relative"] } },
                "order": "newest-first" },
  "pages":    { "request": { "url": "{base}/chapter/{id}" },
                "images": { "css": "div.reader img", "attr": "data-src" },
                "reading": "longstrip",
                "image_headers": { "referer": "{base}/" },
                "unscramble": null },
  "settings": [ { "id": "quality", "type": "select", "default": "high" } ]
}
```

### 5.3 What must be added to the engine

New expression operations, on top of the existing ones (kept in the same `Operation` enum so JSON and HTML
sources share `template`, `regex`, `coalesce`, `map`, `plain_text`, `scale`):

- HTML: `css` (select rows or one node), `text`, `html`, `attr`, `exists`, `next_sibling`, using the `scraper`
  crate (html5ever, pure Rust, no C build).
- `date`: parse absolute formats (given a pattern) and relative text ("3 days ago", "2 hours ago", and
  localized forms), producing a unix time; this is the most common source bug in every app.
- `number`: pull a chapter number out of titles ("Ch. 12.5", "Chapter 12 - Name", "Episode 12"), the second
  most common bug.
- `json_in_html`: read a JSON blob out of a `<script>` tag (Next.js `__NEXT_DATA__`, `window.__data = ...`),
  common on modern sites.
- `base64`, `url_join`, `url_decode`, `percent_encode` for the odd cases.
- `unscramble`: a closed set of image operations for sites that scramble the pages (block shuffles with a
  fixed or per-page seed), done in Rust after download, so no code is needed. Sites that need more go to WASM.

Requests: the existing `url`, `method`, `query`, `json` and `{? ?}` rules stay. Add `cookies`, `form`, a
`headers` map with the same templates and `rate` (per source token bucket). **There is no web view, anywhere in
the app** (decided 2026-10-21, see section 15): a site that answers with a Cloudflare or anti-bot page makes the
engine return a typed `Challenge` error, and the app shows a "this site checks visitors first" state with the
native ways round it, never a blank list.

Validation at install (as `sources.rs` does now): schema, limits, regexes compile, every URL starts with the
declared base or a mirror (a definition cannot send a request anywhere else: the same rule as `{base}/` in
the existing providers), selectors parse, required fields present. Plus a **`check` command** and recorded
fixtures per source (`fixtures/<id>/popular.html`, ...) so a CI job runs every definition against saved
responses (`read_tests.rs` already does this for providers) and a nightly job runs them against the live sites
and opens a report. A source whose live check fails for three days is flagged in the list.

### 5.4 Distribution: source lists

- A **source list** is a signed JSON index: `{schema, version, name, sources: [{id, version, url, sha256,
  min_app, lang, nsfw, icon}]}`. Same ed25519 signing and "version must rise" rule as today.
- **Akidev ships one list, `sources.akidev.live/manga/index.json`**, with a small curated set (section 8
  explains which). **Users can add any other list by URL.** The app trusts a list by the user's act of adding
  it: lists we did not sign are shown with a plain warning, their sources run in the same sandbox, and they
  can be removed with their data.
- Third-party lists need their own keys. The index carries the list's public key on first add (trust on first
  use) and the app refuses a later document signed by another key.
- WASM modules are content-addressed (`sha256` in the index) and sit in the same bucket or anywhere HTTPS.
- `min_app` and `abi` lock out sources the app is too old to run, instead of breaking, which is what burned
  Paperback 0.8 users.
- Authoring kit: `akidev-sources` already has `keygen` and `check`; add `akidev-manga-sources init`, `check`,
  `test --live`, `pack` (build a module or sign a definition) and a docs book, since Aidoku shows a good
  authoring path decides how many sources exist.

## 6. The reader and the app (Swift, Liquid Glass)

Targets: iOS 26 and iPadOS 26 first (Liquid Glass is iOS 26+; the SwiftUI APIs are `glassEffect(_:in:)`,
`GlassEffectContainer`, `glassEffectID` for morphing, `.buttonStyle(.glass)`; standard bars, tab bars, sheets
and toolbars adopt it automatically on the current SDK). macOS (Mac Catalyst or native SwiftUI target) second.
Support iOS 18 with a material fallback only if the numbers justify it (decision, section 10).

**Look: same product as the rest of Akidev**, per `design/DESIGN_PLAN.md`: warm ink canvas, amber only for
state (progress, active, focus), serif display titles, tracked-capital metadata, state cards instead of bare
empty screens. Liquid Glass is the chrome (tab bar, reader toolbar, filter sheets, mini controls), never the
content: covers and pages stay flat and sharp. Rules from Apple's guidance worth enforcing: glass for
controls and navigation only, group glass in a `GlassEffectContainer` (glass cannot sample other glass, and it
is costly), `.interactive()` only on tappable elements. Add the three Aki buttons as glass styles
(`primary`, `secondary`, `text`) in one design module, mirroring `desktop/app/src/design/`.

### Screens

1. **Library**: grid of covers, categories, unread badges, sort and filter, continue-reading rail, pull to
   refresh updates. Local-first, works offline.
2. **Browse**: sources and source lists, per-source Popular / Latest / Search with that source's filters;
   a "global search" across enabled sources, results grouped per source (ours is a fan-out with per-source
   time limits and the slow ones filling in).
3. **Series page**: cover and metadata (serif title, tracked-caps line), chapter list with read state,
   download, "continue from chapter N", and **"Also on"**: the same series on other enabled sources, so a dead
   source is a switch, not a dead end (matching by title and aliases, user can confirm; migration moves read
   progress).
4. **Reader** (the part that decides if the app is good):
   - Modes: **vertical long strip (default for manhwa, via the `longstrip` flag)**, paged left-to-right,
     paged right-to-left (manga), and a two-page spread on iPad/landscape.
   - Long strip: continuous scroll with no gaps, optional gap, side margins, tap-zones and a scrub bar by
     chapter, **seamless chapter-to-chapter** (the next chapter's first pages prefetched and joined under a
     thin chapter marker), position restored to the exact image and offset.
   - Image pipeline in Rust: decode off the main thread, **downsample to screen width** (a 1,200-px-wide
     strip image of 20,000 px height will kill memory otherwise; slice very tall images into tiles),
     prefetch the next N pages on a budget that respects cellular settings, disk cache with LRU, request
     headers per source, retries with backoff, progressive placeholders with the amber hairline.
   - Controls hidden on tap, glass overlay; brightness, page-fit modes, double-tap zoom, pinch zoom that
     does not fight scroll, haptics on chapter change, Dynamic Island / Live Activity not needed.
   - Accessibility from the first build: VoiceOver on chrome and lists, Reduce Motion, Dynamic Type in the
     metadata and lists.
5. **Updates**: new chapters across the library; background refresh through `BGAppRefreshTask`, which is
   best-effort on iOS (not a promise): also check on launch and on pull-to-refresh, and say so in the UI
   instead of promising notifications.
6. **History, Downloads, Settings, Sources, Trackers**.

### Local data (Rust, SQLite)

Reuse `library`'s rusqlite approach: tables for `source`, `series` (keyed by `source_id + series_id`),
`chapter`, `progress` (chapter, page index, offset, finished), `category`, `download`, `history`, and a
`mapping` table linking duplicates across sources and tracker ids. Library backup and restore as one file
(also import from Paperback and Aidoku backups later, since people will arrive with libraries). Downloads are
chapter folders of original images (CBZ export optional), counted in a storage screen. Optional encrypted
container for downloads already exists in `library/container.rs`, use it only if asked.

### Trackers (v1.1)

AniList first (GraphQL, OAuth with PKCE and a custom URL scheme), then MyAnimeList. Sync read progress per
chapter. Match by title search with a confirm step; remember the mapping.

### Sync (v2, optional)

A small account on the Akidev server for library and progress between devices, end-to-end encrypted so we do
not hold anybody's reading list. Not in v1: iCloud (CloudKit) is the zero-server answer for an iOS-only app,
decide when we get there.

## 7. Adapters (no source files, just clients)

- **MangaDex**: a plugin like any other since 2026-10-21 (`manga-sources/sources/mangadex.json`, section 16); the
  rules below are how that definition behaves, not code in the app. Follow its rules: honest `akidev-manga/<version>` User-Agent, no
  `Via`, a 5/s per-IP limiter (a shared token bucket, with `429` backoff), at-home server URLs fetched per
  chapter and report image success and failure to their `https://api.mangadex.network/report` endpoint as
  the docs ask (**check the current docs**: 503 when fetched on 2026-10-21 so this is unverified), content
  rating filter defaults to safe, language filter by user setting.
- **Komga**: REST plus OPDS; **Kavita**: REST/OPDS; **Suwayomi**: OPDS 1.2 with PSE streaming
  (`/api/opds/v1.2`). Each is a "server source": user enters URL and credentials (Keychain), the app lists
  libraries, series, books and streams pages. Local CBZ/CBR/folder import through the Files app.

## 8. Legal and distribution

- Scraping third-party sites for chapters is a copyright grey-to-red area, and it is the single biggest risk
  to this app existing. The structure that Paperback and Aidoku use is the defensible one: **the app ships
  with no scraped sources.** What we ship: a reader, MangaDex (an official API that wants third-party apps),
  self-hosted server clients, local files, and the ability to add a source list by URL. Our curated list
  (`sources.akidev.live/manga`) should hold only sources that are legal to read (official publisher sites,
  free webcomic platforms with public APIs, open licences). Anything else is a list somebody else publishes.
- **Keep the third-party list entirely out of our infrastructure and docs**: no default lists beyond ours, no
  "recommended" ones, no links in the app. This mirrors Paperback's own position (it "has no content servers"
  and users add sources themselves), and is what has let it stay on the App Store: Paperback's App Store
  listing is live, described as a Komga client with a scripting API (**listing verified, review outcome and
  Apple's current stance on user-added sources not verified**, read Guideline 5.2 and 4.7 on "mini apps"
  and downloaded code before submitting).
- Guideline risk specific to us: downloaded executable content. Declarative definitions are data, WASM run in
  an interpreter is the same category as JS in a WebView, native-code downloads would not be allowed (we do not
  do them). Ship `wasmi` (interpreter), not a JIT.
- Ship order of channels: TestFlight + sideload first (we already plan IPA distribution in `MOBILE_PLAN.md`),
  App Store only after a build that has only MangaDex, servers and local files is clean, with the
  source-list URL box behind an "Advanced" switch.
- NSFW: sources flagged `nsfw`, hidden unless enabled with an age gate, never in our curated list.

## 9. Phases

| Phase | Weeks | Outcome |
| --- | --- | --- |
| **0. Spikes** | 1 to 2 | Answers below. No product code kept. |
| **1. Engine** | 3 | `akidev-manga` crate: the HTML ops, requests with cookies and rate limit, the six operations, fixtures + `check`, the MangaDex adapter, SQLite schema. Command-line tool (`akidev-manga-cli`) that can search, list chapters and dump page URLs for a definition. Nothing visual yet. |
| **2. FFI and shell** | 2 | uniffi facade, xcframework build script, a Swift package, an Xcode app with Browse → Series → Reader working on MangaDex, plain styling. |
| **3. Reader and library** | 4 | The reader (long strip first, then paged), image pipeline, library, history, progress, downloads, categories, Liquid Glass chrome and Aki design module. |
| **4. Sources** | 3 | Source lists, install, update, validation UI, theme layer, 10 to 15 definitions across 3 themes, server clients (Komga, OPDS). WASM runtime and ABI v1 can land here or in 6 depending on spike 4. |
| **5. Polish and release** | 2 | Updates and background refresh, trackers (AniList), backup, accessibility pass, design review against `DESIGN_PLAN.md`, TestFlight. |
| 6. Later | | WASM authoring kit and book, MAL, sync, macOS target, Paperback/Aidoku backup import, optional JS layer. |

About **15 weeks** to a TestFlight build for one engineer who knows both Rust and SwiftUI; the reader and the
source corpus are where the time goes, not the shell.

### Spikes (phase 0)

1. **Core on iOS.** Add a `streaming` feature to `akidev-core` and build it without `librqbit`, `axum` and
   native-tls for `aarch64-apple-ios` and `aarch64-apple-ios-sim` with rustls. Find anything that pulls a C
   build (`aws-lc-sys`). Pass/fail: it compiles and a `search` call returns on a simulator.
2. **uniffi hello.** One async call (`popular` on MangaDex) from SwiftUI, with a cancel handle.
3. **Source corpus census.** Take the 60 most-used sites from Mihon's repo and Aidoku's lists and classify each:
   declarative / theme / needs code / needs a challenge solver / scrambled images. This sets how much weight
   WASM carries and which ops matter. Do this by reading definitions, not by contacting the sites.
4. **WASM host.** `wasmi` on iOS: cold start, memory, call overhead on a 500 KB HTML parse; compare with
   parsing in the host. Decide whether v1 includes it.
5. ~~Challenge hook (a `WKWebView` that clears a Cloudflare interstitial).~~ **Dropped: the app is fully native,
   no web view** (section 15). Replaced by a native-fingerprint spike, below.
6. **Reader perf.** A 40-image, 800×12,000 px strip in SwiftUI `ScrollView`+`LazyVStack` vs a
   `UICollectionView`/UIKit-backed reader via `UIViewRepresentable`; measure memory and dropped frames on a
   real device. My prior: SwiftUI alone will not hold 60 fps for long strips, so plan the reader as UIKit
   inside SwiftUI and keep the chrome SwiftUI (**unverified**, this spike decides).
7. **Liquid Glass over content.** Check legibility of glass controls over white manga pages and over dark
   panels, and the cost on an older device (iPhone 12 class).

## 10. Decisions I need from you

1. **Minimum OS:** iOS 26 only (Liquid Glass native, smallest code) or iOS 18+ with a material fallback?
   My pick: 26 only for v1.
2. **Which curated sources** go in the Akidev list, given section 8: MangaDex plus official/free sources only,
   and nothing else from us. Agree?
3. **Name and identity:** a separate app name and bundle id (not "Akidev") so that a manga app and the
   streaming app cannot take each other down in a store review. Name?
4. **Repository layout:** `manga-app/` at the repo root plus `desktop/manga*` crates, or a separate repository
   with `akidev-core` as a git dependency?
5. **WASM in v1:** include it (more sources, more work) or declarative + themes only first (my pick, then add
   WASM when the census shows which sites need it).
6. **macOS and iPad in v1?** iPad yes (reader benefits most), macOS later (my pick).
7. **Release channel first:** TestFlight and sideload (my pick) vs trying the App Store immediately.

## 11. Spike results (phase 0, run 2026-10-21)

Decisions taken: iOS 26 only, TestFlight and sideload first, declarative sources plus themes, no WASM in v1
(spike 4 dropped from phase 0).

| Spike | Result |
| --- | --- |
| **1. Core on iOS** | **Pass.** `librqbit` does not compile for iOS (`librqbit-dualstack-sockets` calls `Socket::bind_device`, which does not exist there), as feared. Fix made: `akidev-core` now has a default-on `streaming` feature holding `librqbit`, `axum` and `tokio-util` and the `torrent`, `serve` and `stream` modules. With `--no-default-features` the library compiles for `aarch64-apple-ios` and `aarch64-apple-ios-sim`, 246 tests pass (265 with the feature), and the desktop workspace still checks. The iOS tree has **no `aws-lc-sys` and no OpenSSL**: `native-tls` uses Security.framework on Apple, so the rustls switch is not needed for iOS (still needed for Android). `getrandom` needed its `std` feature set explicitly. |
| **2. uniffi hello** | **Pass on macOS, builds for iOS.** New crate `desktop/manga-ffi` (`akidev-manga-ffi`, lib name `akimanga`, uniffi 0.32 proc-macros, tokio). One `async popular(limit)` call returns typed records and a typed error to Swift: generated bindings, a Swift program with top-level `await` printed titles from a local fake of the MangaDex response in 6 ms. Static libs build for both iOS targets (about 45 MB each before the linker strips) and `xcodebuild -create-xcframework` accepts them. **Not tested:** a call on an iOS simulator or device, and cancellation (uniffi has none; a cancel handle is still to be designed). |
| **MangaDex reachable?** | **Not from this machine.** `api.mangadex.org` answers `503` with an HTML login page (looks like a network filter, not MangaDex). Everything that touches it needs a different network; the engine takes `AKI_MANGADEX_BASE` for tests. |
| **3. Source census** | **Done, and it corrects the plan: see 11.1.** |
| **5. Challenge hook** | **Dropped, see section 15 (no web view).** What was run: mechanism passed, real Cloudflare untested.** A hidden-app `WKWebView` loaded a local page that sets a `cf_clearance` cookie after 2 s; the host read it from `WKHTTPCookieStore` after 3.8 s and read `navigator.userAgent`, so the cookie-plus-UA handoff to the Rust client is possible. **Not shown:** that a real Cloudflare interstitial clears in a `WKWebView` (this network filters outside sites and the simulator is not a home connection), and the pass criterion (3 of 5 protected sites) is still open. Note the UA the webview reports (`iPhone OS 18_7`) must be the one the Rust client sends, or the clearance cookie is rejected. |
| **6. Reader perf** | **Inconclusive: simulator only.** 40 JPEG pages of 1000×10000 px, programmatic scroll at 2,500 pt/s for 25 s, iPhone 17 simulator (iOS 27.1). `UICollectionView` reader: 0 frames over 25 ms, p99 16.7 ms, peak footprint 361 MB (247 MB at the end). SwiftUI `LazyVStack`, views kept: 0 hitches, peak 127 MB; with the image dropped on `onDisappear`: 2 hitches (max 55 ms), peak 125 MB. A screenshot at 12 s shows SwiftUI rendering the right pages while decode catches up (a blurred placeholder at the top). **Nothing separates them on a Mac GPU**, and the SwiftUI lazy stack did not hoard memory as I had feared, so my prior that the reader must be UIKit is **not confirmed**. The memory numbers are not comparable either: both paths decode a page at 1179×11790 px (about 55 MB), a size to avoid in the real reader (tile or cap it). Real answer needs an iPhone, a long run and Instruments. Default: SwiftUI reader, keep the UIKit one as the fallback. |
| **7. Glass legibility** | **Pass, with one design note.** `GlassEffectContainer` with `.glass`, `.glassProminent`, an interactive circle and a capsule is legible over white, grey and black strips and over the dark canvas. On the dark canvas the pills render as **light grey**, which is bright against the warm-ink look in `design/DESIGN_PLAN.md`; the reader chrome needs a tint or a forced dark scheme. Two oddities seen on the dark screen: the container drew two dark arched shapes behind the controls when glass morphing grouped them (needs a look with a spacing value that does not merge them). Cost on an older device is not measured. |

### 11.1 Source census (read from the extension repos only, no site was contacted)

Method: counted `theme = "..."` in each extension's `build.gradle.kts` in keiyoushi/extensions-source (main,
2026-10-20), and read the Kotlin of 59 popular English sources for markers (`Cipher`, `Bitmap`, `WebView`,
`QuickJs`, `POST`). The popularity ranking is the agent's own pick (no install counts found), and the marker
grep over-matches, so the split is approximate. Paperback's repo was unreachable; inkdex/general-extensions was
read by directory listing only.

- **Themes:** 1,444 extensions, 728 themed (about 50%), 69 theme libraries. English: 418 extensions, 236
  themed. English leaders: **madara 102, mangathemesia 26, keyoapp 17, mangacatalog 13, mangahub 11, yiimanga
  10, iken 10, vinetheme 7**. Madara + mangathemesia are 54% of the English themed ones. Aidoku's repo: 138
  sources, about 70 on a template (mangathemesia 15, madara 11, wpcomics 8, liliana 6, iken 5, mangabox 4).
- **59 popular sources:** about **34 declarative or theme-only**, 8 need login (JSON APIs behind accounts or
  purchase), 6 scramble or encrypt images, 6 need other custom code, 1 needs an anti-bot captcha solved in a
  WebView. So **about 60% declarative, not 80%**, and the big names are disproportionately the hard ones
  (MangaPlus protobuf+XOR, Mangago AES+QuickJS, MangaHere packed JS, AllAnime GraphQL+captcha, MangaFire
  captcha, Asura/Comix/Philia tile descrambling).
- **Operations needed beyond selectors, by frequency:** (1) date parsing, about 48%, with per-site formats and
  "x days ago"; (2) regex and string slicing, about 70%; (3) JSON, about 63%, often inside a `<script>` blob
  (Next.js `__NEXT_DATA__`, mangathemesia `ts_reader.run({...})`); (4) POST and ajax, about 35% (madara
  `admin-ajax.php` and `/ajax/chapters/` for search and chapter lists); (5) per-source preferences, about 50%;
  (6) cookies or WebView login, about 33%; (7) image interceptors, about 10%, tile descrambling the commonest;
  (8) WebView or Cloudflare solving, about 24%; (9) real crypto, under 10%; (10) protobuf, GraphQL, 2 to 3.
- **Madara catch:** some sites wrap chapter lists in an AES "chapter protector" blob; MangaHub sends an
  `x-mhub-access` key and AES-decrypts a field. A theme can still be declarative if the engine has an `aes`
  op, but that needs a decision on shipping a small crypto op set.

What changes in the plan:
1. **Themes matter more than assumed and decide v1.** Build the `madara` and `mangathemesia` themes first
   (54% of English themed extensions), then keyoapp, mangacatalog, mangahub, iken.
2. **The four extras are v1, not later:** a script-blob extractor (`json_in_html`), ajax POST requests, date
   formats including relative ones, and tile descrambling as a closed op set.
3. ~~**Add to v1 if cheap:** `aes` decrypt and `xor`, since they unlock the madara protector and some image
   scrambles without WASM.~~ **`aes` decrypt shipped 2026-10-22** as the `aes_cbc` step (`desktop/manga/src/crypt.rs`);
   `xor` stays out until a source needs it.
4. **Expect to leave out in v1** (needs code or a captcha): MangaPlus, Mangago, MangaHere, AllAnime, MangaFire,
   Asura-class tile permutations with signatures, and every paid or login-gated API. Say so in the source list
   instead of faking support.
5. **The "WASM later" decision holds** but the ceiling is about 60 to 70% of the popular names until it lands.
6. "MangaDex is the only official API" is wrong: MangaPlus, Webtoons, Tappytoon, Toomics, Comikey, Manta,
   Kodansha and Bookwalker have first-party APIs, mostly behind login or purchase. A future native adapter for
   the free ones (Webtoons, MangaPlus) is a legal route worth costing; both need work (MangaPlus is protobuf
   with an XOR image step).

## 12. Phase 1 status (engine), 2026-10-21

Built in `desktop/manga/` (`akidev-manga`, a workspace member that depends on `akidev-core` without the
`streaming` feature; builds for both iOS targets):

- **Definition format and engine** (`def.rs`, `eval.rs`, `source.rs`): the six operations (popular, latest,
  search, details, chapters, pages) over HTML (the `scraper` crate) or JSON, with the expression steps from 5.3
  that census says matter first: `css`, `attr` with fallbacks, `regex`, `json` for script blobs, `date` (patterns,
  `iso`, `relative`), `number`, `coalesce`, `abs`, and request holes including POST `form` and `json` bodies.
  A request can only start with `{base}`, so a definition cannot send anything anywhere else; mirrors are tried
  when a host cannot be reached.
- **Themes** (`theme.rs`): `extends` with key-by-key merging, circles refused. One theme so far, `madara`,
  and one site on it.
- **Fetching** (`fetch.rs`): per-source rate limit, cookies and user agent that a challenge can set, a typed
  `Challenge` error for the app to act on (Cloudflare and captcha pages), a recorded transport for tests.
- **Fixtures and CLI** (`check.rs`, `bin/cli.rs`): `check` runs a definition against recorded pages and
  compares with expected answers (`--bless` writes them), `run` asks a live site. `tests/sources.rs` runs every
  definition's fixtures in `cargo test`.
- **MangaDex** (`mangadex.rs`): native client with an honest user agent, a 4 requests per second limiter, no
  `Via`, chapters without external ones, pages from the chapter's at-home server. **Tested against hand-written
  answers only: MangaDex is unreachable from this network and its docs did not load, so the request shapes are
  from memory and need a run against the real API.**
- **Store** (`store.rs`): SQLite schema v1 (series, chapter, progress, category, download, link), library with
  unread counts, new-chapter detection on sync (none on first sync), continue-reading, history.

Tests: 28 unit tests and one integration test pass; clippy is clean.

**Not verified:** any real site. The `madara` theme and its fixtures are written from memory of Madara's
markup and are synthetic (see `manga-sources/README.md`); the engine is proven against that shape,
not against a live Madara site. Phase 1 exit criterion "a CLI that can search, list chapters and dump page URLs
for a definition" is met for recorded pages; against a live site it is untried.

**Still to do in phase 1:** a second theme (`mangathemesia`, the next biggest), filters end to end, an `xor` step
and tile descrambling (both waiting on a source that needs them: the one tile source, AsuraScans, stopped sending
permutations — see `MANGA_SOURCES_PORT_PLAN.md` §Phase 4), a nonce step for ajax chapter lists, and a live run of
everything once a network that reaches the sites is available. **Done since:** `aes_cbc` decryption. Next phase (2): uniffi facade over this crate and the store, and
the Swift shell.

## 13. Phase 2 status (facade and Swift shell), 2026-10-21

- **Facade** (`desktop/manga-ffi`, lib `akimanga`): one `Engine` object over the sources, the SQLite store and the
  settings a challenge hands over. Installs themes and sources (kept in its folder across launches, a source
  that no longer loads is skipped), asks a source (popular, latest, search, details, chapters, pages), keeps
  chapters and progress (`chapters` syncs the store and names new chapters), library, resume, history, and
  `set_cookies`/`set_user_agent` for the challenge hook. Errors reach Swift as `Challenge {url, status}` or
  `Failed {message}`. `tests/engine.rs` runs the whole path against a local server answering with the madara
  fixtures, including a challenge answer and reopening the folder in a new engine.
- **Swift package and shell** (`manga-app/`): `scripts/build-ffi.sh` builds both iOS targets, generates the
  bindings from the library's own metadata and makes the xcframework. The app has Library (grid with unread
  badges), Browse (source picker, popular/latest, search, show more, state cards for empty and error),
  Series (cover, authors, tags, description, Continue/Start, chapter list with read state and dates, save),
  the long-strip Reader (images loaded with the source's headers and downsampled to the screen width, glass
  top and bottom bars that hide on tap, page counter, next and previous chapter, an end-of-chapter card,
  progress saved as the person scrolls and a chapter finished when its end card is reached) and Sources (list,
  add by address, remove). Palette and radii are the Akidev ones; titles are a serif.
- **Driven in the simulator** (iPhone 17, iOS 27.1) against `scripts/demo-server.py`: Browse listed titles and
  covers read through the engine, the Series page showed details and the chapters with correct dates and
  numbers, the reader opened a chapter and scrolled, progress and the library were written to SQLite, and after
  leaving the reader the series page showed the finished chapter and "Continue" at the next one. A page the
  server did not have showed the error card and Try again recovered.
- **Not done or not verified:** a real device; a real site (the demo server serves the same synthetic
  fixtures); MangaDex through the app (unreachable here); clearing a challenge (the app says a site wants a check and offers the native ways round it, section 15); paged and right-to-left reading; zoom; prefetching the next chapter and the
  seamless join; saving the position inside a page; downloads; the library search, categories and the updates
  tab; Dynamic Type and VoiceOver passes; the UIKit reader fallback (spike 6 left that open). The Rust image
  pipeline is not used: images load through `URLSession` in Swift.

## 14. Phase 3 status (the reader), 2026-10-21

Built in `manga-app/App` (`ReaderModel.swift`, `ReaderView.swift`, `ZoomableImage.swift`):

- **Three reading modes**, chosen per series from a menu in the reader and remembered: long strip, pages left to
  right, pages right to left (a series' `reading` hint picks the first one). Switching keeps the chapter and page.
  A page much taller than the screen is fitted to the width and scrolled in page mode.
- **Pinch zoom.** Pages: a `UIScrollView` per page (pinch to 4×, double tap to zoom on a point, pan when zoomed,
  a single tap turns the page at the edges or toggles the controls; the edge that goes on follows the reading
  direction). Strip: pinch scales the strip's width up to 3× and snaps on release, with horizontal drag when zoomed.
- **Seamless chapters** in the strip: the next chapter's pages are appended under a thin "end of chapter" row when
  the reader is within three pages of the end, the title and counter follow whichever chapter is at the top,
  and reading into the next chapter finishes the one before.
- **Prefetch:** as soon as a chapter opens, the next one's page list is fetched and its first three images are
  loaded, and the three images after the current page are warmed as the person reads. The "next chapter" button
  then opens from the cached page list with no request for it.
- **Position inside a page:** the strip reports the page at the top and how far down it (0 to 1 of its height),
  saved 350 ms after scrolling stops and on leaving; reopening scrolls to the page, waits for its shape to be
  known and then to the saved offset. Saving is held back while the strip is being put back so its own layout does
  not overwrite the position.

Driven in the simulator (iPhone 17, iOS 27.1) against `manga-app/scripts/demo-server.py`, whose pages are now
labelled with chapter, page and distance down so a screenshot shows what is on screen:
- reopening a chapter at page 3, 0.678 down put the screen where it was (same band at the top) and kept the saved
  value; the counter showed 3 / 6;
- reaching the end of chapter 178 joined 178.5 below it, the title changed to 178.5, 178 was marked finished and
  178.5's offset was recorded;
- right-to-left: opened at the saved page, a tap on the left edge went on (saved page 2), a right swipe went on
  (page 3); left-to-right: a tap on the right edge went on;
- pinch zoomed a page to about 4× and the strip to about 3×;
- the server log showed chapter 179's page list and first three images requested right after 178.5 opened, and
  the next-chapter button then made no request for the page list.

**Bugs found on the way, fixed:** strip rows had to be one `ForEach` of string ids (a nested `ForEach` with `.id`
modifiers made `scrollTo(id:)` do nothing); `scrollTargetLayout` is needed for it; the first laid-out rows
overwrote the saved position before it was restored.

**Not verified:** a real device or finger (all input here is synthetic, so pinch feel, momentum and zoomed-strip
panning are untested); long chapters (the strip estimates the height of unrealised rows, so restoring to a page
far down is only as exact as that estimate); the position inside a page in paged mode (page only); zoom level is
not saved; Dynamic Type and VoiceOver; low-memory behaviour with many chapters appended (the strip never drops
earlier chapters); the old `sizes` problem of decoding a full-width image per row stays: images are decoded at
the zoomed width, not tiled.

## 15. Decision: no web view, fully native (2026-10-21)

The app has no `WKWebView`, no `SFSafariViewController` and no embedded browser of any kind: everything on screen is
SwiftUI, everything that fetches is the Rust engine. (The app already was this; the only web view was the
challenge hook planned in 5.3 and tried in spike 5, now removed from the plan.)

**What that costs, said plainly.** A site behind Cloudflare's JavaScript challenge or a captcha cannot be passed by
a native HTTP client; that is what the challenge is for. The census counted about 24% of popular sources as
needing a web view or a solver. Those sources are **not supported** by Akidev, and the app says so instead of
showing an empty list. MangaDex (no challenge), plain sites, JSON APIs and every themed site that is not behind a
check are unaffected.

**The native ways round it, in order of preference:**
1. **A server the person already runs.** Komga, Kavita and Suwayomi adapters (planned, section 7) read sources
   that do their own bypass (Suwayomi supports FlareSolverr). The person adds the server's address; the app reads
   OPDS or its REST API. Nothing in the app loads a page.
2. **A lighter check can be met natively.** Many sites only turn away clients whose TLS handshake or header
   order does not look like a browser's (no JavaScript challenge). The engine can look more like Safari there:
   a browser user agent (done), header order and a Safari-like TLS fingerprint (not done: a spike, below).
   This does not pass JavaScript challenges or captchas, and a site may block it again tomorrow.
3. **Cookies the person already has.** `Engine.set_cookies` and `set_user_agent` stay in the facade (a source
   may need a login cookie), but nothing in the app collects them from a browser. A text field for pasting a
   `cf_clearance` cookie and its user agent is possible and is native; whether to offer it is open, since
   the clearance is tied to the IP and the browser's fingerprint and expires within hours.

**Spike 5 (new): native TLS fingerprint.** Find out whether a Safari-like handshake (BoringSSL based crates such as
`wreq`, or `reqwest` with `rustls` and a hand-built ClientHello) builds for iOS, and whether it gets 3 of 5
fingerprint-only protected sites to answer. Pass/fail is that count; JavaScript-challenged sites are expected to
stay out. Needs a network that reaches the sites, so it cannot be run from this machine.

**In the app now:** a `Challenge` error reaches the screen as the state card "This site checks visitors first"
with what to do instead. The facade and engine comments were corrected to match.

## 16. Decision: the app is a renderer, every source is a plugin (2026-10-21)

Akidev has **no built-in source and no default repository**. Everything it reads comes from a definition the
person installed, from a repository they added by address (a GitHub link, or any https folder with an
`index.json`). The hardcoded MangaDex client (`mangadex.rs`) and the `Catalogue` enum are gone; MangaDex is
`manga-sources/sources/mangadex.json`, published like any other plugin. Sections 7 and 8 above describe the plan
before this decision where they say the app ships with MangaDex.

**What was built** (all committed, with tests):
- **Format additions, driven by MangaDex** (documented in `manga-sources/README.md`): `vars` and `prepare`
  (chained requests whose answers become holes), `chapters.repeat` (a paged chapter list), `total` and `page_size` with
  `{offset}` for lists, `where` and `unique` steps (a related item of one type, a row that is not external),
  `{^.path}` in templates (a value in the row), `settings` with holes (language, content rating),
  `user_agent`, and `image_headers` that can remove the default referer. MangaDex's definition reads covers from
  relationships, authors without duplicates, skips external and empty chapters, pages its chapter feed and builds page
  addresses from the image server its answer names.
- **Repository format** (`desktop/manga/src/repo.rs`): `index.json` lists sources and themes with version, SHA-256, languages,
  `min_app` and `requires` (themes installed with the source). Paths cannot leave the repository, hashes are checked on
  every file, an optional ed25519 key and `index.json.sig` sign the index, and a signed repository's key is pinned the
  first time (a different key later is refused until the person removes and re-adds it). GitHub addresses
  (`github.com/o/r`, `/tree/branch/folder`, raw links) resolve to the raw folder; plain http only for this machine.
  `akidev-manga-cli index` writes the index and `keygen` makes a key; a test fails when `index.json` is stale.
- **Facade and app:** `add_repo`, `repo_sources`, `install_from_repo` (themes first, then the source, then check that the
  file is the source the index lists), `update_sources` (run at start), `remove_repo`, `source_settings` and
  `set_setting` (saved, put back at launch). The app: Sources tab with Installed and Repositories, a repository page with
  Install and Update, a "signed or not" note, a settings page per source, and a "No sources yet" state on Browse that
  leads to Sources. SQLite schema v2 adds `repo`, `installed` and `setting`.

**Run:** 35 unit tests, 4 repository tests over HTTP (hash mismatch, tampered index, wrong key, missing signature) and 2
facade tests (add, install, browse, read, update, refused file, reopen; key swap) pass. In the simulator: a fresh app
showed "No sources yet", an unreachable address showed a clear error, adding the demo repository by typing its address
listed its source, Install wrote the theme and source and made Browse show titles, and installing MangaDex from the real
`manga-sources` index (hash-checked) showed its language and content settings.

**Not verified:** MangaDex itself (its definition is tested against hand-written answers; the site is unreachable from
this network, so request shapes, query brackets and the image server flow are unproven), GitHub as the host (only a
local server was used), a signed repository in the app (tested in Rust only), a theme update reloading installed
sources (tested only in the facade's first install), and settings changing a live Browse (the screen reloads, the
effect on MangaDex requests is only unit tested).

**Costs and open points:** an app that opens empty needs a first-run screen and, for App Review, something to do
without a repository (local CBZ import is planned); a plugin decides which hosts the app contacts, so the repository warning
matters, and images load from any host a plugin names; there is still no scripting layer, so about 40% of popular sites
stay out (section 11.1) unless one is added later.

## 17. More sources: research, what was published, what was tried (2026-10-21)

**Rule for our repository:** only sources that are legal to read: a public API that welcomes third-party readers, or work released
under an open licence. Other sources are other people's repositories (section 8).

**Candidates checked** (reachability from this network, then whether they fit):

| Candidate | Result |
| --- | --- |
| xkcd | JSON endpoints (`/info.0.json`, `/{n}/info.0.json`) and an archive page; CC BY-NC 2.5. **Published.** |
| Pepper&Carrot | An official episode index (`0_sources/episodes-v1.json`, documented in its API page) and images at `0_sources/{episode}/{low-res or hi-res}/{lang}_{file}`; CC BY 4.0, 39 episodes, 70 languages. **Published.** |
| Internet Archive | `advancedsearch.php`, `metadata/{id}`, a book's `*_scandata.xml` (leaf numbers) and page images at `download/{id}/page/n{N}_w{W}.jpg`, covers at `services/img/{id}`. Searched with `licenseurl:*publicdomain*` and a scandata file so every result can be read (839 items). **Published.** The licence mark is the uploader's claim, and the plain collections (Manga Library, comics inbox) are full of copyrighted uploads, so they are not used. |
| MangaDex | Already published; unreachable from this network (503), so only hand-written fixtures. |
| Webtoons, Viz, Tapas | Official free-to-read sites with no third-party API; their terms restrict automated access. **Not published** (another repository's call). Webtoons and Viz answer from here; Tapas does not. |
| MangaPlus | Protobuf and an XOR image step: not expressible in a definition. |
| Digital Comic Museum, Comic Book Plus | Public-domain comic sites; answer 403 and 406 to a script from here. A candidate for later with another user agent. |
| AniList, Jikan | Metadata only, no pages. |
| Komga, Kavita, Suwayomi | Self-hosted: the address and login are the person's. **Needs the format to take a server address and credentials as settings** (and cover requests that carry them): the next format addition, and the most useful one for people with their own library. |

**What the engine gained for these:** the CLI's `record` command (asks the live site for named calls and saves the answers
as fixtures, then writes the expected answers), `sort` (natural order for file names), `plain_text` (markup in a JSON
field), `where` values with holes (`"equals": "{id}"`), and `{base}` as a hole in expressions.

**Result against the live sites:** from the Mac (the CLI), all three read real data on the first run: xkcd listed its 3,313
comics with dates and gave the image of #614; Pepper&Carrot listed 39 episodes titled and numbered correctly with the
cover and pages in order; the Internet Archive search, the details, the chapter and the 421 page addresses of *Little Nemo*
came from the real scandata. Their fixtures in `manga-sources/fixtures` are the real answers (xkcd's archive trimmed to
the newest 60). The repository was published (`d09d350` in `dishant0406/manga-sources`) and the app, with a fresh
install, added it by `github.com/dishant0406/manga-sources` and installed all three, hash-checked.

**Not seen in the app:** these sources' content on screen. In the simulator every request to these sites fails with "the
certificate was not trusted": this network decrypts TLS (the certificates are issued by `Groww-Decryption-*`, a company
CA that macOS trusts and the simulator does not), except for GitHub. MangaDex, Tapas and others are also blocked by the same
filter. The app itself is not at fault; it needs a network without that, a device, or the company CA added to the
simulator's trust (a security change that was not made).

**Weaknesses found:** xkcd has no search (so no search box result), a single-series source shows one card and says there is
no next page via a constant `total`; Pepper&Carrot translations are per episode and a language an old episode lacks gives
broken page images; an Internet Archive page is a full-size scan (about 1 MB each at width 1100).

## 18. The look: a reference chosen by the owner (2026-10-22)

The owner shared a screenshot of a webtoon-style reader and asked for the app to look like it, "not like Paperback", with
Liquid Glass used as much as possible. The shell was redone to match (`manga-app/DESIGN.md` has the analysis and the rules):
a bright rounded design with one yellow, soft grey chapter and update cards, and glass for all chrome.

**Built:** a design system (adaptive light and dark colours, rounded type, glass buttons, yellow-tinted pills that slide,
chips, state cards); a floating glass tab bar (Home, Library, Updates, Sources) whose current tab is a yellow glass circle
that flows to the next, with an unread badge, hidden on pushed screens; **Home** (a glass search field, source chips,
Popular/Latest pills, a large featured card and a two-column grid); the **series page** as in the reference (full-width
picture that stretches when pulled, glass back/save/more, chips and a white title on a gradient, a sheet with rounded
top corners, Chapters/Details pills, a grey card per chapter with thumbnail, small label, bold title, date and scanlator
chips and a read toggle, a floating yellow "Continue" button; Details has the description, an info card and tag chips);
**Library** (Saved grid with unread badges, History cards); **Updates** (Unread/Recent pills, cards for series with new
chapters, a refresh that asks every source); **Sources** in the same cards; the reader keeps its black glass chrome with the
yellow action. History now carries each series' title and cover (store join, facade, tests).

**Checked in the simulator** (iPhone 17, light): every screen above rendered as described, the glass tab bar switched
tabs with the yellow circle following, a saved bookmark merged into its neighbour as glass does, and the reader opened in
paged mode with the new chrome. Dark appearance follows the system and was seen once, on Home.

**Where it departs from the reference, and why:** no per-episode artwork (a source gives one cover, so chapter rows reuse the
series cover as their thumbnail); no view counts (sources do not give them), so chips show dates and scanlators; the
download arrow became a read toggle because downloads do not exist yet; the reference's black "home" circle on the tab bar
was read as a pressed state and not copied. **Not done:** a real device, dark-mode review of every screen, Dynamic Type,
accessibility labels beyond the tab bar, and the Details tab and Updates refresh against a live source.

## 19. User flow (2026-10-22)

Researched the flows of the plugin readers (Mihon and forks, Aidoku, Paperback, Kotatsu, Suwayomi, Cubari), the official webtoon
platforms, the publisher manga apps and the self-hosted readers, and wrote the screen map, the flows and the build order in
`manga-app/USER_FLOW.md`. Evidence is uneven (many official help pages did not load; unverified items are marked in the
document). Main findings for us: the first minute (empty app, add a repository, install a source) is the most important flow
and has no Welcome screen or one-tap add link yet; app settings, a reader settings sheet and an error vocabulary are table
stakes; global search, filters, categories and backup are the next tier.

## Sources

- [Paperback sources intro (extensions-common wiki)](https://github.com/Paperback-iOS/extensions-common/wiki/Intro-to-Paperback-Sources) and [function definitions](https://github.com/Paperback-iOS/extensions-common/wiki/Function-Definitions)
- [Paperback on GitHub](https://github.com/Paperback-iOS), [App Store listing](https://apps.apple.com/app/id1626613373)
- [Aidoku](https://github.com/Aidoku/Aidoku), [aidoku-rs source SDK](https://github.com/Aidoku/aidoku-rs), [source development book](https://aidoku.github.io/aidoku-rs/book/)
- [Suwayomi-Server](https://github.com/Suwayomi/Suwayomi-Server), [Keiyoushi extensions source](https://docsearch.algolia.com/mcp/docs/repo/keiyoushi/extensions-source)
- [MangaDex API limitations](https://api.mangadex.org/docs/2-limitations)
- [Apple: GlassEffectContainer](https://developer.apple.com/documentation/swiftui/glasseffectcontainer.md)
- [Ferrostar: Rust to Swift with uniffi and XCFramework](https://stadiamaps.com/blog/ferrostar-building-a-cross-platform-navigation-sdk-in-rust-part-2/)
- In-repo: `desktop/core/src/definition.rs`, `desktop/sources/README.md`, `pipeline/MOBILE_PLAN.md`, `design/DESIGN_PLAN.md`
