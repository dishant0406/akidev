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
