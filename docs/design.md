# Design decisions

Each entry says what we chose, why, what we turned down, and what it costs. New entries go at the bottom.

## The skins only draw (2026-11-21)

**Chosen.** The desktop app and the website do not hold lists of tabs, sorts, genres or home rows. They read a
`screens` section of the signed sources document (`sources.json`, now in the akidev-sources repo). Core reads it
(`core/src/screens.rs`, `sources::screens()`). The server sends it at `GET /v1/screens`. The desktop app calls
core directly. The website asks the server.

**Why.** There were four copies of the same lists (desktop home, server home, desktop browse, website
constants). A change meant four edits and a release of each. Now it is one edit and a publish.

**Turned down.** Calling the server from the desktop app. The desktop works offline and has no extra hop, and
the app is still thin: it only reads data.

**Costs.** A new field means an old app ignores it, but a new app reading an old live document must still work.
So `screens()` falls back to the copy built into the app, and `image_hosts()` adds the built-in hosts to the
live ones.

## One built-in copy beats a "required" flag (2026-11-21)

**Chosen.** When the live document lacks something newer (screens, image hosts), code falls back to the
embedded document. When it lacks something the code now needs from the JSON (`empty_when` for YTS, `headers`
for Prowlarr), we publish the document first and release second.

**Why.** A "this document needs feature X" marker is more machinery than the problem is worth. The two things
that need ordering degrade softly: a YTS empty answer counts as a failed request (not cached as empty, other
indexers carry on), and the Prowlarr fallback answers 400 (it only runs when others found nothing).

**Turned down.** Keeping the old `if def.id == "yts"` branch "just in case". That is the clutter we removed.

## An old-app gate before publishing (2026-11-21)

**Chosen.** `content/sources/check-old-apps.sh` builds the checker from an old release tag and runs it on a
document. `publish-sources.sh` runs it before signing.

**Why.** An installed app ignores unknown fields but refuses the whole document for a value it does not know
(a new hole name, a new kind, another `schema`). A refused document also stops tracker updates and
`min_version`. This is the one failure that strands every user, so it is checked by machine.

**Costs.** It builds an old tree (about a minute the first time). The default tag is `desktop-mac-v0.1.17`;
set `OLD_REF` to the oldest release you still support. Today the repo copy of `sources.json` is refused by
v0.1.24 because of the unreleased Kitsu holes, and that is the gate working.

## Image hosts come from the providers (2026-11-21)

**Chosen.** Each provider lists `image_hosts`. The image proxy allows those, the built-in ones, and anything
in the `IMAGE_HOSTS` setting.

**Why.** A new catalogue's posters were refused until someone edited an env var and redeployed.

**Turned down.** Deriving hosts from `base_url`. Four of the eight old hosts (`kitsu.app`, `metahub.space`,
`media-amazon.com`, `tmdb.org`) are not the base address of any provider.

## Plugins: unsigned, desktop only (2026-11-21)

**Chosen.** A plugin is one JSON file (`core/src/plugin.rs`). It adds indexers, subtitle sources and
catalogues (providers, with home rows) after the built-in ones. Every address must be a public https address. It can add, never replace. The desktop loads it
from `<data folder>/plugins`. The server never does.

**Why.** "Anyone can add their own torrent list" is the part that works fully today. A shared server must not
call hosts a stranger chose, so the website only uses the signed list.

**Turned down.**
- Updating a plugin from its link in the background. A refresh could quietly add new hosts. Adding the link
  again is the update.
- A file picker. A drop-in folder plus Reload is less code and works the same.

**Also.** The shared HTTP client (`core/src/http.rs`) does not follow a redirect to a private address, so a
plugin's host cannot bounce a request to the viewer's own machine or network.

**Costs.** A public name that resolves to a private address (`127.0.0.1.nip.io`) is not caught. The plugin is
not checked for what it finds, only for where it talks. Settings says so. Installing a plugin that would stop
another one working is refused, and the old copy is put back.

## Kinds are small ids with a shape (2026-11-21)

**Chosen.** `Kind` (`core/src/model.rs`) is a copyable id, not an enum. The three built-in kinds are constants
(`Kind::Anime`, `Kind::Series`, `Kind::Movie`), so most of the code did not change. A plugin can bring more
(`Kind::register`). A kind has a `Shape` (anime, series, movie) that says how its keys, episodes and playback
work, and code that used to match on the kind matches on `kind.shape()`. Saved data that names a kind nobody
has declared (a plugin that was removed) still reads, as a series.

**Why.** A new kind was impossible without Rust changes, and about 60 places named the three. Now the apps ask
the kind what it is like.

**Turned down.** A `kinds` section in the signed document. An old app refuses a document with a kind it does not
know, so it would have to be in the schema 2 feed (`sources-v2.json` (in the akidev-sources repo), see "Two feeds, one version"). Nothing
needs it yet: plugins load only in new builds.

**Costs.** The registry is global and keeps its names for the life of the program (at most 32, each 1 to 16
letters). A kind a plugin declared cannot be declared again with another shape until the app restarts. The
website still knows only the three kinds, because the server never loads plugins.

## Id mappers replace the Kitsu flag (2026-11-21)

**Chosen.** The signed document has `mappers` (the Kitsu definition moved in from its own file). An indexer says
`"map": "kitsu"`. The holes of its request are named after the mapper (`{kitsu}`, `{kitsu_kind}`) and can be
anywhere in the request. The engine checks the mapped id has the exact shape `<mapper>:<title>:<episode>`.

**Why.** The Kitsu bridge was code that only worked with `{kitsu}` in the path, assumed AniList, and could not
be changed without a release.

**Turned down.** New hole names (`{mapped}`): an old app refuses a document with a hole it does not know. The
hole names follow the mapper's id instead, so for `kitsu` they are the ones that were already there, and the same
document works for old and new apps. The old `anime_kitsu: true` stays in the signed document for old apps.

**Costs.** One mapper is used for one search. A document from before `mappers` gets the built-in one.

## A request stays on the host it names (2026-11-21)

**Chosen.** An indexer's or a subtitle source's URL is its host with the request's path added after it. Right
before it is sent, `http::stays_on` checks the finished URL still has the same scheme, host and port. The shared
HTTP client also refuses to follow a redirect to a private address.

**Why.** A plugin's host is checked when the plugin is read, but a path like `@127.0.0.1:8443/x` would turn
`https://torrents.example.com@127.0.0.1:8443/x` into a request to this machine. An independent review found it.

**Costs.** One URL parse per request. A public name that points at a private address is still not caught.

## Two feeds, one version (2026-11-21)

**Chosen.** The signed document is published twice with one version number: schema 1 at the root of the feed (what
every installed app reads) and schema 2 under `/v2` (`sources::refresh_from` tries it first, and falls back to
the root when `/v2` is missing, forged or in a format it cannot read). `SCHEMA` is 2, and a build reads 1 and 2.
Today the two documents are the same apart from the number. The publish script derives `/v2` from the root document,
or uses `sources-v2.json` (in the akidev-sources repo) when there is one, and the daily tracker job refreshes both.

**Why.** An installed app refuses a whole document for a value it does not know, and a refused document also stops
tracker updates and `min_version`. With two feeds, a value only newer builds can read has somewhere to go that old
apps never look. I checked that a schema 1 build refuses a schema 2 document ("this app reads 1").

**Turned down.** Two full copies of `sources.json` in the repository (they would drift), and a "features" list in
the document (more machinery than the problem needs).

**Costs.** Same version in both, so a build prefers `/v2` when both exist and takes the root when `/v2` has the same
version only after the next bump. Two sets of files to publish, which the script and the workflow do together.

## Stream sources are a trait (2026-11-21)

**Chosen.** `server/src/play.rs` has `StreamSource` (a name, `honors_pick`, and `find`) with three implementations
(debrid, torrent, the Internet Archive) and one for a name in `STREAM_SOURCES` that is not a source. The old
`match` over three strings is `Player::try_sources`, which takes the sources as a list. The three bodies were moved
without change.

**Why.** The order, the pinned-release rule, the split of the time and the "which reason to tell the viewer" rule
were wired into one function that could not be run without a torrent engine and the network. Now they run against
fake sources in tests (`play::tests`), 9 of them: order, early stop, busy, a pick skipping the Archive, the time
share (a third, then a half, then all), no releases, an unknown name, and failure notes cleared by a success.

**Turned down.** A trait for `Origin` (the bytes) and the Archive as a JSON definition: nothing needs them yet. A new
debrid service is still a new Rust type that implements `StreamSource`; the trait makes that one type, not a change
to the loop.

**Costs.** One more indirection to read. The sources share the plan through `Search`, so a source that drops a
failed release also drops it for the next one, as before.

## Work not done, and why (2026-11-21)

- **Credits and privacy text from data.** The text is a legal statement. Which providers it names, and under
  which licence, is the owner's call.
- **The shared "bonus words" list.** The three lists mean slightly different things (one is a tie-break when
  merging, one rejects extras). Merging them would change behaviour.

## Titles from a plugin's catalogue say where they came from (2026-11-21)

**Chosen.** A key is `kind-provider~escapedid` for a plugin's catalogue (`media_id::qualified_id`), and stays
`kind-id` for the built-in ones. The provider is folded into the text that was already called `provider_id`,
so every function that passes an id around keeps working. `providers::detail` splits a key with a `~` and asks
that provider only. The app makes the episode ids of such a title itself.

**Why.** The built-in chain asks provider after provider with the same id, so a plugin's numeric ids would be
answered by AniList with the wrong title. A catalogue plugin needs its own keys.

**Turned down.** A `provider` field in every key (it would change every saved key in the library, history,
downloads and party links) and a key format for all providers (the same reason).

**Costs.** A key can be long with a UUID in it (each dash becomes `_2d`). A plugin's provider id cannot have a
dash, because a key is split at the first one after the provider.

## The data lives in its own repo (2026-11-21)

**Chosen.** `sources.json` and its README moved to a public repo, `dishant0406/akidev-sources`. This repo keeps only code.
`crates/core/build.rs` downloads the file to `content/sources/sources.json` (git-ignored) when it is missing, so every
build, local or CI, gets a built-in copy without any workflow step. `publish-sources.sh` needs `SOURCES_DIR`, a checkout of
the new repo, so what gets published is always what is committed there.

**Why.** Changing a source should not mean touching the code repo, and the code repo should be only the skeleton.

**Turned down.** A git submodule (reproducible, but every data change needs a pin commit here). A download step in each
workflow (six places to forget; `build.rs` covers all of them, plus the iOS and Android engine builds).

**Trade-off.** A build needs the network once, and it is not reproducible: it reads the newest `main` of the sources repo.
A bad commit there can break this repo's tests (`read_tests.rs`, `screens.test.ts` read the document). To pin an older copy,
put your own file at `content/sources/sources.json`. The old copies stay in this repo's git history.

## Credits and kind labels are data; the old flag is gone (2026-11-21)

**Chosen.** The footer credit, the names on the privacy page and the small label over a title ("FILM") now come from the
`screens` section (`about`, and `tag` on each tab), so the website has no list of sources or kinds of its own. The
`anime_kitsu` flag, and the built-in fallback for a document without mappers, were removed: `map` is the only way.

**Why.** Adding or dropping a catalogue should be one change to `sources.json`. There are no users on old documents, so
the compatibility code had no job left.

**Left in code on purpose.** The three shapes (anime, series, movie) are behaviour: how episodes are keyed and played.
The Prowlarr route and its URL and key are server secrets, which cannot live in a public document. The `schema 1`/`/v2`
feeds and the old-app check are still there; they only matter while old apps exist.

## Plugins can bring code, as WebAssembly (2026-11-22)

**Chosen.** A plugin may bring a `.wasm` file (named in the plugin JSON with its sha256). An indexer, a catalogue or a
mapper names an export in `code`. The module replaces only the *transport step*: it makes the requests (through one host
function, `akidev.http`) and answers with JSON. The source's own `response`/`card`/`list` definitions read that JSON, so
every check that exists (hash, episode and ranking rules) stays and a module cannot get a release past them. The engine is
`wasmi` (a pure Rust interpreter) behind the cargo feature `code-plugins`, which only the desktop app turns on.

**Why.** JSON cannot sign a request, log in or read a page. The owner wanted people to add their own service without a
code change in the app. WebAssembly because the owner chose it: authors can use any language, and it isolates well.
`wasmi`, not `wasmtime`, because a JIT is not allowed on iOS and the interpreter is small.

**Rules.** Fuel (300 million instructions), a memory limit, a wall clock, a request cap, an answer cap and a module size cap,
each tested with hand-written modules that loop, grow memory or flood requests. Every request is held to the public https
rules of a JSON plugin, to the hosts the source declared, and to the forbidden-header list. A signed document can never
carry `code`.

**Turned down.** Putting the module inside the JSON (base64, large) and giving the host crypto and HTML parser functions
(a bigger host surface; signing and parsing compile into the module instead). A scripting language (Rhai, QuickJS): the
owner picked WebAssembly.

**Not built.** Stream services as plugins: the desktop app has no way to turn a release into a playable URL except its own
torrent engine (debrid and the Internet Archive are code in the server), so it needs a new playback path in the app. A
Settings field for a plugin's login or key. Plugins on the website: the server does not turn the feature on.

## Stream services: a plugin turns a release into a link (2026-11-22)

**Chosen.** A plugin's `services` entry names a module export that wraps a debrid-style service's API. The app calls it
for `files` (what the service holds for the release) and then for `link` (the file the app picked). A module that needs time
answers `retry`, and the app does the waiting. The app tries enabled services before the torrent engine, for the best four
releases of the plan, within 20 seconds, and falls back to the torrent engine when none has the release. The link plays in
mpv as it is, so the player needed no change (it already shows no torrent stats when there are none).

**Why.** The desktop app could turn a release into a URL only through its own torrent engine, so a user could not bring a
service of their own. The module does only the service-specific calls; everything that needs judgement stays in the app.

**Choices inside it.** The app, not the module, picks the episode's file, with `releases::pick_for_identity`, which moved out
of the server so both use one copy. A module has no clock, so the app owns polling and the deadline. The link must be https,
public and on a host the plugin named (`media_hosts`). The key is typed in Settings, kept in the settings file (now readable
only by its owner) and given only to the module. Tried first by default, because it starts at once.

**Turned down.** Letting the module pick the file (it would have to reimplement the episode rules, and a wrong pick plays the
wrong episode). A Keychain for the key (a new dependency; the settings file is private enough for now). Request headers on
the link (TorBox's link carries its own token; add them when a service needs them).

**Not built.** Downloads to the library and Watch party from a service link. The server keeps its own TorBox and
Internet Archive code.

## The request cap is shared across indexers (2026-11-22)

**Chosen.** A search plans at most 96 requests (`MAX_PLANNED_REQUESTS`). When everything fits, nothing changed: the indexers'
requests stay in their own order. When the cap would cut the list, it is now shared out one request at a time across the indexers,
and each indexer keeps its requests in their own order.

**Why.** Before, the cap was filled one indexer at a time. A title with dozens of names (an anime with many AniList synonyms)
gave 76 requests to `nyaa` and 20 to `subsplease`, so the eight indexers after them were never asked, and a popular anime such as
`anime-16498-ep1` found no releases at all, on the website and in the app. Found while testing before the 0.1.28 release.

**Turned down.** Raising the cap (more load on every indexer) and cutting the number of names per title (fewer ways to match).

**Not changed here.** The season problem for anime is fixed in the next entry.

## Anime releases of another season go to the back (2026-11-22)

**Chosen.** An anime entry has no season number to filter on (its episodes are a flat count), so `rank_releases_by` now reads the
season the entry is from its titles ("Season 2", "2nd Season"; none means season 1). A release that names a different season
("S02E01" for the first season's episode 1) is not thrown away, it is sent behind the others.

**Why.** Found by watching the app: Frieren episode 1 started a Season 2 file, because it had more seeders. Same episode number,
wrong season.

**Turned down.** A hard filter. Some titles never say their season ("Jujutsu Kaisen 2nd Season" is written many ways), and a hard
filter on a guess would hide the right file. Moving it behind keeps it as a fallback.

## Plugins and stream services in Settings look like the rest (2026-11-22)

**Chosen.** One hairline between rows, no boxes around boxes, one line under each heading, and the Add and Save buttons sit inside
their field. A plugin row says what it brings and leaves out what it does not ("v2, 1 indexer", "v1, 1 stream service"). The
folder path shows with `~`. The long explanations were cut to one line each.

**Why.** The old section had paragraph help text, a button beside every field and a sentence about the folder. It read as
generated. The new one says only what a person needs to act.

## Manga sources can sign in (2026-11-22)

**Chosen.** A manga source can have an `auth` section. It says which fields the person types (an email, a password), which request signs in, and where in the answer the session is (`access`, `refresh`, `user`). Those session values become holes (`{access}`) in every later request, and an `authorization` header is added to requests to `base_url`. If a request gets 401, the engine asks the `refresh` request once and tries again. The code is `crates/manga/src/source.rs` (`sign_in`, `sign_out`, `refresh_unless_done`) and the shapes are in `def.rs` (`AuthDef`).

**Why.** Some sites (Asura is the one we looked at) have free titles and paid ones behind an account. Their sign-in is a plain POST that answers a token, so it fits the idea that the JSON controls everything. A company that wants to give us a source can give a token endpoint or an API key the same way.

**The safety rules.**
- The definition holds no secret, because a repository is public. The person types the account in the app.
- The engine never keeps the password. It keeps the session, and the app keeps that in Keychain or Keystore.
- The session goes only to `base_url`. The sign-in, the refresh, the logout and the headers never go to a mirror, because a mirror is another host.
- A password is never in an error message. A refused sign-in says "is the account right?".
- Fixtures use made-up accounts. `record` is not used for a sign-in, because it would save real tokens.

**Turned down.**
- Letting a setting hold the password. A setting only holds one of the options the definition lists, and it would be stored in the database.
- Sign in with Google and other redirects. They need a browser, and the app has no web view.
- A captcha or Turnstile token. Nothing here can pass a check.

**In the apps (same day).** The engine does not store the session itself. The app gives it a `SessionKeeper` (a small interface in `crates/manga-ffi/src/lib.rs`, with two calls: `load` and `save`). The engine calls `load` when a source loads and `save` every time the session changes, a refresh included. So the apps only hold a safe key-value store: `apps/manga-ios/App/SessionKeychain.swift` (Keychain, this device only, after first unlock) and `apps/manga-android/.../sources/SessionKeystore.kt` (AES-GCM with a key that lives in the Android Keystore). All the sign-in logic stays in Rust.

The screen is one sheet, drawn from the fields the definition lists, with the host the account goes to: `SignInSheet.swift` and `SignInSheet.kt`, opened from the source's own settings page (an "Account" row: Sign in, or the person's name and Sign out). It uses the app's small components only. Two new ones: a secure field and a labelled field (`AkiField`), because a form field keeps its name above it when it fills.

**Turned down in the apps.** Keeping the password so the app can sign in again by itself (the refresh does that without it).

## A chapter can say it is locked (2026-11-22)

**Chosen.** `chapters.chapter.locked` is an expression like the others (`"locked": "is_premium"`). Any text counts as yes except `false`, `0`, `no` and `null`. The engine keeps it as `Chapter.locked` and the library keeps it in a new `chapter.locked` column (database version 11, added by an `ALTER TABLE` with a default of 0, so an existing library is not touched). When a listing is read again the value is replaced, so a chapter whose early access ended unlocks by itself.

**In the apps.** While the person is signed out, a locked chapter shows a filled lock in the place of the read mark (muted, 20 pt, with the accessible name "Locked"). The icon is the state, so the line above the title is left as it was, unless the source gives its own word in `chapters.locked_label` ("Members only"). By default a tap opens the sign-in sheet first (when the source has `auth`), and the chapter opens as soon as sign-in works; `chapters.locked_action: "none"` only shows the lock and opens the chapter as usual. A signed-in person sees it as any chapter, because the site decides what an account may read.

**Why the lock replaces the read mark and no word is added.** The two marks mean different things in one slot and cannot both be true (a chapter you cannot open has not been read). An icon and the word "Locked" would say the same thing twice. The word comes from the source only when it says something more ("Early access").

**Why a flag from the site and not a guess from a failed request.** A failed read cannot tell "locked" from "broken", and the first tap would always cost a request and an error screen. The chapter list already says it (`is_premium` on Asura).

**Turned down.**
- Hiding locked chapters. Some people see them on the website too, and they read in order.
- Reading a date as the flag directly. A date in the past would still count as "yes". The `in_future` step turns a date into yes or no, and it is the source's JSON that says to use it.
- Dimming the cover of a locked chapter. It makes the row look disabled, and it is not: a tap does something.
- Colour for the lock (an accent or red). The app has one accent and it marks the current choice. The lock is a quiet fact.

## The video loop waits only a few milliseconds for events (2026-11-22)

Video felt laggy on 4K files. We measured it: the picture reached the screen about every 61 ms (roughly 16 fps) for a 24 fps film, and mpv dropped about 8 frames a second. Decoding and drawing were not the problem (a frame draws in about 6 ms, no decoder drops).

The cause was the loop in `apps/desktop/src/player.rs` that both reads mpv events and draws frames. It waited up to 50 ms for events, so frames were drawn late and mpv threw them away. Now it waits at most 4 ms. The same file then played at a steady 24 fps (frames 42 ms apart) and mpv dropped one frame in 40 seconds. CPU is about 30% of one core for 4K.

Rejected for now: drawing on the GPU instead of the CPU, which mpv's own header suggests for speed. The CPU path costs little on Apple silicon (about 20% of one core for 1080p), and the GPU path needs a large change in how GPUI shows frames. We will do it only if a real file still stutters.

Not tested: Windows, and files at 60 fps.

## A skipping catalogue is asked for one page, and a second only when the first is short (2026-11-22)

We measured the desktop app before changing anything. Home scrolls at 120 fps, a title page needs one request (about 120 ms), and the window opens in about 215 ms, most of it the graphics framework. The gap was in the network: a cold start made about 19 requests, and the banner waited for the slowest.

The cause: for a catalogue that pages by skip (Cinemeta, 48 titles a page), `crates/core/src/providers/list.rs` always asked for one page more than the row needed, "in case" titles were left out. The banner needs 6 titles and a row 24, so the extra page of about 120 KB was almost never used, but the banner waited for it.

Now one page is asked for. If it comes up short after titles are left out (not out yet, twice) and the catalogue goes on, one more page is asked for. Three cold launches each, alternating old and new: banner median 1808 ms before, 384 ms after; first row 546 ms before, about 270 ms after. Two tests in `crates/core/src/providers/read_tests.rs` cover both cases.

Not changed, on purpose:
- Rows that read a window of 3 pages (the date sort and genre rows). Reading fewer would show older titles in "New Releases", and Cinemeta's own genre address did not give matching titles in a test.
- Pressing Play takes about 13 s to the first frame, mostly torrent pieces (the file index at the far end of a 12 GB file, the resume jump). That needs a change in the torrent engine, so it was left for its own piece of work.

## Tried and left out: fetching the end of the file at the same time as the start (2026-11-22)

When Play is pressed, the torrent engine fetches the first 3 MB of the file and only then the last 3 MB, where an MKV keeps its index. mpv waited about 6 s for that index on a 4K file. We made the two fetches run together and compared old and new builds on the same release, alternating runs. The index was then local (mpv opened the file in 0.02 s), but the start of the file took about 6 s longer (2.2 s became 8.4 s) because both fetches share the same few peers. Total time to the first frame was about the same, with too few runs to tell. We reverted it.

What this tells us: a Play start of 10 to 20 s on a 4K WEB-DL is limited by how fast the swarm sends, not by the order we ask. The release ranker puts the healthiest swarm first (50 or more seeders), and that is often a 4K file. Changing that trades startup speed against how many seeders the pick has, so it is a product choice, not a bug fix.

## The arrows at the ends of a row fade the right way (2026-11-22)

The fade behind a row's arrow was drawn backwards: solid on the side facing the posters and clear at the edge of the screen. That gave a hard vertical line across the poster, with a bare arrow on it. In `apps/desktop/src/row.rs` the fade is now solid at the screen edge and clears toward the posters, 96 px wide instead of 64, and the arrow sits in a quiet disc (the same wash as the hover on a button) so it reads as a button. Checked on screen at both ends of a row. Not done: the website's rows, which were not looked at.

## The search icon always ends on a page of titles, and the web palette opens at once (2026-11-22)

The magnifier in the top bar now does the same in both apps. On a screen that is not a browse screen it goes to Browse (the app: `Route::Browse`, the site: `/search`, which with nothing typed shows every catalogue) and opens the Cmd-K panel on top, so closing the panel leaves the viewer on a page of titles and not on whatever screen they came from. On a browse screen (any catalogue tab or Browse) it just opens the panel. In the app the music screens keep their own panel, as before. The code is in `apps/desktop/src/app.rs` and `apps/web/src/components/topbar.tsx`.

The site's panel felt slow for three reasons, all removed: the dimmed backdrop faded in over 0.7 s (now it appears at once), the panel animated for 0.16 s (now 0.08 s, in a new `snap` animation in `tailwind.config.ts`), and the panel's code was only downloaded on the first open (now it is fetched one second after the page loads). Measured in the browser: the panel is on screen about 6 to 8 ms after the click. The party panel keeps the 0.16 s animation.

Checked on screen in the app from Home and from Browse, and in the browser on a title page. Not checked: a phone, or the music screens after this change.

## One pass over words, screen readers and type, in both phone apps (2026-11-22)

We went through every screen of the iPhone app and the Android app, three ways: the words, what a screen reader says, and how the text is set. We did it for both apps at the same time, with one shared list of rules and one shared vocabulary, so the two apps say the same thing.

The choices:
- Fix shared pieces in the shared parts. Icon buttons got a bigger touch area (44 pt on iOS, 48 dp on Android) without changing how they look. Icons are hidden from screen readers, so every control must have its own name. Tabs, chips and the tab bar say which one is selected. A new `digits` option (iOS) and `tabular` option (Android) keep changing numbers from jittering. Message helpers announce toasts and errors.
- Vocabulary: a source is one site, a repository is a folder of sources, "Sign in", "Remove" takes something out of a list, "Delete" erases data from the phone. Errors say what failed, then what to do. Titles use sentence case. No "OK", no "Oops", no "Are you sure".
- Error titles are specific ("Could not load this series", "Could not reach the site") and come from one helper, `describe(error, failed)`, on both platforms.
- Menu items are verb first ("Edit categories", "Change categories", "Delete series data").

Rejected: a shared string file for both apps. It would be cleaner, but it would mean a build step and a larger change than this pass needed. The cost of the choice is that the two apps can drift again, so check the other app when a string changes.

Not fixed, and why: a few colour pairs may be under 4.5:1 (red caption text, muted text on a pale chip, text on a dimmed card). Colours are a design decision, so they are listed for the owner. Menu actions that remove a source, repository or catalogue still act at once without a question. The page text of a book drawn on a canvas on iPhone has no screen reader text yet.

Checked: both apps build, the Android unit tests and lint pass, and the Sources and Discover screens on the iPhone simulator. Not checked: a screen reader (VoiceOver, TalkBack) on a real phone, and the largest text size on every screen.

## Long press on Continue reading and History (2026-11-23)

Chosen: a long press on a "Continue reading" card offers "Mark all as read" and "Remove from history". A long press on a History row offers "Remove from history". On Android a screen reader gets the same items as actions, because it cannot long press.

Why: the shelf had no way to drop a series you stopped reading. The library cards already use a long press menu, so this looks the same.

"Remove from history" is one engine call, `forget_history` (`crates/manga/src/store.rs`, exported in `crates/manga-ffi/src/lib.rs`). It deletes every progress row of that one series. History is built from progress, so there is no separate history table to clear.

Trade-off: this also forgets which chapters were read. Rejected: deleting only the unfinished row. The series would then stay in the History list, which is not what "remove" says. Another cost: it acts at once, with no question, like the other menu items in the library.

## Speed pass: what was slow and what was not (2026-11-23)

I timed things before and after, on a Mac and on the iPhone simulator. The numbers are the reason for each choice.

- Date and chapter-number reading built its regular expressions on every call. A chapter list reads two per row. They are now built once (`crates/manga/src/date.rs`). 2000 rows went from 1069 ms to 40 ms on the Mac. A phone is slower, so the win there is bigger in real time.
- The HTTP client spoke only HTTP/1.1. I turned on `http2` and `brotli` in `crates/manga/Cargo.toml`. Twelve parallel requests to one host went from about 252 ms to about 135 ms. The first request costs the same. It only helps a site that speaks HTTP/2 (a site that does not still works as before).
- The home feed asked every source for its popular and new lists on every open. A list the engine kept in the last ten minutes is now shown and not asked for again (`Engine.fresh_listing`). Pull to refresh and "Try again" still ask again. On the simulator a relaunch went from 3 listing requests to 0. The first launch with an empty cache is unchanged (4 requests). At most four sites are asked at a time, so the first sources answer first. Reading what was kept now happens off the main thread.
- A series that was opened in the last ten minutes no longer asks again for its details when the stored ones exist (it already skipped the chapter list).
- The library sorted its whole list up to three times for each screen draw (iOS). It now sorts once.
- Android covers: a cover that is still waiting for a connection is dropped when the last row that wanted it scrolls away. Before, a fast scroll left the covers on screen waiting behind dozens of covers scrolled past. A download that has begun is still finished and kept.

Measured and left alone: starting the engine takes about 2 ms (12 ms the first time), and every engine read is under 1 ms (the library after a write is 7 ms on a big library). The audit guessed 50 to 300 ms for start-up. That guess was wrong, so I did not move the engine off the main thread. Saving a 2000-chapter list takes about 10 ms, also left alone.

Not measured: the Android cover change and the iOS library change (no device scroll test), the Android feed request count (the Android app has no demo repository switch), and how much the library file grew from HTTP/2 and brotli.

Rejected: a cache inside `popular` and `latest` themselves. Pull to refresh would then need a flag in every call, and the apps already know when the person asked for fresh data.

Trade-off: a list can be up to ten minutes old. New releases on a site can show up that much later unless you pull to refresh.

## The phone app page: one phone that follows the text (2026-11-23)

Chosen: a new page, `/app` (`apps/web/src/app/app/page.tsx`), for the iPhone and Android app, with links to the App Store and Google Play. One phone stays on screen while the text scrolls past. The screen on the phone follows the text: a short recording of the app first, then a picture for each part (sources, a series, reading, library, books, read aloud). The phone turns a little for each part and follows the mouse.

Why: a store link alone does not say what the app is. The film we made already has the app on a phone, so the page reuses it. `/manga` stays as it was, because the App Store listing links to it.

The pictures are not the store screenshots. Those show a real series. The page uses the film's made-up art, taken by `content/manga/film/tools/web-assets.py`, and says so in the fine print. The words come from the Play listing and the app's own screens, nothing more. "Free" and the iOS 26 and Android 10 minimums were read from the stores and `build.gradle.kts`.

Details that matter:
- Yellow is used here (and nowhere else on the site) because this page is about the phone app, and the phone app is yellow. The rest of the site keeps amber for state.
- The store buttons are our own: a plain Apple mark and a play triangle, with text. The official badge art needs a download from Apple and Google, so it is not in. Swap it in if you want it.
- The phone turns on a critically damped spring (no bounce, about half a second), starting from where it is, so a new push or a new part never snaps it. Without motion allowed there is no turn, no video, and a short fade between screens.
- The phone is sized from the screen height (`svh`), so it fits any window. On a narrow screen only its top part shows above the text, so the store buttons are in view at once.
- A store click sends the same `download_clicked` event as the desktop page, with `ios` or `android`.

Rejected: a full-width background video. The film is square and crops badly on a wide window. Also rejected: scrubbing a video with the scroll bar. It costs a lot of work for little gain over six pictures.

Trade-off: the pictures and the recording are copies of the film. If the app changes, run the script again and copy the files into `apps/web/public/phone/`.

## The phone page lives on its own host, and has no site around it (2026-11-23)

Chosen: the page is shown at `app.akidev.live`, not at `akidev.live/app`. It is the same frontend, not a second app. The middleware shows the page `/app` at the root of that host, and `layout.tsx` leaves out the top bar, the footer and the providers that call the API when the host is `app.`. Caddy and the DNS script (`scripts/cloudflare.sh`) list the new host.

Why: it is a landing page. A short address is easy to say, print on a store page and share. A top bar full of anime, series and films, and a footer about downloads, would hide what the page is for.

Details: `/app` on the site redirects to the host (308), so there is one address. Any other path on the host redirects to the same path on the site, so no page of the site can appear without its top bar. Links from the landing page to the site (privacy, help, desktop app) use the full `https://akidev.live/...` address, because the host has none of those pages.

Rejected: a second Next.js app or container (a second build, deploy and set of fonts for one page), and route groups with two layouts (many pages would move for one page).

Trade-off: the layout now reads the `Host` header, as the middleware already did. The host name must match `app.` plus `NEXT_PUBLIC_SITE_DOMAIN`. In development, set that variable to `localhost:3100` and open `app.localhost:3100`.

## The phone page is bright, with one 3D phone (2026-11-23)

Chosen: the page at `app.akidev.live` is bright and quiet, like the phone app and unlike the dark site. One real 3D phone (three.js, built from code, lit by a studio room) plays the app on its screen inside one large grey panel. You can drag it, and the scroll turns it. Below it are the app's screens in a swipeable row, a short list of what the app opens, the privacy lines in large type, the questions, and a last block with the store links. The only colour besides black and white is the app's yellow, used as a highlighter on one word.

Why: the first version (a dark page, glowing gradients, tilted phones, a sticky phone with a blurred glow) looked like every generated landing page. We looked at Things, Bear, Overcast, Readwise Reader, Lusion, Apple and the Dribbble results for "app landing page". The pages that look expensive are calm. They have one real object, one accent and a lot of space. The Dribbble results were mostly the gradient look we had.

Details that matter:
- The bright look is the same token names with light values (`html[data-landing]` in `globals.css`), so every colour class on the page works unchanged. This is an exception to `design/DESIGN_PLAN.md`, for this host only: the phone app has its own look (`design/manga.md`).
- Type is one family (the site's Hanken Grotesk), weight 600, tracking -0.02em on large text, line height 1.05 to 1.15 on headings and 1.6 on body text. No uppercase labels. No gradients except one hard-edged highlight.
- Store links are plain black badges drawn by us. The official badge art needs a download from Apple and Google.
- The 3D phone is a progressive enhancement. The page draws a flat phone first and swaps in the 3D one when the browser is idle and WebGL works. The 3D code is a separate chunk (about 178 KB gzipped, measured in a production build), so the first picture never waits for it.
- The phone turns on a critically damped spring (no bounce, about half a second), so a drag or a scroll never snaps it. Vertical swipes scroll the page, sideways swipes turn the phone.
- There is a pause button for the video, because a loop that never stops needs one.

Rejected: a full-width video behind the page (the film is square and crops badly), a second phone model file (a model is a download and a licence to track, and the phone is simple), and scroll-pinned sections with a huge empty page (they looked fine on a desktop and bad on a phone).

Trade-off: this adds the `three` package. On a device that can run WebGL it is one more download after the page is ready, and it uses the GPU while the stage is on screen. Devices that cannot, or that save data, get the flat phone and lose nothing else.

## The phone turns through the app as you scroll (2026-11-23)

Chosen: the 3D phone is now the page's main show. A tall block of scroll holds one panel that stays on screen. As you scroll, the phone turns and changes size, the screen on it changes to the next part of the app, and the words beside it change. Dots under the words show the step and jump to it. Six steps: sources, series, reading, library, books, read aloud.

Why: the first version showed one phone you could drag. You asked for the phone to work and show the features as you scroll. The research (Apple's product pages, Awwwards scroll stories, the tutorials) agrees on one structure: a tall container with a pinned stage, where the scroll position drives the object.

Details that matter:
- The pose for each step is written in `POSES` in `feature-story.tsx`. The phone is always between two of them, so it is moving all the time the page scrolls. The turn alternates left and right so no two steps look alike, and the last step faces front.
- The screen swap is a fade between stacked pictures on the phone, in the middle of the turn. The first screen is the video.
- It is one panel, two columns on a wide window (words, then phone) and one column on a phone (phone, then words). The panel is always one screen high.
- A visitor who asks for less motion gets the plain row of screens, not a pinned tour.

Rejected: a long page of one-screen-high sections with the phone sticky beside them (the first design, which had big empty gaps and was bad on a phone), and a pre-rendered image sequence (it is heavy, and it cannot follow a drag).

Trade-off: the tour is about 5.5 screens tall on a wide window. That is long for a landing page. The dots and the fast scroll help, and the rest of the page (what it opens, privacy, questions, store links) comes right after.

## The tour fills the screen, with no container and no video (2026-11-23)

Chosen: the tour is the whole page's first screen and fills the window. There is no grey panel and no play and pause button. The opening words are the first picture of the stage, with the phone rising under them. As you scroll, the phone flies across the stage, turns and grows, giant words sit behind it, and the stage changes colour (white, grey, the app's Night theme for books, and its yellow for read aloud). The 3D canvas is as big as the window.

Why: you said the contained panel and the play button were not what you wanted, and that you wanted a full immersive 3D experience. The earlier panel was a box on a page. This is a stage.

Details that matter:
- The video is gone. Every screen is a still picture, so nothing plays on its own and no pause button is needed. The file `app-loop.mp4` was removed from `public/phone/`. `web-assets.py` can still make it.
- Each step holds its pose for half its scroll, then moves to the next pose. Before, the phone was always in between two poses and never at rest, which was hard to read.
- The giant words are behind the phone and the small text is in front, on the opposite side. On a narrow screen the words are above the phone and the text is below it, and the phone is smaller and nearer the middle.
- The phone sits under the opening words whatever the window size: the page measures the words and places the phone just below them.
- The drag works anywhere on the stage, but not on a link or a button, so the store buttons stay usable. Sideways drags turn the phone. Vertical swipes scroll the page.
- The opening's buttons leave the tab order once they have faded out. The heading stays in the page.

Rejected: keeping the video for the first screen (it needed a pause button and a loop that never stops), and a coloured background that changes for every step (we use four looks and tie two of them to the app: Night and the highlighter).

Trade-off: the stage is about 6.5 screens tall. A visitor who wants the store buttons later can use the last block of the page. The page works the same without WebGL, with a flat phone that moves and changes screen.

## The tour snaps to its steps (2026-11-23)

Chosen: one scroll gesture moves the tour by exactly one step, and none can be skipped. A wheel or trackpad is handled in code (`onWheel` in `feature-story.tsx`): the first event of a gesture scrolls smoothly to the next or the previous step, and the rest of that gesture is ignored for a moment. A touch screen and the keyboard use CSS scroll snapping with `scroll-snap-stop: always`. Snapping is on only while the stage is pinned.

Why: a hard flick, or a trackpad that keeps sending events after the finger lifts, carried the page past two or three steps, so visitors missed parts of the tour. Plain CSS snapping did not fix a single mouse wheel tick, because a tick is smaller than half a step and the page snapped back to where it was.

Details that matter:
- Snap points are at the start of each step (where the phone has reached its pose and rests) and at the end of the last hold. There is no snap point past that, so scrolling down from the last step leaves the tour.
- Snapping turns off outside the pinned range. With it on at the bottom edge, a small scroll was undone and the visitor could not leave, or could not come back.
- It does not catch pinch zoom (Control and wheel) or sideways scrolls.

Rejected: only CSS snapping (a single wheel tick did not move), and only code (touch and keyboard would still skip steps). Using both is a little more code and gives the same behaviour everywhere.

Trade-off: taking over the wheel inside the tour is stronger than normal scrolling, so a visitor cannot scroll slowly through a step. The tour is the point of the page, and the rest of the page scrolls as usual.

## No dots, and the words never meet the phone (2026-11-23)

Chosen: the row of dots under the tour is gone. The scroll is the only control. The words and the phone each have their own half of the stage: the phone moves to one side (at 50% of the half-width, and no larger than 1.35 times its size), and the title and text take the other side. On a narrow screen the phone is in the middle and smaller, with the title above it and the text below. The words show while the phone rests, go as soon as it moves, and return a moment after it has arrived.

Why: you said the dots were not wanted, and that the words and the phone overlapped too much on a phone and on a computer. The overlap had two causes: the phone was too close to the words at rest, and the words stayed on screen while the phone flew across them.

Details that matter:
- On a computer the title is at most 36% of the width wide and the phone's half starts at about 60%, so they have a gap. The step with the large phone (reading) moves it to the right half too, and no longer covers the title.
- On a phone the phone is 40 to 45% of the height. The title is in the top 20% and the text in the bottom 20%.
- Out is instant and in has a 0.4 s delay, so a visitor never sees the words and the phone at the same place.

Rejected: keeping the giant words behind the phone (it looked deep but hid the words), and fading the words in step with the scroll (the phone rests at one exact scroll position because of snapping, so a scroll-linked fade was not finished when it rested).

Trade-off: the text is smaller than before, and the phone is smaller on a phone. The page reads clearly at the cost of some drama.

## The phone zooms through each step, and the page ends white (2026-11-23)

Chosen: between two steps the phone no longer just turns to the next side. In the middle of each move it goes to the centre, faces front and grows until its screen covers the whole stage. It shows the part of the screen that matters for the next step (the `FOCUS` list), swaps to the next screen at the biggest point, then shrinks and settles at its next pose. The page colour follows the scroll on `html` and `body`, and in the last hold it fades from the last step's colour to the page's white. The tour ends on plain white.

Why: you said turning the phone was not enough, and asked for moves that fill the screen. You also said the ending must be white and the colour must belong to the whole page, not a box.

Details that matter:
- `coverScale()` in `phone-scene.ts` gives the size at which the screen covers the stage. `focusY()` slides the screen up or down to the wanted part, but never further than the spare height of the screen, so no white strip shows at the edge.
- The move uses a sine curve (`bump`): 0 at rest, 1 in the middle. Position, turn and tilt are multiplied by `1 - bump`, and the size goes towards the cover size by `bump`.
- `HOLD` is 0.4 of a step (it was 0.5) and the springs answer in 0.38 s, so the move feels quicker.
- The white fade uses the unclamped scroll (`scrolled`), because the clamped value stops at the last step and the fade would never start. This was a real bug found while testing.

Rejected: a zoom that only scales (it kept the side pose and the screen crossed the words), and a separate full-screen overlay (it would need a second copy of every screen).

Trade-off: the phone shows only a slice of the screen at the peak. The peak lasts a moment, so this reads as a zoom and not as a crop.

## The zoom-through is gone: one smooth move, a big phone (2026-11-23)

Chosen: the zoom in and then out on every step is removed. Each step now has a large phone (1.5 to 1.8 times its first size on a wide screen) on one side, and the phone moves to the next pose in one smooth turn. The words show 0.12 of a step before the phone rests and come in after only 0.1 s. `HOLD` is 0.5, so the phone rests for half of each step. The page colour still follows the scroll, and the tour still ends on plain white.

Why: you said the zoom in and out looked broken, the text came and went, and the words were missing most of the time. Two things caused the missing words: the zoom made the move long, and the words came in 0.4 s late and left early. You asked for a simple zoom and then the text.

Rejected: zooming only on two steps (more code, and the page would feel uneven), and keeping the zoom with a softer curve (it was still in and out).

Trade-off: the phone is big but never fills the whole window with its screen, so the effect is calmer than a full zoom.

## The stage follows the visible height, and the phone is bigger on a phone (2026-11-23)

Chosen: the stage is `h-dvh` (the height you can see now), not `h-svh` (the height with the browser bars out). On a phone the phone is about 1.05 times its first size (it was 0.78), and the flat placeholder phone never grows past its first size.

Why: in a phone browser the bars slide away as you scroll, so the visible area is taller than the `svh` height. The stage stayed short and left a white gap under the text. The placeholder was also scaled like the 3D phone and ran into the words.

Trade-off: when the bars move, the stage changes height and the 3D phone fits itself again. That is a small shift, and it is better than an empty gap.

## On a narrow screen the phone is sized to the room, and the placeholder sits under the words (2026-11-23)

Chosen: on a narrow screen each step's phone is sized to 60% of the stage height, worked out from the scene (`0.6 / heightFraction()` in `update()`), not set as a fixed scale. The flat placeholder phone is put just under the opening words, the same way the 3D phone is, using the `--fy` variable.

Why: you sent screenshots from a real phone. The phone was taller than the room between the title and the words, so it covered both. The placeholder also sat on top of the "Free. iPhone needs iOS 26..." line.

Trade-off: the phone is a little smaller on a tall screen than a fixed scale would make it, but it never covers the words, on any height.

## The phone loads lighter, with the same picture (2026-11-23)

Chosen: three changes, and no change to what you see. (1) The 3D phone asks for the normal graphics card, not the high-power one (it was `powerPreference: "high-performance"`). (2) The six screen pictures reach the graphics card one after the other, and each goes up as soon as it arrives, so the first frame never waits for six big uploads. A screen that has not arrived yet is the dark of the phone, not a white plane. (3) The flat placeholder asks only for the picture on show and the next one.

Why: you said the phone felt heavy. We measured first. The 3D code is about 144 kB gzipped in the production build and the shape has a few thousand triangles, so those are small. What was heavy was the start: every picture was fetched twice and sent to the graphics card in the first frame, and the high-power card cost battery on laptops.

Left alone on purpose: the picture size (924 by 2160), mipmaps, 2x pixel ratio and antialiasing. Cutting any of them would make the phone look worse. Six pictures still take about 8 MB each in graphics memory. Going lower needs compressed textures (a new dependency) or smaller pictures, and I did not do either without asking.

## Half-size pictures for narrow screens (2026-11-23)

Chosen: each screen picture now also exists at 462 by 1080 (`public/phone/*-m.webp`, made from the 924 by 2160 file with Lanczos and `cwebp -q 90 -sharp_yuv`). A screen narrower than 768 px loads those for the 3D phone, so each picture takes about 2 MB in graphics memory, not 8 MB (about 12 MB in all, not 48 MB). The flat placeholder uses `srcSet`, so it always takes the small file, and its images are removed once the 3D phone is on.

Why: webp helps the download, not the graphics memory, because the graphics card holds every picture as plain pixels. The only way to use less memory is fewer pixels. On a narrow screen the phone is about 450 CSS pixels tall (up to 900 device pixels), and the graphics card already read the 1080 high level of the picture there, so the half-size file looks the same.

Kept at full size: wide screens. A big phone on a retina laptop needs the 2160 pixels.

Rejected: compressed textures (KTX2). They cut memory too, but need an encoder step and the Basis decoder files, and the 4 by 4 block loss would show on small text in the screens.

## The first phone follows the opening words (2026-11-23)

Chosen: the page now watches the size of the opening (`ResizeObserver` on the hero, and `document.fonts.ready`) and puts the phone again whenever it changes. The gap between the last line of the opening and the top of the phone is 32 px (it was 24).

Why: you sent a screenshot where the phone sat on the line "Free. iPhone needs iOS 26...". The phone was placed once, from the size of the opening at that moment. When the fonts arrived, the lines changed size and the phone stayed where it was.

Trade-off: a little more work on each resize of the opening, which only happens a few times when the page loads.

## Search on the phone: the crash, Done, and the Home icon (2026-11-24)

**The crash.** Chosen: in `GlobalSearchView.swift` the source task now keeps the search result in a variable (`let found = try await ...`) and returns it on the next line. Before, the `try await` sat inside the returned value (`return (id, .done(try await engine.search(...).items), nil)`). The same change is in `BooksBrowseView.swift` (catalogue search).

Why: a TestFlight build on an iPhone 17 Pro (iOS 27.2.1) stopped with "pure virtual function called" inside Apple's task code. The crashing thread had no app code on it, so the report did not say where. We built a Release version with logs and ran it on the phone. It crashed every time (three runs, same report): four sources started, none ever reported back, the search ended in under a second, then the app aborted. With the result kept in a variable it did not crash (four runs, including one search while another had run before). The debug build on the simulator never crashed, so only a Release build on the phone shows it.

Honest limit: we do not know the exact cause. Our best guess is that the Release compiler handles `try await` inside a returned tuple wrongly, so the group ends while child tasks still run. The fix matches the evidence, but it is not proven. `Feed.swift` has a similar line (`await Result { ... }` inside a returned tuple) that we did not change, because it has not crashed.

Rejected: changing `for await ... where` into a plain `guard` (we tried it, the crash stayed, so it was not the cause; the loop is back to the original). Replacing the task group with one `Task` per source (more code, and the group's wait and cancel would be lost). A version of the same fix is not needed on Android: the crash is in Swift's runtime.

**Done in a search field.** Chosen: the "Done" button inside the field now does what the keyboard's Search key does: it closes the keyboard and searches (`AkiInput.swift`, `AkiInput.kt`). In a password or email field it only closes the keyboard.

Why: it used to only close the keyboard, so people typed a word, pressed Done, and nothing happened. That was on purpose (a way out without a search), but it is not what anyone expects from the button next to a search field.

Trade-off: there is no longer a button that closes the keyboard without searching. On iPhone, scrolling the page still closes it (`scrollDismissesKeyboard`). We did not check how Android does it.

**Tapping the Home icon again.** Chosen: when Home is already open and has something to undo (typed text, a search, filters, or a series opened from it), tapping the Home icon clears all of it and shows the dashboard from the top. A clean Home is left alone.

On iPhone the Home screen is made again with a new identity (`.id(model.homeResets)` in `AkiMangaApp.swift`). That clears every `@State`, and closes any series page, because the whole stack goes. `HomeView` tells the model if it is "dirty" (`model.homeDirty`), and the tab bar only counts a tap when it is. On Android the state lives in `DiscoverModel`, so it is cleared in place (`DiscoverModel.goHome`), the pushed screens are closed (`Router.clear`), and the list scrolls to the top.

Why only when dirty: the first version made the screen again on every tap. On a clean dashboard that reloaded the lists and the screen flickered.

Rejected: a navigation path just for Home (more code, and `MangaStack` is shared with the Manga hub). Resetting the chosen source to "All sources" (the source you picked stays, as it does after a relaunch).

Trade-off: on iPhone, "a series covers the screen" is known from `onAppear` and `onDisappear` of the Home screen. This was not tried on a phone with a series open at the time of writing.

## Asian drama title and source matching (2026-11-25)

Chosen. Cinemeta remains the first series catalog. When a title has one name, Akidev asks TVmaze for the same IMDb id or TV database id. It adds at most two TVmaze names to the release search. TVmaze also supplies country and language data.

Chosen. Akidev adds two fallback Nyaa live-action sources. Category `4_1` and category `4_4` each search once per title. The engine asks them only when the normal sources find nothing worth opening. The fallback search has a longer time limit.

Chosen. Release matching accepts Korean, Chinese, Thai, Mandarin, and Cantonese language tokens. It also accepts Korean `제N회` and Japanese `第N回` episode forms.

Chosen. A year in brackets no longer makes a series pack impossible to inspect. Akidev puts these packs last. The pack year must match the series year. The torrent or debrid file list must still prove the episode number.

Chosen. Akidev applies a strict country filter only when TVmaze finds more than one same-name title from the same year. An ambiguous title must have a country token in the release name. A foreign native alias cannot bypass this filter. This stops the US The Gifted from answering a request for the Thai The Gifted.

Chosen. A host rate-limit wait has its own time limit. It does not reduce the time allowed for the HTTP request.

Why. The old search used one English title. It had no live-action Asian source. It treated every year-bearing pack as a film. It also accepted a same-name show from another country.

Turned down. A hardcoded alias list. It would become stale. A blanket language filter would hide valid packs that do not name their language. Private trackers need accounts that this work does not have.

Costs. A series detail can add one TVmaze search and detail request. The limit is two seconds. Nyaa can rate-limit a busy host. An ambiguous title with no correct country evidence shows no source instead of a wrong show.

## Languages, countries and fallback sites are data again (2026-11-25)

Chosen. The sources document has three new parts. `languages` holds the codes, names, release words, short tags and country names. `aliases` says which provider gives a title's other names and which catalogue is searched when nothing was found. The `prowlarr` indexer has a `prowlarr` block with the sites to set up and the category range of each kind.

Why. The Asian drama work (see the entry above) put four countries and a list of language words into `releases.rs`, the name `tvmaze` and the anime catalogue into `stream.rs`, and Prowlarr category numbers into `fallback.rs`. A new country or site then needed a build. Now it needs a publish.

How. `languages.rs` is the one place that reads the list: the subtitle sources, the country filter and the three release patterns all use it. The patterns that name languages are built from the document in use and kept until the document changes. `subtitles.rs` no longer has its own table. The server reads the category range with `sources::prowlarr()`, and the deploy script reads the site list from the published document.

Turned down. A second language table only for ranking: two lists would drift. Putting the ranking rules themselves in the document: they are engine behaviour and the README says so. Only their words moved.

Costs. A document without these parts uses the copy built into the app, so a server or app can ship before the publish, but the new words only reach users with the publish. Old apps ignore the new fields and keep their built-in words. The country words and the Prowlarr sites in the live document must be published before the deploy script can sync them. Words in a `languages` entry must be lower case letters and digits, so a pattern can never be broken by the document.

## Sources and the launch film leave this repo (2026-11-26)

**Chosen.** The plugin sources (`content/plugins`), the publish and tracker scripts (`content/sources`), the daily
`refresh-trackers.yml` workflow and the launch film (`content/manga/film`) were removed. `sources.json` is still
downloaded by `crates/core/build.rs`, now to `crates/core/sources.json` (git-ignored). The phone landing page keeps the
pictures that the film made.

**Why.** The sources and plugins belong to the `akidev-sources` repo, and the film was a one-time job. `content/` is for
what both phone apps share.

**Trade-off.** Nothing here refreshes the trackers each day or signs a publish any more. The built plugin files are in the
sources repo, but the plugins' Rust source is only in this repo's git history, before this commit.

## One copy and one set of tokens for both phone apps (2026-11-26)

**Chosen.** The words and the design tokens of the iPhone and Android apps live in `content/manga/`: `tokens.json` and one
`copy/<area>.json` per area of the app. `content/manga/generate.py` writes typed constants from them: `Copy.Home.title`
in Swift (`App/UI/Generated/Copy.swift`, `TokenValues.swift`) and in Kotlin (`AkiCopy.kt`, `AkiTokenValues.kt`). The generated files are
committed. `generate.py --check` and `check-copy.sh` run at the start of both release scripts.

**Why.** The two apps said the same things in two places, so a fix or a new word had to be made twice. One source also lets
the store text and the apps share words later.

**Turned down.** Native string files (`Localizable.xcstrings`, `strings.xml`): translation-ready, but every call site changes,
placeholders differ between the platforms and plurals need extra rules. The app is English only. Moving to native files later
is a change inside `generate.py` and one call-site pattern.

**Trade-off.** There is no translation support today. Plurals are English only (`one` and `other`). Where the two apps used
different words for the same thing, the owner chose one text for both (same day, commit "Make the iPhone and Android copy the
same"). Only three things differ on purpose: the reading font names (each platform has its own fonts), the 18+ footer in
Settings (`contentFooterNoSwitch` and `contentFooterWithSwitch`: Android has a switch, the iPhone has none) and the Android-only
font credit (`aboutFontCredit`). A few places in the code still compare against English text (`said.endsWith("is in your library.")` in `AppModel.kt`,
`contains("could not reach")` in `ShareModel.kt`): they break silently if that copy changes.


## The Android TV app (2026-11-26)

**Chosen.** A new app, `apps/tv-android` (Kotlin, Compose for TV, Media3 ExoPlayer), with its own small Rust crate,
`crates/tv-ffi` (library `akitv`), over `akidev-core`. It is the streaming product on a TV, and nothing from the manga
crates is in it. The Rust engine runs inside the app: there is no server to run. The viewer types their TorBox key in
Settings. The key stays in the app's private storage and goes only to TorBox. The engine asks TorBox for a link and the
player reads the link directly, so nothing is written to the disk.

**Why.** The first TV we tested (KTC Lumio Vision) is Android 14 but 32-bit only (`armeabi-v7a`). The Rust core builds for
it with no change, so the same search, catalogue and ranking code runs on the TV. The screens are new because the desktop
screens are gpui and cannot run on Android. They follow `design/DESIGN_PLAN.md`: same colours, same fonts (Shippori
Mincho for titles, Hanken Grotesk for the rest), amber only for focus and progress.

**Turned down.**
- Turning on the `streaming` feature of core for Android. It pulls in `librqbit`, which needs OpenSSL, and it would put a
  torrent engine on the TV that we never start. Instead `stream.rs` was split: `plan` (title to ranked releases) is now a
  free function that works without the engine, and only the engine parts stay behind `streaming`.
- Moving `apps/server/src/sources/debrid.rs` into core now. The TV has a small TorBox client of its own
  (`crates/tv-ffi/src/torbox.rs`). It copies the server's order of steps. Two copies is the price. Merging them is the
  next clean-up.
- A server on the viewer's computer. It would need the computer to be on.

**Trade-offs.**
- Only releases TorBox already holds are played (`add_only_if_cached`). A title that is not cached shows "TorBox does not
  hold a copy of this yet."
- The TV tries the releases the other apps hold back (HEVC, AV1) after the usual ones, because its chip decodes them.
  It has no DTS or TrueHD decoder, so a release with only those can play without sound. Not handled yet.
- The link carries the key, so it is asked for again on each play, never stored, and the Rust crate never logs it.
- A debug build (and only a debug build) reads `files/torbox.key` once so a test can set the key. A viewer types it in Settings.

## The TV home: top bar and a hero that follows focus (2026-11-26)

**Chosen.** Home has a glass pill bar on top (Home, Films, Series, Anime in the middle; Search and Settings at the right) and
one hero that shows the title the remote is on: backdrop, name, a line of facts, two lines of story, and the buttons. A film
has Play (warm white) and More info. A series or anime has one button, Episodes, because it cannot play without an episode.
The tabs filter the home rows by kind (each row carries its kind), so they make no new network call. The story is not in the
catalogue lists, so the hero asks for the title's page 0.4 seconds after the remote rests on it.

**Why.** Netflix and Apple TV use a top bar and it gives the rows the full width. A hero that follows focus (Plex, Stremio
Kai) means one big picture always names the title you are on, so the poster names under the tiles are gone. Research and
sources are in `design/TV_PLAN.md`.

**Turned down.** A left icon rail (Hotstar, MovieBox): it takes width from every row. A "Play" button on a series that
secretly opens the title page: it says one thing and does another.

**Trade-offs.**
- The image loader (`MainActivity`) allows 16 downloads per host. The default is 5, and all posters and backdrops come from one
  host, so the hero's backdrop waited about 30 seconds behind the posters on the TV. Now it takes about 1.5 seconds.
- Each focus change can start one story request after 0.4 seconds. Fast scrolling does not.
- The hero is a fixed 204 dp tall, so on a 540 dp screen the first row is fully visible and the second row's title peeks in.

## The TV player, history and key storage (2026-11-26)

**Chosen.**
- **Key.** Encrypted with AES-GCM using a key kept in the Android Keystore (`Key.kt`), in the app's private storage. If a TV cannot do
  that, Settings says so and the key is not saved. Nothing falls back to plain text.
- **History.** One JSON list in private storage (`History.kt`), at most 300 entries, one per film or episode. Past 90 % counts as seen.
  Continue watching is derived from it (one card per title). No database: it is small and read as a whole.
- **Player.** Media3 ExoPlayer with our own overlay. Audio tracks and subtitle tracks use its track selection. A subtitle from outside
  the file is fetched by core as WebVTT, saved in the cache and added by loading the same video again at the same place.
- **A copy that fails.** "Try another copy" asks the engine again with the failed release's hash in a `skip` list. The list lives only
  as long as the screen.
- **DTS and TrueHD.** Releases with only those sound go last in the list, because the TV has no decoder for them (checked: the decoder
  list of the Lumio Vision has AC3, EAC3 and AC4 but not DTS or TrueHD).
- **Release build.** R8 shrinks it (11 MB). JNA and the generated `uniffi` classes are kept by name because they are found by
  reflection. It is signed with the same upload key as the manga app when `~/.akidev-keys` exists.

**Turned down.**
- A pairing page so a phone can type the key: a small web server in the app for a job the Android TV Remote app already does.
- Plain `SharedPreferences` for the key: private to the app, but readable on a rooted TV or a backup. Backups are off, yet the
  encrypted form costs about 40 lines.
- Showing the file name or the release name in the player: it would be the one place a link or a release title could reach a screen.

**Trade-offs.**
- Picking a subtitle from outside restarts the stream for a moment (same place).
- Subtitles use a fixed look (white, black edge, the app's font, 22 sp), not the TV's caption setting: the TV's own style made the
  text unreadable in a test.
- The preferred subtitle language is the TV's language. The viewer's choice is not remembered between plays.

## Icons first, text only when the icon is not enough (2026-11-26)

**Chosen.** The same Solar icon set as the phone apps, written to the TV by `content/manga/fetch-icons.py --tv` (30 icons, the list
`TV_USES`; the phone apps' icons are not touched by that option). The top bar is only icons. The name of one opens beside it while the
remote is on it. The shown tab has the filled icon and the amber line. Buttons are icons too: Play, More info, Search, Back, Try again,
the subtitles and next-episode buttons in the player. A text stays where it carries information the icon cannot (the episode and the time
on the Play button, an episode name, the story). A spinner (the refresh icon, turning) says "working", a warning icon heads every
failure card, a check icon marks the chosen track and a seen episode.

**Why.** A TV is read from a sofa: an icon is read faster than a word, and the text of every button took room from the pictures.

**Trade-off.** An icon alone must be understood. Where it is not (the list icon for Episodes, the check for "Save and check") the text
stays (`labelAlways`).

## Why the TV app was slow, and what fixed it (2026-11-26)

Measured on the KTC Lumio Vision (3 weak CPU cores, a PowerVR BXE-4-32 graphics chip, a 4K panel fed 1080p) with a release build and
`apps/tv-android/scripts/bench.sh`. The system's own frame numbers (`dumpsys gfxinfo`) and a system trace (Perfetto) pointed to the cause;
each guess was tried on its own and kept only if the number moved.

| What | Before | After |
|---|---|---|
| Median frame while scrolling Home | 61 ms | 14 ms |
| Frames that missed their time | 47 % | 20 % |
| 90th percentile frame | 93 ms | 34 ms |
| Home rows on screen (second launch on) | 2.1 to 4.7 s | 0.92 to 1.03 s |
| First frame after launch | 410 ms | 420 to 490 ms (the same) |
| Find a copy after pressing Play | 5.4 s | 3.1 s |

**The big one: two full-screen fades.** The picture behind the hero had two gradients as large as the screen, one over the other, blended
in every frame. On this graphics chip that alone took a frame from 10 ms to 61 ms (tested by switching each part off: no backdrop 10 ms,
picture alone 11 ms, fades alone 61 ms; the rows and the posters made no difference). The fade is now one small picture (96 by 54 pixels,
made once) stretched over the screen: the same look at 14 ms.

**Catalogue answers kept on disk.** Core already could (`providers::keep_answers_in`, the desktop and the server use it) but the TV
never turned it on, so each launch waited for ten rows from the network (1.5 to 4 s). Now the second launch shows the rows of the first
in about 290 ms and refreshes them in the background. The very first launch still takes about 3.4 s.

**The search for releases starts when the title page opens** (`prepare_stream`), so it is answered (core keeps it) when Play is pressed:
from 1.5 to 2.6 s to about 0.5 s.

**Tried and not kept.** Fading between two backdrops (no gain when removed), holding the hero back until the remote rests (kept for
the picture churn, but no gain in the number), replacing the TV library's `Surface` with a plain box (57 ms against 65: kept, it is
simpler and cheaper), compiling the app ahead of time (950 to 760 ms to rows on screen, so a baseline profile would gain at most
about 190 ms: not worth a profile project), starting to play after 1 s buffered instead of 2.5 s (no gain; the network varied more).

**Not solved: the start of the video.** After the copy is found, the player needs 10 to 12 more seconds to show the first picture. A
trace of the bytes shows about 3 s with no data (opening the connection on the TV), then about 40 MB read before it is ready, at about
5 MB/s: the big files carry a lot to read before the first picture. It is the same for the first test on this TV, so it is not new.
Ideas not tried: the OkHttp data source with a connection opened early, and preferring releases with smaller headers.

**The TV's accessibility service** ("Home button" of the TV maker) is switched on for every app, so Compose keeps its accessibility
tree in step on every frame (about 10 % of the main thread). That is the TV's setting.

## Nothing moves: fixed places for everything (2026-11-26)

**Chosen.** Every screen gives each thing a place of fixed size, and what is still loading is drawn as a quiet shape in that place.
- **Home.** The hero has four slots of fixed height (name 50 dp, facts 22, story two lines 52, buttons 48). A title with no story or no
  facts leaves the slot empty and the buttons stay where they are. While the rows load, the screen shows the real top bar and shapes
  with the size of the hero and of two rows (`HomeScreen`, `placeholder`), not a card that is replaced later.
- **Title page.** It opens at once with the name, the facts and the picture of the card the viewer opened (`Screen.Detail` carries the
  `Title`). Story (4 lines), main button, seasons and episodes each have a slot of fixed size with a shape in it until the page has loaded.
- **Buttons and the bar.** A button with an icon never grows. Its name floats in a small tip under (or over) it while the remote is on
  it (`Tip`, drawn over the screen and taking no room). Before, the label opened inside the button and pushed its neighbours aside, which
  was the sudden jump when moving the cursor. The shown tab is the bright filled icon; the underline is gone.
- **Text lines.** Every text style has a fixed line height (`Theme.kt`), so a line takes the same room whatever it says; the skeleton
  and the real text are the same height.
- **Search and Settings.** The message line (searching, nothing found, failed, key check) has its own slot.
- **Motion.** Pictures fade in when they arrive, the hero text cross-fades in its fixed slot, the tip fades and scales in, the player's bars
  fade, and its panel slides in from the right. None of these changes any size.

**Turned down.** Letting a label grow inside the button but reserving its width: it wastes room beside every icon.

## Pictures and stories that did not appear (2026-11-26)

**Cause (stories).** The home lists carry no story, so the hero asked for the title's page each time the remote rested on a card. AniList
answers 30 requests a minute; scrolling an anime row used more, and the refused answers left the story empty for some titles. The desktop
asks only for the title that is open. Now `Story.of` asks for one at a time, at least 2.1 s apart, and keeps the answers.

**Pictures.** A fresh run loaded every picture on the TV, so the failures need a condition. Three guards: the image client has longer
timeouts (15 s connect, 30 s read) and asks again once after 1.5 s (`RetryOnce`); a backdrop that cannot be loaded shows the poster instead;
a tile that cannot load shows the title's name.

## The TV follows the same definitions as the web and the desktop (2026-11-26)

The TV already used the same live signed document (`sources::keep_fresh` downloads it at start and every 6 hours, as the desktop and the
server do). What was fixed here: the tabs were four fixed ones, and the screens were asked for once at start, so a tab or row someone added
to the definitions did not show until the next launch. Now the tabs come from the definitions (`tabs()`: their order and names; an unknown
kind gets a neutral icon) and the app checks `sources_version()` every 20 seconds; when newer definitions have been downloaded it asks for the
rows again.

## The start of the video (2026-11-26)

Measured on the TV: a film is ready 3 to 4 s after the player starts, an anime fansub release 9 to 10 s (it has to read a lot of embedded
data before the first picture; about 40 MB at about 5 MB/s). So the player is not slow in itself: the file is. Using the OkHttp data source
for the video (the same client family as the pictures) cut about 1.3 s. Starting to play after 1 s of buffer instead of 2.5 s gave nothing.
Not tried: preferring releases with smaller headers.

## Why pictures were slow: a dead IPv6 address (2026-11-26)

Timed on the TV (a log of each picture's time and where it came from): pictures from `images.metahub.space` and `live.metahub.space` took
15.5 seconds each, and 15 seconds was the connect timeout of the image client. The host gives an IPv6 address that does not work on this
kind of home network, and the client waited the whole timeout on it before trying the IPv4 one (then the retry was instant). Now the
clients try IPv4 addresses first and wait at most 8 seconds (`HttpClient`, for the pictures and the video).

| Picture | Before | After |
|---|---|---|
| metahub poster, from the network | 15.5 s | 0.6 s |
| AniList poster, from the network | 1.6 s | 0.25 s |
| First poster on screen, cold start | 9.8 s | 3.1 s |

## Motion (2026-11-26)

Taking out the buttons that grew in place (to stop layout jumps) had taken most of the motion with them, and tabs and screens simply cut.
Now: a screen that opens slides in a little from the right while the old one fades, and Back reverses it (`AnimatedContent` in
`MainActivity`); switching tab cross-fades the hero and rows while the bar stays; a highlight glides under the shown tab and its
icon cross-fades from the outline to the filled one; the focus ring fades in and out and the grow is a soft spring; shapes fade into the
rows, the story, the button and the episodes when they arrive; the player's bars fade and its panel slides in. All of it is fades, slides
and scale: none changes the size of anything. The TV's own animation setting (here 0.5, set by the maker) halves every Compose animation, so screens and tabs felt like cuts
(a tab change measured 0.27 s against 0.37 s at 1.0 by recording the screen). `Motion.kt` divides each duration by that factor, so
the times written in code are the times people see; a setting of 0 still turns animation off. All curves are ease-out
(`0.2, 0, 0, 1`) with no bounce, and a press scales to 0.97. Pictures from `http://` addresses are upgraded to `https://`
(`UpgradeToHttps`); new providers need no new APK as long as their pictures are served over HTTPS.

## One bar for every place (2026-11-26)

Search and Settings used to be screens pushed over Home, so the bar vanished and going there was a cut. Now the bar lists the tabs,
then Search and Settings, in one pill, and it stays on screen. Picking one swaps the first screen of the stack (`pick` in
`MainActivity`), which cross-fades (340 ms in, 200 ms out) while the highlight glides to the new icon. Back on Search or Settings goes
to Home; Back on Home leaves. The bar is hidden on deeper screens (title page, player). Three fixes came with it: the floating `Tip`
had been widening its button while shown (so icons next to it shifted 20 dp; it now takes no room), the highlight is `Ink.Line` so it
can be seen, and after a pick the focus is put back on the chosen icon (swapping the screen had thrown it to Home). The search field
and the key field no longer grab focus by themselves: press Down from the bar.

The list on Home now keeps 48 dp under the row the remote is on (`roomBelow` in `Components.kt`, used for each row in `HomeScreen`). Before, a focused row only scrolled until its cards touched the bottom edge, so at the end of the list they looked cut off. `contentPadding` could not fix it, because scrolling to a focused card ignores it.

## Home: no buttons in the hero, rows move one at a time (2026-11-27)

Why the buttons went: pressing OK on a card already opens it, so "Episodes", "Play" and "More info" repeated what OK does. The hero now only names what is in focus: title, facts, two lines of story. That follows the shots on Dribbble and the Apple skill (the picture sets the mood, nothing extra on screen, one lit element: the focused card). Because the hero is 56 dp shorter, the list gets more room. The title also has more width (it was cut with "..." at 640 dp) and a little tighter letter spacing (-0.5 sp), as large text should have.

Rows now move by whole rows. When the remote lands on a card, the list scrolls until that card's row stands at the top, title included, and the next row's title peeks in below it. Before, the list scrolled only until the card touched the screen edge, so a row title was cut at the top and cards were cut at the bottom. How: `RowAtTop` (a `BringIntoViewSpec`) lines up the top of what is asked for with the top of the list, and `wholeRowInView` makes each row ask for its whole height, not just the card. Inside a row the old rule stays (`LeastToShow`). Tried first and dropped: calling `animateScrollToItem` from the card, because Compose's own scroll-to-focus cancelled it. The list has 96 dp of space at its end so the last row can also reach the top.

## Title page and player, same rules (2026-11-27)

These two screens had not been reviewed against the Dribbble shots, the Apple skill, better-ui and better-layout. Fixed:

- Title page: the title has the whole width (a long name was cut at 640 dp; the story keeps 640 dp for a readable line). The main button always shows its words ("Play", "Episode 3 · 12:40"), not just an icon. Only the main button is filled: the season buttons were filled too, so two things looked like "the" button. Seasons now show only when there is more than one (outlined = chosen, plain = not), and a heading "Episodes" always shows. An episode card says "EPISODE" over its number, so it no longer looks like an empty box.
- Player panel: each group has a heading with words ("AUDIO", "SUBTITLES"), not only an icon. The choice in use has an amber tick and a raised background; before, it was a filled white bar, which fought with the focus ring. Subtitles from outside show only their language (a second one in the same language gets a number), not the release's file name. The time under the bar is brighter, for contrast over the picture.
- Not changed: the player bars and the pause icon (already quiet), and episode pictures (the catalogues give none, so the cards stay text only).

## Choosing the source in the player (2026-11-27)

Before, the app picked one copy by itself and the viewer could only press "Try another copy" after a failure. Now the player's panel (button "Options") has a third section, SOURCE, under Audio and Subtitles. Each line is what tells copies apart: "1080p  ·  H.264  ·  1.7 GB" (and "No sound on this TV" for DTS or TrueHD copies); two that look alike get a number. The copy in use has the amber tick.

How: `sources(id, key)` in `crates/tv-ffi/src/lib.rs` runs the same steps as `find_stream` (the releases, then which ones TorBox holds and that have a file for this film or episode) and returns up to 10, best first. `find_stream_of(id, key, hash)` plays exactly one of them. The steps they share are in `candidates()` and `Candidates::has_file()`. The list is asked for only when the panel opens (it costs calls to TorBox, about 1 to 1.6 s), and only once per film or episode. Picking one saves the place, finds that copy, and the player starts again from the saved place (about 3 s to a link, then the video's own start). Subtitles from outside are now at most 6 lines, so the source section is not far below them.

## Speed, second pass (2026-11-27)

Measured on the TV (release build, `apps/tv-android/scripts/bench.sh`), before and after each change. The point: find what really costs time, change that, and keep a change only if the number moves.

| What | Before | After | Change |
|---|---|---|---|
| Warm start: first frame | 465 ms | 335 ms | baseline profile |
| Warm start: rows on screen | 1,020 ms | 550 ms | baseline profile |
| Cold start (no cache): first row on screen | 3.5 to 4.1 s | 1.5 to 2.0 s | rows one by one, first 3 first |
| Scrolling: janky frames | 6.9% (p99 125 ms) | about 2.2% (p99 55 to 65 ms) | baseline profile |
| Java heap, after 8 rounds of scrolling | 8 to 13 MB, no growth | same | (no leak found) |

**Baseline profile (the biggest one).** The TV has three slow cores. Without a profile the system compiles the app's code while the viewer uses it (the trace showed the JIT thread using 515 ms of CPU in the first seconds, and "lock contention on the JIT code cache" inside slow frames). `app/src/main/baseline-prof.txt` lists what to compile ahead of time: the app, the engine bindings, Compose, the image loader, the network and the player. Play applies it at install. A copy put on the TV with adb applies it on the first run, and the system compiles it later, so after `adb install` run once: `adb shell cmd package compile -m speed-profile -f live.akidev.tv`. (Compiling everything with `-m speed` gave the same gain, so the profile reaches the limit.)

**Home rows one by one.** Before, `home()` fetched all 10 rows (200 titles) together and showed nothing until the slowest was back. Now `home_rows()` gives the names from the definitions at once and `home_row(i)` gives one row. The app asks 3 at a time, in order, and a row is shown only when every row before it has an answer, so a late row never pushes the others down. Skeleton rows show for the rest. A failed row is left out; all failed shows the "Try again" card.

**Old content shows at once.** The disk cache showed an answer only if it was under 24 hours old; past that, a start waited for the network as if the cache were empty. It now shows answers up to a week old and refreshes them in the background (they are fresh at the next start), and files are kept 8 days.

**Looked at and not a problem, or not worth it.**
- Memory: after scrolling, graphics memory is 170 to 200 MB (about 90 MB are the window's buffers, the rest the system's GPU texture cache, which it frees when memory is short; it peaked at 290 MB and fell back to 205 MB). Capping Coil's picture cache to 16 MB, storing posters at 2 bytes a pixel and decoding backdrops at about half size each changed nothing measurable, so none of them was kept.
- Garbage collection: 34 runs in 8 rounds of scrolling, the longest pause 95 microseconds.
- Cinemeta answers through a redirect (the first host answers 307 to another), which doubles the time of those requests (0.54 s against 0.27 s). The address is in the signed definitions and the target has another path layout, so it is left for the definitions' owner. Its catalogues also hold 50 titles with descriptions and cast (up to 111 KB) and there is no way to ask for fewer; the app uses 20.

## Search results (2026-11-27)

The TV keyboard closes itself when its search key is pressed (the app has no code that hides it). So the first result takes the focus when results arrive (`PosterCard(autofocus = ...)` in `SearchScreen.kt`) and the viewer can browse with the D-pad at once. The grid runs to the bottom of the screen with its own space at the end, and a poster with its name under it asks the list to scroll 56 dp past it (`roomForName` in `Components.kt`), so the last row's names are not cut off.

## Suggestions while typing (2026-11-27)

Like the desktop's search palette (250 ms pause, the newest word wins), but words only: after 450 ms without typing, and from 3 letters, the app asks `search()` for the word and shows up to 6 names in a row under the field (`SearchScreen.kt`, `MainActivity.kt`). The results grid does not change while typing; it changes when the viewer sends the search. OK on a suggestion puts it in the field and searches for it. The row shows only while the field differs from the last search, and it sits above the TV keyboard (which covers the lower half of the screen), in the same 48 dp slot as the messages, so nothing moves.

**Why not in the keyboard's own suggestion strip.** The suggestion strip belongs to the keyboard app and shows what its own dictionary offers (on this TV: "Frier, Frieren, Friers"). Android has no call for an app to add words to it. The only ways in are an Autofill service or a system spell-checker service: both are chosen by the viewer for the whole TV (not per app), the first is for saved logins and addresses, and TV keyboards do not draw inline autofill suggestions. So the app draws its own row.

## Definitions: stale while revalidating (2026-11-27)

This replaces how the TV used new definitions before (it swapped them in while the app ran and reloaded the rows, polling every 20 s).

- **At launch** the app uses the definitions it has: the copy saved by an earlier run (checked again against the signature), or else the built-in one. It does not wait for the network, and the tabs and rows never change under the viewer.
- **In the background** it asks `sources.akidev.live` at once, again every 6 hours, and again when the app comes back to the screen (not more than once a minute: `check_definitions`). A copy is used only if its signature is valid, its version is higher and it passes `validate` (unchanged). With `apply_updates_at_next_start()` (called by the TV's `start`) such a copy is only saved, and noted in `waiting_version()`. The same copy is not fetched again.
- **At the next launch** `load_saved` finds it and uses it.
- **The sign.** `definitions_waiting()` turns true when a copy is saved. The app then shows a small amber dot on the Settings icon, and Settings says "New sources are ready. They apply the next time the app opens." The app reads this every 5 s until it is true (a number in memory, no network). Nothing else changes on screen.
- The other apps (desktop, web, server) are not changed: they still use a new copy at once, because only the TV calls `apply_updates_at_next_start()`.
- The built-in copy stays as the safety net for a first start with no network (and for the tests of the other apps). It is the same copy the build downloads from the sources repository, so it is not edited by hand. Everything you add to the published sources reaches an installed TV without a release: it is fetched and saved, and used at the next start.

## Search field and the keyboard (2026-11-27)

- **The keyboard opens only on OK.** A text field takes the focus and opens the TV keyboard in one step, so the keyboard opened by itself whenever the remote passed over the field (for example going Up from the suggestions to the bar). `Field` in `Components.kt` is now two things: at rest a plain box (same look, ring when the remote is on it) and, after OK, the real text field with the keyboard. When the remote moves away it rests again. The suggestion row and the results still hold the remote as before. Going Up from a suggestion still passes over the field (now with the keyboard shut) on the way to the bar.
- **Typing no longer stutters while suggestions load.** The keyboard is another app on the same three slow cores. The engine now has 2 worker threads at a lower priority (nice 10, `lower_priority` in `crates/tv-ffi/src/lib.rs`), the suggestion call itself runs at background priority, and the pause before asking is 600 ms (was 450). Checked: the engine threads show nice 10 against -10 for the main thread. The stutter itself was not measured (no number for the keyboard), only judged by hand.

## Releasing the TV app and updating it from GitHub (2026-11-27)

**Making a release.** `fastlane release` (run in `apps/tv-android`, see `fastlane/Fastfile`). It checks the branch is `main` and clean, works out the version (the first release uses the version in `app/build.gradle.kts`; later ones bump the last `tv-android-v*` tag: `bump:patch` by default, or `bump:minor`, `bump:major`, `version:1.4.0`), builds the engine and the app, runs the unit tests and lint, checks the APK is signed, writes the new version into `build.gradle.kts`, commits it ("Release Akidev TV X"), pushes, and creates the GitHub release `tv-android-vX.Y.Z` with `AkidevTV-X.Y.Z.apk`, its SHA-256 and the notes. The notes are the subjects of the commits since the last release that touch `apps/tv-android`, `crates/tv-ffi`, `crates/core` or `design/TV_PLAN.md`. `fastlane release dry_run:true` does everything except commit, push and publish (it builds the APK and prints the notes); `fastlane notes` only prints the next version and the notes; `draft:true` makes the release a draft.
- Needs `GITHUB_API_TOKEN`: a token of the account that owns the repository (`dishant0406`) with contents write on it. Not `gh auth token`: `gh` here is logged in as the work account.
- Tags are `tv-android-v...` because the repository already has `desktop-mac-v...` and `desktop-windows-v...`.
- The version code goes up by one each release (Android refuses an update that does not have a higher one).

**Why GitHub releases for the check.** The repository is public, so the TV reads `api.github.com/repos/dishant0406/akidev/releases` with no token and no extra service (60 requests an hour per address; the app asks at most every 30 minutes). A private repository would need a token inside the app, so it would need another place for a small JSON file (the sources feed could carry one). `UPDATE_URL` in `app/build.gradle.kts` says where; tests can point it elsewhere with `-PupdateUrl`.

**In the app** (`Updates.kt`, `SettingsScreen.kt`, `MainActivity.kt`). Like the sources: the answer of the last check is read at once at the start, and the check runs in the background at the start and when the app comes back to the screen. The newest published release whose tag starts with `tv-android-v` and that holds an APK counts. When it is newer than the installed version, the amber dot shows on Settings (the same dot as for new sources) and Settings shows "Version X is ready." with "Download and install". The button streams the APK into Android's installer (`PackageInstaller` session), so no APK file is kept on the TV (there is nothing to delete afterwards), checks the SHA-256 when GitHub gives one, and Android shows its own "Do you want to update this app?" with Update and Cancel (Cancel has the focus: press Left, then OK). Android installs it only when it is signed with the same key as the installed app, so nothing else can replace the app.
- The first time, Android must allow this app to "Install unknown apps". The button opens that screen (Akidev TV is selected: press OK, then Back) and says "Allow this app to install updates, then press the button again."
- After the update Android closes the app and the TV shows its home screen: the viewer opens the app again. Opening it by itself is blocked (Android 14 logs "Background activity launch blocked", tried with a `MY_PACKAGE_REPLACED` receiver and removed); the only way round is the "display over other apps" permission, which asks more of the viewer than it saves.
- Problems (download failed, damaged file, the installer's own message) show in place of the sentence, with the button to try again.
- Tested on the TV against a local server that served a fake release list and a 1.0.1 APK: the sign, the sentence, the permission step, the progress ("Downloading 35%"), Android's dialog, and the new version installed (version code 2). The real GitHub release was not made.

**Two bugs this found.** A scroll wrapper on the Settings page swallowed the Down key from the bar (removed). While the key field was being edited, the Up key stayed in the field, so the remote could not leave upward (Up now moves on, like Down).

## TV Settings: the key can be shown, and the notices come last (2026-11-27)

The TorBox key field on the TV shows dots. A mistyped key was hard to find, so an eye button now sits beside the field. OK on it shows the key as text, and OK again hides it. It is a separate button, not part of the field, because the field keeps the Left and Right keys for its cursor while you edit. To reach the eye, leave the field first (Down, then Up, then Right).

The notices (new sources, a new app version) moved below the result of the key check. Before, their empty slots left a big hole between the text and the field, and pushed the result line off the bottom of the screen, where it was cut. Now nothing sits between the text and the field, and a notice that appears later only moves things below it. The icons "eye" and "eye-off" come from Solar through `content/manga/fetch-icons.py --tv`.

## TV: View all, filters, episodes that are not out yet, and a calmer focus (2026-11-27)

Five things the owner saw on the TV, fixed together because they share the same few files.

**Episodes: out or not.** Before, every episode looked playable, even ones that had not aired, and play failed for them. Now each episode has a `released` flag, worked out in Rust (`detail` in `crates/tv-ffi/src/lib.rs`), so the cards, the main button and the player's Next button all agree. The rule: an episode is out unless its air date is a real date after today, or (for an anime still airing) its number is the next one to air or later. A card that is not out is dimmed, shows a clock with its date ("4 DEC", or "SOON" when the date is not known), and OK does nothing. The header says "13 of 25 out" when some are not. The main button and Next skip them, and never pick them. The date rule moved from `stream.rs` into `calendar::is_future_date`, so the card and `find_stream` cannot disagree. `stream.rs` also refuses an anime episode from the next one on ("This episode has not aired yet.").
- **Why a new field on `Media`.** AniList gives the total episodes and, separately, the next episode to air. The old mapping folded them into one number, so the app could not tell which were out. `Media.next_episode` carries the second one (`#[serde(default)]`, so the cache on disk and old documents still read). It is mapped from `nextAiringEpisode.episode` by a new `next_episode` line in the anilist card of `sources.json`. An anime with no total (only the next episode known) still lists just the episodes that are out, as before, so nothing shows as "not out" there.
- **Publish order.** The new line must be in the published `sources.json` before the TV can use it from the network. An app with the old document just shows every episode as out, as before. Nothing breaks. Old apps ignore the unknown line (the structs do not reject unknown fields).
- **Alternative left out:** listing only the aired episodes for every anime. It hides the future episodes instead of marking them, and changes the count shown on every card.

**Back from the player lands where you were.** The title page used to leave the screen when the player opened, so its episode row started again at the beginning and the main button took the focus. Like the home rows, the app now keeps a `DetailState` for each title page that is on the way back (`DetailMemory` in `DetailScreen.kt`): the episodes' scroll, the season, and the episode the remote was on. The last answer for the title is kept too, so the row has its items at once and keeps its scroll while the page asks again. When you leave the title page for good, its memory is dropped, so the next visit starts at the top.

**The focus ring is not cut off.** The focused card grows 5 %, and the row clips what grows past its edge. The row scrolled the least that showed the card, which left the card flush with the edge, so its ring was cut. `LeastToShow` now keeps a 12 dp margin (`rememberLeastToShow`), and the search grid and the season buttons got room at their edges. Checked on the TV: the card at the right edge of a row shows its whole ring. Before this it was cut.

**View all, and Filters.** The last card of every home row is "View all". It opens `BrowseScreen.kt`: the heading, the sort buttons (Popular, Trending, ...), a genre row, a grid of six, and a "Show more" button under the last title, like the website. It starts with the row's own kind, sort and genre. A "Filters" button next to the top bar (Right from Settings) opens the same screen for the current tab, or for all three catalogues on Home. Several decisions:
- **One screen for both**, and the sorts and genres come from the definitions (`browse_options`), as on the website, so nothing is written in Kotlin.
- **All catalogues together** is the same merge the server does (`interleave` over each kind's page), in `browse` in `lib.rs`.
- **The screen keeps its own state** (`BrowseState`, inside the `Screen.Browse` on the stack): the filters, the pages loaded, the scroll and the title the remote was on. Back from a title lands on the same poster. A title that comes on two pages shows once (the order shifts between asks, and a repeated key would crash the list).
- **"Show more" stays in place while the next page loads** (its label says "Loading"). First it was swapped for a spinner, and the remote lost its focus and jumped to the sort row.
- **A filter change keeps the remote on the button** you pressed. Only the first visit puts the remote on the first poster.
- **Bug found while testing:** `titles.first()` on an empty list crashed the app when a genre was picked, because the grid's list can run before the screen switches to its loading view. It uses `firstOrNull` now.
- New Solar icons for this: "filters" and "more" (`content/manga/fetch-icons.py --tv`).

## TV: Browse uses the same bar as the rest of the app (2026-11-28)

The first Browse screen had two rows of text buttons (the orders, then the genres) and a square Filters button next to the top bar. It worked, but it looked like a different app. The owner said so. So it was drawn again.

**What was chosen.** Browse now has the same glass pill at the top as Home. Basically it is `TopBar` used a second time (`TopBar.kt`), with other items: one icon for each way to order the list (fire for Popular, a small chart for Trending, a star for Top rated, a calendar for Newest, the sort arrows for A-Z), then one Genre icon. The chosen order has the filled icon and the highlight glides to it, as a tab does. The name floats under the icon the remote is on.

**Why icons for the orders but a panel for the genres.** There are five orders and they are fixed, so each can have an icon (`sortIcon`, with a plain list icon for one a plugin brings). There are about twenty genres and they come from the definitions, so they cannot have icons. The Genre icon opens a panel at the right. It is the panel the player uses for audio and subtitles (`PanelLine` in `TracksPanel.kt`): a list, with an amber tick on the chosen one. An amber dot on the Genre icon says a genre is chosen, because amber is for state.

**The words stay.** Icons alone do not say what is chosen when the remote is down in the grid. So the right end of the band says it in two small lines, for example "TOP RATED" and "FANTASY". The left end says the catalogue ("Series", or "Browse" for all of them). The heading no longer copies the row name: "Trending Anime" was wrong the moment you picked Popular.

**Show more looks like the website.** Under the last title there is a hairline with "30 SHOWN" in the middle, then the "Show more" button, centred, with a down arrow. The website and the desktop app draw it the same way (`design/shots/web-load-more-after.jpg`). It is still a button you press, not an endless list, because that is what the owner asked for.

**Browse is in the main bar.** The square Filters button is gone. The main pill has a Browse icon (a grid) between the tabs and Search. It opens the list of the tab that is shown. From Home, Search or Settings it opens all catalogues. It does not become the "shown" tab: it opens a screen on top, and Back returns.

**Alternatives rejected.**
- A left rail of filters (Plex does this). We have no rail anywhere, and the design plan chose the top pill.
- Genre chips in a second row. That was the first build, and it is what looked bad: a long row of text that scrolls sideways.
- A poster-size "Show more" card as the last cell. It would look like the "View all" card, but the owner asked for the website's pattern.

**What it cost.** `TopBar` got three small options: `badge` (which item has the dot), `actions` (items that open something and are not "shown"), and `requesters` (so a screen can give the focus back to an item). `Heading` and `Line` of the player panel are now `PanelHeading` and `PanelLine`, shared.

**References looked at (Dribbble, Behance, Google Images, 2026-11-28).** tabii TV (Behance): text tabs and a list at the side, nothing else on the control line. Clipsee Android TV case study (Behance): a side panel with one tick for a single choice, and tabs in place of filter chips. Plex library view (Plex Support): one control line over the grid, and a list with ticks for sort and filter. "Movie Streaming App, Dark Mode Filter UI" (Dribbble, phone): genres as a group of their own, apart from sort. Entertainment Dashboard by Terabit (Dribbble): the filter is one icon beside the tabs. Vision Stream Smart TV (Dribbble): a single top line of navigation over the rows.

**Also fixed.** On a title page the season buttons (S1, S2...) were cut at the top and the bottom. The row was 48 dp and the buttons need 56 dp since they got room to grow. It is 56 dp now.

**The panel no longer pulls the remote back up (2026-11-28).** In the player panel (audio, subtitles, source), going down the list sometimes threw the focus back to the top. The reason: the first line asked for the focus every time it was made, and a lazy list makes a line again each time it scrolls back into view. Now the panel remembers that the focus was placed (`placed` in `TracksPanel`), and the first line asks only once, when the panel opens. The genre panel of Browse has the same rule. Checked in the player on the TV: 12 lines down, 8 up and 5 down again, and the focus stayed on the line the remote was on.


## Release dates and "Coming soon" (2026-11-28)

A viewer opened a film that was in the catalogue and got "no source". The app could not say why, because a card only had a year. Now every card has a real date, and the app uses it in three places.

**Chosen.** `Media` has a new field, `release_date` (`crates/core/src/model.rs`). It is text: `2027-01-09`, `2027-10` or `2027`, as exact as the catalogue is. Each provider card in `sources.json` (akidev-sources repo, version 52) maps it. AniList gives the date in three parts, so there is a new expression, `date`, that joins them (`Operation::Date` in `crates/core/src/definition.rs`, worked out in `crates/core/src/expression.rs`).

- **Why text and not a number.** Half of the upcoming anime on AniList have only a year, or a year and a month. A number would force us to invent a day. Text keeps what the catalogue really knows.
- **Why `date` gives a shorter date instead of nothing.** With no day it gives `2027-10`, with no month `2027`. "Coming October 2027" is more useful than no date at all.

**One rule for "not out yet", in core.** `Media::coming_soon(now)` returns the words to show, or `None` when the title is out. A title is not out when its `release_date` is after today (`calendar::is_upcoming`: a month or a year counts only when all of it is in the future), or when its status is `NOT_YET_RELEASED` (then the words are "Coming soon", because we have no date to trust). A full date that has passed always means out, even with that status: a stale status must not block a title on its release day. All three apps ask this one function, so they cannot disagree.

- The server sends the words as `coming_soon` on every card (`apps/server/src/dto.rs`), next to the raw `release_date`. The website only draws them. **Why:** the website would otherwise need its own date maths, and the server and the browser could land on different days.
- The TV bridge does the same on `Title` (`crates/tv-ffi/src/lib.rs`).
- The desktop app calls `coming_soon` itself, because it has core inside it.
- **Rejected:** each app comparing dates on its own. Three copies of one rule, in three languages.

**What a title that is not out does.** Play is refused in core (`validate_request_at` in `crates/core/src/stream.rs`): "This title is not out yet. Coming 27 January 2027." So no app can start a search for it, even from an old link. On the screens: the card's facts line shows the words in place of the year, Play is replaced by the words, a film's poster opens the title page and not the player, and the episode list is hidden (web, desktop) or dimmed (TV, where `released` is false for every episode).

**The film that started this.** It was not "coming soon". It opened in Japanese cinemas eight days before, so its date was in the past, and no copy exists until the home release. No catalogue we use knows a home release date. So there is a second, softer rule: `Media::recently_out(now)` is true for 120 days after a full release date. When nothing plays for such a title, `Plan::with_too_new_note` adds one sentence to the reason: "… This title came out on 20 November 2026, so it can be too new to have a source." It is added when no release is listed, and also when releases were found but none played. The second case is the real one for this film: the search finds two torrents of the game with the same name.

- **Why 120 days.** A film usually reaches home release two to four months after cinemas. It is a guess, in one constant.
- **Trade-off.** The message is only a hint. An old film with a wrong date, or a new series that really has no source, gets the same words.
- **Trade-off.** Kitsu dates are in UTC and can be one day early. Kitsu is only the mapper card, so no screen shows its date.
- **Trade-off.** Days are counted in UTC, like the episode air dates. Near midnight a title can turn "out" a few hours early or late for a viewer.
- **Trade-off.** The website gets the words from the server, and the answers are cached: a title page for 15 minutes and a list for 5 minutes, and each can be served stale for one more hour while it refreshes. So on release day the page can say "Coming" for a short time after midnight UTC. Play itself is never stale: core checks the date on each request.
- **Not done.** The catalogue lists still hide most upcoming titles (`drop` rules with `after_today` in `sources.json`, and the AniList `recent` sort). An upcoming title is found by search, by a link, or in a related row.

## The home page: a fixed order, and "Top 10" rows (2026-11-28)

The home page felt cluttered. It had ten rows, every row the same shape, and the type changed on every row: anime, film, series, anime, film... A viewer could not learn where to look.

**What I looked at first.** Netflix, Prime Video, JioHotstar, ZEE5, Apple TV, and Google's own TV design rules. Three things were the same everywhere. Each row has one idea. The personal row ("Continue watching") comes first. And one row type looks different from the rest: Netflix has "Top 10" with big numbers. Netflix also moved the facts of a title from the top of the screen to under the tile, because eyes had to travel ("gymnastics", in their own words). Our TV home already does the safe version of that: the focused row always sits right under the title facts, so I kept it.

**Chosen.** Rows keep one kind each, and they alternate between two looks.

- Core has a new row key, `style` (`RowStyle` in `crates/core/src/screens.rs`). `ranked` is a "Top 10": the first ten titles (`RANKED_LEN`), each with a big number beside the poster. No `style` means a plain row.
- The order is in the sources document, version 53: Top 10 anime now, New films, Top 10 series, New anime, Top 10 films, New series, then the genre rows (Action films, Romance anime, Drama series), then Top rated anime. Rows are named for what they hold, in one pattern.
- Every app draws a ranked row the same way: the server sends `style` and cuts to ten (`RowStyle::trim`, used by the server, the TV bridge and the desktop app), the website and the desktop app draw the number beside the tile, and the TV has `RankedCard`.

**Why a `style` key and not a new kind of row.** An old app ignores a key it does not know, so it draws a ranked row as a plain row and still works. Old apps refuse the whole document for a value they do not know. So I did not add a new sort value, and I checked version 53 with the checkers of `desktop-mac-v0.1.27`, `desktop-mac-v0.1.36`, `desktop-mac-v0.1.37`, `tv-android-v1.0.0` and `tv-android-v1.0.3`. All accept it.

**Why one kind per row.** Netflix's own Top 10 rows are split by type ("Top 10 Movies", "Top 10 TV Shows"). A row that mixes anime, series and films also needs the engine to merge three catalogues, and each has its own idea of "popular". One kind per row needs no new code in the engine, and it keeps each tab (Anime, Series, Films) full: the tab shows the rows of its own kind.

**Rejected.**

- *The Netflix growing tile* (the tile grows in place and plays a preview). It is a big change, it needs video in the tile, and our design plan says the app should not be a Netflix copy.
- *A "Coming soon" row.* The catalogues hide upcoming titles on purpose (`drop` rules in `sources.json`). A row for them needs a new sort, and a new sort value would make every old app refuse the document. It needs its own plan.
- *A "Because you watched" row.* It needs a similar-titles source we do not have for every kind.

**Trade-offs.**

- The numbers on the website and the desktop app share one width (80 px) so the row has no uneven steps, and "10" is drawn smaller than the TV's number so it fits. The TV gives "10" extra room, because its list does not need equal steps.
- Number sizes are fixed: they do not grow with a very large system font.
- "Top 10 films" and "Top 10 series" are "popular", not "today": Cinemeta has no daily chart. Only anime is "now", because AniList has a real trending sort. The names say so.
- The row names are data, so a name change is a publish and not a release.
- **Rollout order matters.** The server reads a new sources document without a deploy, but only a server that has this change cuts a ranked row to ten and sends `style`. So deploy the server first, and publish version 53 after. Publish first and the website shows "Top 10" over 24 plain tiles until the deploy ends.
- An old desktop app or TV app (before this change) shows a ranked row as a plain row of up to 24 titles, because it does not know `style`. The website is not affected: the server cuts the row.

## TV Settings has no scroll (2026-11-28)

Release 1.0.3 made the TV Settings page scroll, so that the update row could not fall off the bottom. It broke the remote: pressing Down from the top bar did nothing, so "Download and install" could not be reached. A scroll container takes the focus search for itself, and the search stops at the container instead of going to the buttons inside. Home, Browse and Search have no such container around their buttons, which is why they worked.

**Chosen.** The page does not scroll. The two notices stay at the top (right under the title), where they are always on the screen, and each takes room only while it shows. At 1920 by 1080 and 320 dpi (540 dp tall) the page fits with the update row and a saved key.

**Trade-off.** With both notices, a saved key and an error line all at once, the last line can touch the bottom edge. It is rare, and every button is still reachable. A page that must scroll later needs its own focus handling, and a test on a real remote.

**How it was checked.** On the TV emulator at the real TV's size: Down from the Settings tab lands on "Download and install", Down again on the key field, again on "Save and check", and Up goes back to the bar. The same steps on the 1.0.3 build stopped at the bar.

## The TV home hero says when a title comes (2026-11-28)

The title page of the TV already said "Coming 9 January 2027" in place of Play. The facts line of the home hero still showed the year, so the website, the desktop app and the title page said one thing and the home hero another. Now `facts` in `HomeScreen.kt` uses `Title.comingSoon` in place of the year, as `detailFacts` does.

**Why this changes little today.** The catalogues drop upcoming titles from their lists on purpose, so no home row holds one right now (I asked the local server for every row). The words show when a row does hold such a title, for example a "popular" row that gets one from AniList. A unit test (`LogicTest`) covers the three cases: a title that is out, one with a date, one with only "Coming soon".

## The Top 10 number is never cut off on the TV (2026-11-28)

On the TV, moving back to the first card of a Top 10 row scrolled the row until the poster was clear of the left edge, and the big number to its left went off the screen: only a sliver of the "1" showed. The cause: the scroll rule (`LeastToShow`) keeps the card that has the focus in view, and the number is not part of that card. The number is a separate text beside the poster.

**Chosen.** `RankedCard` tells the scroll rule that the poster belongs with its number (`withItsNumber` in `Components.kt`: the area the poster asks to show starts at the number's edge). It is the same idea as `roomForName`, which makes room for the name under a poster. **Rejected:** a larger margin for every row (it changes plain rows too), and a number drawn inside the poster (it would cover the picture).

**How it was found and checked.** The user saw it on the TV. On the emulator at the TV's size, the same keys (right four times, left four times) cut the "1" on the old build and show it in full on the fixed one. My first test after the Top 10 change missed it, because it went back only to card 5 and 6, never to card 1.

**Desktop: the number is as tall as the poster (2026-11-28).** The first desktop version drew the Top 10 number at the foot of the whole tile, so it sat beside the name under the poster, and the name touched it. The number box is now exactly as tall as the poster (`TILE_HEIGHT`), so the number sits level with the foot of the poster. The website does the same. Checked by running the desktop app and looking at the window: the first time I saw the desktop screens.
