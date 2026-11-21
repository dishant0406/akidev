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
