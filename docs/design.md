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

**Chosen.** `desktop/scripts/check-old-apps.sh` builds the checker from an old release tag and runs it on a
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
`desktop/core/build.rs` downloads the file to `desktop/sources/sources.json` (git-ignored) when it is missing, so every
build, local or CI, gets a built-in copy without any workflow step. `publish-sources.sh` needs `SOURCES_DIR`, a checkout of
the new repo, so what gets published is always what is committed there.

**Why.** Changing a source should not mean touching the code repo, and the code repo should be only the skeleton.

**Turned down.** A git submodule (reproducible, but every data change needs a pin commit here). A download step in each
workflow (six places to forget; `build.rs` covers all of them, plus the iOS and Android engine builds).

**Trade-off.** A build needs the network once, and it is not reproducible: it reads the newest `main` of the sources repo.
A bad commit there can break this repo's tests (`read_tests.rs`, `screens.test.ts` read the document). To pin an older copy,
put your own file at `desktop/sources/sources.json`. The old copies stay in this repo's git history.

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

**Chosen.** A manga source can have an `auth` section. It says which fields the person types (an email, a password), which request signs in, and where in the answer the session is (`access`, `refresh`, `user`). Those session values become holes (`{access}`) in every later request, and an `authorization` header is added to requests to `base_url`. If a request gets 401, the engine asks the `refresh` request once and tries again. The code is `desktop/manga/src/source.rs` (`sign_in`, `sign_out`, `refresh_unless_done`) and the shapes are in `def.rs` (`AuthDef`).

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

**In the apps (same day).** The engine does not store the session itself. The app gives it a `SessionKeeper` (a small interface in `desktop/manga-ffi/src/lib.rs`, with two calls: `load` and `save`). The engine calls `load` when a source loads and `save` every time the session changes, a refresh included. So the apps only hold a safe key-value store: `manga-app/App/SessionKeychain.swift` (Keychain, this device only, after first unlock) and `manga-android/.../sources/SessionKeystore.kt` (AES-GCM with a key that lives in the Android Keystore). All the sign-in logic stays in Rust.

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

The cause was the loop in `desktop/app/src/player.rs` that both reads mpv events and draws frames. It waited up to 50 ms for events, so frames were drawn late and mpv threw them away. Now it waits at most 4 ms. The same file then played at a steady 24 fps (frames 42 ms apart) and mpv dropped one frame in 40 seconds. CPU is about 30% of one core for 4K.

Rejected for now: drawing on the GPU instead of the CPU, which mpv's own header suggests for speed. The CPU path costs little on Apple silicon (about 20% of one core for 1080p), and the GPU path needs a large change in how GPUI shows frames. We will do it only if a real file still stutters.

Not tested: Windows, and files at 60 fps.

## A skipping catalogue is asked for one page, and a second only when the first is short (2026-11-22)

We measured the desktop app before changing anything. Home scrolls at 120 fps, a title page needs one request (about 120 ms), and the window opens in about 215 ms, most of it the graphics framework. The gap was in the network: a cold start made about 19 requests, and the banner waited for the slowest.

The cause: for a catalogue that pages by skip (Cinemeta, 48 titles a page), `desktop/core/src/providers/list.rs` always asked for one page more than the row needed, "in case" titles were left out. The banner needs 6 titles and a row 24, so the extra page of about 120 KB was almost never used, but the banner waited for it.

Now one page is asked for. If it comes up short after titles are left out (not out yet, twice) and the catalogue goes on, one more page is asked for. Three cold launches each, alternating old and new: banner median 1808 ms before, 384 ms after; first row 546 ms before, about 270 ms after. Two tests in `desktop/core/src/providers/read_tests.rs` cover both cases.

Not changed, on purpose:
- Rows that read a window of 3 pages (the date sort and genre rows). Reading fewer would show older titles in "New Releases", and Cinemeta's own genre address did not give matching titles in a test.
- Pressing Play takes about 13 s to the first frame, mostly torrent pieces (the file index at the far end of a 12 GB file, the resume jump). That needs a change in the torrent engine, so it was left for its own piece of work.

## Tried and left out: fetching the end of the file at the same time as the start (2026-11-22)

When Play is pressed, the torrent engine fetches the first 3 MB of the file and only then the last 3 MB, where an MKV keeps its index. mpv waited about 6 s for that index on a 4K file. We made the two fetches run together and compared old and new builds on the same release, alternating runs. The index was then local (mpv opened the file in 0.02 s), but the start of the file took about 6 s longer (2.2 s became 8.4 s) because both fetches share the same few peers. Total time to the first frame was about the same, with too few runs to tell. We reverted it.

What this tells us: a Play start of 10 to 20 s on a 4K WEB-DL is limited by how fast the swarm sends, not by the order we ask. The release ranker puts the healthiest swarm first (50 or more seeders), and that is often a 4K file. Changing that trades startup speed against how many seeders the pick has, so it is a product choice, not a bug fix.

## The arrows at the ends of a row fade the right way (2026-11-22)

The fade behind a row's arrow was drawn backwards: solid on the side facing the posters and clear at the edge of the screen. That gave a hard vertical line across the poster, with a bare arrow on it. In `desktop/app/src/row.rs` the fade is now solid at the screen edge and clears toward the posters, 96 px wide instead of 64, and the arrow sits in a quiet disc (the same wash as the hover on a button) so it reads as a button. Checked on screen at both ends of a row. Not done: the website's rows, which were not looked at.

## The search icon always ends on a page of titles, and the web palette opens at once (2026-11-22)

The magnifier in the top bar now does the same in both apps. On a screen that is not a browse screen it goes to Browse (the app: `Route::Browse`, the site: `/search`, which with nothing typed shows every catalogue) and opens the Cmd-K panel on top, so closing the panel leaves the viewer on a page of titles and not on whatever screen they came from. On a browse screen (any catalogue tab or Browse) it just opens the panel. In the app the music screens keep their own panel, as before. The code is in `desktop/app/src/app.rs` and `akidev-frontend/src/components/topbar.tsx`.

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

"Remove from history" is one engine call, `forget_history` (`desktop/manga/src/store.rs`, exported in `desktop/manga-ffi/src/lib.rs`). It deletes every progress row of that one series. History is built from progress, so there is no separate history table to clear.

Trade-off: this also forgets which chapters were read. Rejected: deleting only the unfinished row. The series would then stay in the History list, which is not what "remove" says. Another cost: it acts at once, with no question, like the other menu items in the library.

## Speed pass: what was slow and what was not (2026-11-23)

I timed things before and after, on a Mac and on the iPhone simulator. The numbers are the reason for each choice.

- Date and chapter-number reading built its regular expressions on every call. A chapter list reads two per row. They are now built once (`desktop/manga/src/date.rs`). 2000 rows went from 1069 ms to 40 ms on the Mac. A phone is slower, so the win there is bigger in real time.
- The HTTP client spoke only HTTP/1.1. I turned on `http2` and `brotli` in `desktop/manga/Cargo.toml`. Twelve parallel requests to one host went from about 252 ms to about 135 ms. The first request costs the same. It only helps a site that speaks HTTP/2 (a site that does not still works as before).
- The home feed asked every source for its popular and new lists on every open. A list the engine kept in the last ten minutes is now shown and not asked for again (`Engine.fresh_listing`). Pull to refresh and "Try again" still ask again. On the simulator a relaunch went from 3 listing requests to 0. The first launch with an empty cache is unchanged (4 requests). At most four sites are asked at a time, so the first sources answer first. Reading what was kept now happens off the main thread.
- A series that was opened in the last ten minutes no longer asks again for its details when the stored ones exist (it already skipped the chapter list).
- The library sorted its whole list up to three times for each screen draw (iOS). It now sorts once.
- Android covers: a cover that is still waiting for a connection is dropped when the last row that wanted it scrolls away. Before, a fast scroll left the covers on screen waiting behind dozens of covers scrolled past. A download that has begun is still finished and kept.

Measured and left alone: starting the engine takes about 2 ms (12 ms the first time), and every engine read is under 1 ms (the library after a write is 7 ms on a big library). The audit guessed 50 to 300 ms for start-up. That guess was wrong, so I did not move the engine off the main thread. Saving a 2000-chapter list takes about 10 ms, also left alone.

Not measured: the Android cover change and the iOS library change (no device scroll test), the Android feed request count (the Android app has no demo repository switch), and how much the library file grew from HTTP/2 and brotli.

Rejected: a cache inside `popular` and `latest` themselves. Pull to refresh would then need a flag in every call, and the apps already know when the person asked for fresh data.

Trade-off: a list can be up to ten minutes old. New releases on a site can show up that much later unless you pull to refresh.

## The phone app page: one phone that follows the text (2026-11-23)

Chosen: a new page, `/app` (`akidev-frontend/src/app/app/page.tsx`), for the iPhone and Android app, with links to the App Store and Google Play. One phone stays on screen while the text scrolls past. The screen on the phone follows the text: a short recording of the app first, then a picture for each part (sources, a series, reading, library, books, read aloud). The phone turns a little for each part and follows the mouse.

Why: a store link alone does not say what the app is. The film we made already has the app on a phone, so the page reuses it. `/manga` stays as it was, because the App Store listing links to it.

The pictures are not the store screenshots. Those show a real series. The page uses the film's made-up art, taken by `manga-android/store/film/tools/web-assets.py`, and says so in the fine print. The words come from the Play listing and the app's own screens, nothing more. "Free" and the iOS 26 and Android 10 minimums were read from the stores and `build.gradle.kts`.

Details that matter:
- Yellow is used here (and nowhere else on the site) because this page is about the phone app, and the phone app is yellow. The rest of the site keeps amber for state.
- The store buttons are our own: a plain Apple mark and a play triangle, with text. The official badge art needs a download from Apple and Google, so it is not in. Swap it in if you want it.
- The phone turns on a critically damped spring (no bounce, about half a second), starting from where it is, so a new push or a new part never snaps it. Without motion allowed there is no turn, no video, and a short fade between screens.
- The phone is sized from the screen height (`svh`), so it fits any window. On a narrow screen only its top part shows above the text, so the store buttons are in view at once.
- A store click sends the same `download_clicked` event as the desktop page, with `ios` or `android`.

Rejected: a full-width background video. The film is square and crops badly on a wide window. Also rejected: scrubbing a video with the scroll bar. It costs a lot of work for little gain over six pictures.

Trade-off: the pictures and the recording are copies of the film. If the app changes, run the script again and copy the files into `akidev-frontend/public/phone/`.

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
- The bright look is the same token names with light values (`html[data-landing]` in `globals.css`), so every colour class on the page works unchanged. This is an exception to `design/DESIGN_PLAN.md`, for this host only: the phone app has its own look (`manga-app/DESIGN.md`).
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
